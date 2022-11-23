use crate::{Constraint, Generics, Type, TypeData, TypeKind, TypeVisitor, Types, TypesBuf};
use itertools::Itertools;
use owo_colors::OwoColorize;
use std::fmt;

pub type RefID = usize;

#[derive(Clone, Debug)]
pub struct Assignment<D: TypeData> {
    pub value: Type<D>,
}

#[derive(Clone, Debug)]
pub struct TEnv<D: TypeData> {
    // we need some way to track associated types as well
    asgn: Vec<TEntry<D>>,
}

#[derive(Clone, Debug)]
pub struct TEntry<D: TypeData> {
    pub assignment: Option<Assignment<D>>,
    pub constraints: Vec<Constraint<D>>,
}

impl<D: TypeData> TEnv<D> {
    pub fn new() -> Self {
        Self { asgn: vec![] }
    }

    pub fn is_empty(&self) -> bool {
        self.asgn.is_empty()
    }

    pub fn assign(&mut self, rid: RefID, value: Type<D>) {
        assert!(
            std::mem::replace(&mut self.asgn[rid].assignment, Some(Assignment { value })).is_none(),
            "type reference already assigned"
        );
    }

    pub fn spawn_with_cons(&mut self, constraints: Vec<Constraint<D>>) -> RefID {
        let rid = self.asgn.len();
        self.asgn.push(TEntry { assignment: None, constraints });
        rid
    }

    pub fn spawn(&mut self) -> RefID {
        self.spawn_with_cons(vec![])
    }

    pub fn spawn_type(&mut self, meta: D::Meta) -> Type<D> {
        Type::reference(meta, self.spawn(), vec![])
    }

    pub fn spawn_types(&mut self, count: usize, meta: impl Fn(usize) -> D::Meta) -> TypesBuf<D> {
        (0..count).map(|pid| self.spawn_type(meta(pid))).collect()
    }

    pub(crate) fn get(&self, rid: RefID) -> &TEntry<D> {
        self.asgn.get(rid).expect("type reference not defined")
    }
    pub(crate) fn get_mut(&mut self, rid: RefID) -> &mut TEntry<D> {
        self.asgn.get_mut(rid).expect("type reference not defined")
    }

    pub(crate) fn get_type(&self, rid: RefID) -> Option<&Type<D>> {
        self.get(rid)
            .assignment
            .as_ref()
            .map(|assignment| &assignment.value)
    }

    pub(crate) fn constraints(&self, rid: RefID) -> &[Constraint<D>] {
        self.get(rid).constraints.as_slice()
    }

    pub(crate) fn constraints_mut(&mut self, rid: RefID) -> &mut Vec<Constraint<D>> {
        &mut self.get_mut(rid).constraints
    }

    pub fn concretify_type(&self, t: &Type<D>) -> Type<D> {
        t.map_type(&mut |meta, constr, params| match constr {
            TypeKind::Ref(rid) => match self.get_type(*rid).cloned() {
                None => Type {
                    meta,
                    constr: constr.clone(),
                    params,
                },
                Some(mut t) => {
                    t.params.extend(params.into_iter());
                    self.concretify_type(&t)
                }
            },
            _ => Type {
                meta,
                constr: constr.clone(),
                params,
            },
        })
    }

    pub fn concretify_types(&self, ts: &Types<D>) -> TypesBuf<D> {
        ts.iter().map(|t| self.concretify_type(t)).collect()
    }

    pub fn into_concretify_type(&self, t: Type<D>) -> Type<D> {
        t.into_map_type(&mut |meta, constr, params| match constr {
            TypeKind::Ref(rid) => match self.get_type(rid).cloned() {
                None => Type { meta, constr, params },
                Some(mut t) => {
                    t.params.extend(params.into_iter());
                    self.concretify_type(&t)
                }
            },
            _ => Type { meta, constr, params },
        })
    }
}

/// Turns local types into foreign top-level types by substituting any un-infered type variables
/// into declared generics.
pub struct Lift<'a, D: TypeData> {
    pub(crate) forall: &'a mut Generics<D>,
    tenv: &'a TEnv<D>,
    reverse_conversion: Vec<(RefID, D::Generic)>,
}

impl<'a, D: TypeData> Lift<'a, D> {
    pub fn new(tenv: &'a TEnv<D>, forall: &'a mut Generics<D>) -> Self {
        Self {
            tenv,
            forall,
            reverse_conversion: vec![],
        }
    }

    pub fn type_(&mut self, t: Type<D>) -> Type<D> {
        self.tenv
            .into_concretify_type(t)
            .into_map_constr(&mut |constr| self.mapper(constr))
    }

    pub fn types(&mut self, t: TypesBuf<D>) -> TypesBuf<D> {
        t.into_iter().map(|t| self.type_(t)).collect()
    }

    fn constraints(&mut self, rid: RefID) -> Vec<Constraint<D>> {
        self.tenv
            .constraints(rid)
            .iter()
            .map(|constraint| {
                constraint.map_types(|meta, constr, params| {
                    let mut t = self.tenv.concretify_type(&Type {
                        meta,
                        constr: constr.clone(),
                        params,
                    });
                    t.constr = self.mapper(t.constr);
                    t
                })
            })
            .collect()
    }

    fn mapper(&mut self, constr: TypeKind<D>) -> TypeKind<D> {
        match constr {
            // TODO: this is invalid; because: it could be assigned in self.tenv no?
            //
            // ALTHOUGH: perhaps we should just concretify_type it first then?
            //
            // BUG: this is reachable through constraints. because we *don't* really concretify
            // those first.
            //
            // So; perhaps we do still want to check the tenv?
            // although; it's more clean to keep it separate.
            TypeKind::Ref(rid) => TypeKind::Generic(self.generate_gid(rid)),
            other => other,
        }
    }

    fn generate_gid(&mut self, rid: RefID) -> D::Generic {
        match self.reverse_conversion.iter().find(|(r, _)| *r == rid) {
            Some((_, gid)) => gid.clone(),
            None => {
                let gid = D::first_available(self.forall);

                self.reverse_conversion.push((rid, gid.clone()));
                self.forall.insert(gid.clone());

                let lifted_constrs = self.constraints(rid);
                self.forall.update_with_cons(gid.clone(), lifted_constrs);

                gid
            }
        }
    }
}

impl<D: TypeData> fmt::Display for TEnv<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}\n  {}",
            "with tenv".green(),
            self.asgn
                .iter()
                .enumerate()
                .format_with("\n  ", |(rid, tentry), f| f(&format_args!(
                    "{} {} {}",
                    Type::<D>::reference(D::Meta::default(), rid, vec![]),
                    "->".purple(),
                    tentry
                ))),
        )
    }
}

impl<D: TypeData> fmt::Display for TEntry<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.assignment.as_ref() {
            Some(asgn) => asgn.fmt(f),
            None => {
                write!(
                    f,
                    "{} {}",
                    "when".green(),
                    self.constraints.iter().format(", ")
                )
            }
        }
    }
}

impl<D: TypeData> fmt::Display for Assignment<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.value.fmt(f)
    }
}
