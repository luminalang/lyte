use crate::{
    check, query, Constraint, ErrorHandler, Generics, RefID, TEnv, TraitIndex, Type, TypeContext,
    TypeData, TypeKind, Types, TypesBuf,
};
use itertools::Itertools;
use owo_colors::OwoColorize;
use std::fmt;

#[derive(Debug, Clone)]
pub struct Mapping<D: TypeData> {
    conversion: Vec<(D::Generic, RefID)>,
    self_: Option<RefID>,
}

impl<D: TypeData> Default for Mapping<D> {
    fn default() -> Self {
        Self {
            conversion: vec![],
            self_: None,
        }
    }
}

fn find<G: PartialEq>(gids: &[(G, RefID)], gid: &G) -> Option<RefID> {
    gids.iter()
        .find_map(|(g, rid)| if g == gid { Some(*rid) } else { None })
}

impl<D: TypeData> Mapping<D> {
    pub(crate) fn get_by_index(&self, idx: usize) -> &(D::Generic, RefID) {
        self.conversion.get(idx).expect("generic does not exist")
    }

    pub fn to_types(&self, meta: D::Meta) -> TypesBuf<D> {
        self.conversion
            .iter()
            .map(|(_, rid)| Type::reference(meta.clone(), *rid, vec![]))
            .collect()
    }

    pub fn resolve_gid(&self, gid: &D::Generic) -> Option<RefID> {
        find(&self.conversion, gid)
    }

    pub fn resolve_self(&self) -> Option<RefID> {
        self.self_
    }

    pub fn assign(&mut self, gid: D::Generic, rid: RefID) {
        self.conversion.push((gid, rid))
    }

    pub fn assign_self(&mut self, rid: RefID) {
        self.self_ = Some(rid);
    }

    pub fn apply(&self, kind: &TypeKind<D>) -> TypeKind<D> {
        match kind {
            TypeKind::Generic(gid) => match self.resolve_gid(gid) {
                Some(rid) => TypeKind::Ref(rid),
                None => panic!("unrecognized generic: {}", gid),
            },
            TypeKind::Self_ => match self.self_ {
                Some(rid) => TypeKind::Ref(rid),
                None => TypeKind::Self_, // TODO: is this dangerous?
            },
            other => other.clone(),
        }
    }

    pub fn apply_type(&self, type_: &Type<D>) -> Type<D> {
        type_.map_constr(&mut |_, constr| self.apply(constr))
    }
    pub fn apply_types(&self, types: &Types<D>) -> TypesBuf<D> {
        types.iter().map(|t| self.apply_type(t)).collect()
    }

    pub fn annotate_self(
        &mut self,
        tenv: &mut TEnv<D>,
        index: &TraitIndex<D>,
        type_: Type<D>,
    ) -> Result<(), AnnotationError<D>> {
        let rid = self.self_.expect("annotate_self called on non-trait");
        Self::annotate_rid(tenv, index, rid, type_)
    }

    pub fn annotate_index(
        &mut self,
        tenv: &mut TEnv<D>,
        index: &TraitIndex<D>,
        idx: usize,
        type_: Type<D>,
    ) -> Result<(), AnnotationError<D>> {
        let (_, rid) = self.get_by_index(idx);
        Self::annotate_rid(tenv, index, *rid, type_)
    }

    pub fn annotate_gid(
        &mut self,
        tenv: &mut TEnv<D>,
        index: &TraitIndex<D>,
        gid: &D::Generic,
        type_: Type<D>,
    ) -> Result<(), AnnotationError<D>> {
        let rid = find(&self.conversion, gid).expect("Generic not defined");
        Self::annotate_rid(tenv, index, rid, type_)
    }

    fn annotate_rid(
        tenv: &mut TEnv<D>,
        traits: &TraitIndex<D>,
        rid: RefID,
        annotated: Type<D>,
    ) -> Result<(), AnnotationError<D>> {
        let entry = tenv.get_mut(rid);
        if let Some(t) = &entry.assignment {
            Err(AnnotationError::AlreadyAssigned(rid, t.value.clone()))
        } else {
            let mut tctx = TypeContext::new(tenv, traits, ErrorHandler::Expensive);

            // TODO: are higher-kinded types treated correctly if we just do this?
            tctx.check_constraints_then_assign(rid, &[], annotated)
                .map_err(|err| match err {
                    check::Error::ConstraintNotMet(type_, constraints, contenders) => {
                        AnnotationError::Constraint(type_, constraints, contenders)
                    }
                    _ => unreachable!(
                        "since we hardcode 0 type params, no check errors should be able to occour"
                    ),
                })
        }
    }

    pub fn annotate_types(
        &mut self,
        tenv: &mut TEnv<D>,
        traits: &TraitIndex<D>,
        types: &Types<D>,
    ) -> Result<(), check::Error<D>> {
        Self::annotate_type_params_raw(&self.conversion, tenv, traits, types)
    }

    fn annotate_type_params_raw(
        buf: &[(D::Generic, RefID)],
        tenv: &mut TEnv<D>,
        traits: &TraitIndex<D>,
        types: &[Type<D>],
    ) -> Result<(), check::Error<D>> {
        assert_eq!(
            buf.len(),
            types.len(),
            "length of applicated types for type doesn't match the types declaration"
        );
        buf.iter().zip(types).try_for_each(|((_, rid), t)| {
            let tentry = tenv.get(*rid);
            assert!(
                tentry.assignment.is_none(),
                "annotate_type_params called on already annotated mapping"
            );
            TypeContext::new(tenv, traits, ErrorHandler::Expensive).check_constraints_then_assign(
                *rid,
                &[],
                t.clone(),
            )
        })
    }
}

#[derive(Clone, Debug)]
pub enum AnnotationError<D: TypeData> {
    AlreadyAssigned(RefID, Type<D>),
    Constraint(Type<D>, Constraint<D>, Vec<query::Contender>),
}

/// Maps the trait declarations generics to the implementations generics
pub struct ImplToTrait<'t, D: TypeData> {
    conversion: Vec<(D::Generic, D::Generic)>,
    trait_mapping: &'t Mapping<D>,
    tenv: &'t TEnv<D>,
    generics: Generics<D>,

    errbuf: Vec<ImplHeaderFailure<D>>,
}

#[derive(Clone, Debug)]
pub enum ImplHeaderFailure<D: TypeData> {
    ConflictingGeneric {
        from_impl_block: (Type<D>, D::Generic),
        in_method: (Type<D>, D::Generic),
    },
    Missmatch {
        got: Type<D>,
        exp: Type<D>,
    },
}

impl<'t, D: TypeData> ImplToTrait<'t, D> {
    pub fn new(generics: Generics<D>, trait_mapping: &'t Mapping<D>, tenv: &'t TEnv<D>) -> Self {
        Self {
            conversion: vec![],
            generics,
            tenv,

            trait_mapping,

            errbuf: vec![],
        }
    }

    fn missmatch(&mut self, got: Type<D>, exp: Type<D>) {
        self.errbuf.push(ImplHeaderFailure::Missmatch { got, exp })
    }

    fn conflict(
        &mut self,
        in_method: (Type<D>, D::Generic),
        from_impl_block: (Type<D>, D::Generic),
    ) {
        self.errbuf
            .push(ImplHeaderFailure::ConflictingGeneric { in_method, from_impl_block });
    }

    pub fn type_(&mut self, got: Type<D>, exp: &Type<D>) -> Type<D> {
        let constr = match (&got.constr, &exp.constr) {
            (TypeKind::Self_, TypeKind::Self_) => TypeKind::Ref(self.trait_mapping.self_.unwrap()),
            (_, TypeKind::Self_) => {
                let rid = self.trait_mapping.self_.unwrap();
                TypeKind::Ref(rid)
            }
            (TypeKind::Ref(_), _) | (_, TypeKind::Ref(_)) => panic!("unexpected inference"),
            (TypeKind::Concrete(gcon), TypeKind::Concrete(econ)) if gcon == econ => got.constr,

            (gconstr, TypeKind::Generic(rgid)) => {
                if let Some(rid) = self.trait_mapping.resolve_gid(rgid) {
                    let exp = self.tenv.get(rid).assignment.clone().unwrap();

                    if !got.direct_eq(&exp.value) {
                        self.missmatch(got.clone(), exp.value.clone());
                    }

                    return exp.value;
                } else {
                    match gconstr {
                        TypeKind::Generic(lgid) => {
                            if let Some(conflicting) =
                                self.trait_mapping.conversion.iter().find_map(|(_, rid)| {
                                    let assigned_by_trait = self.tenv.get_type(*rid)?;
                                    if assigned_by_trait.constr == *gconstr {
                                        Some(assigned_by_trait)
                                    } else {
                                        None
                                    }
                                })
                            {
                                self.conflict(
                                    (got.clone(), lgid.clone()),
                                    (conflicting.clone(), rgid.clone()),
                                );
                                return exp.clone();
                            } else {
                                if let Some((from, _)) =
                                    self.conversion.iter().find(|(_, to)| to == rgid)
                                {
                                    if lgid != from {
                                        self.missmatch(got.clone(), exp.clone());
                                        return got;
                                    }
                                } else {
                                    self.conversion.push((lgid.clone(), rgid.clone()));
                                }
                                gconstr.clone()
                            }
                        }
                        _ => {
                            self.missmatch(got.clone(), exp.clone());
                            return exp.clone();
                        }
                    }
                }
            }
            _ => {
                self.missmatch(got.clone(), exp.clone());
                return exp.clone();
            }
        };

        Type {
            meta: got.meta,
            constr,
            params: self.types(got.params, &exp.params),
        }
    }

    pub fn types(&mut self, got: TypesBuf<D>, exp: &Types<D>) -> TypesBuf<D> {
        got.into_iter()
            .zip(exp.iter())
            .map(|(g, e)| self.type_(g, e))
            .collect()
    }

    #[must_use]
    pub fn finish(self) -> (Generics<D>, Vec<ImplHeaderFailure<D>>) {
        (self.generics, self.errbuf)
    }
}

impl<D: TypeData> fmt::Display for Mapping<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let con_rid = |rid| Type::<D>::reference(D::Meta::default(), rid, vec![]);

        if !self.conversion.is_empty() {
            write!(
                f,
                "{} ({})",
                "converting".green(),
                self.conversion
                    .iter()
                    .format_with(" ", |(gid, rid), f| f(&format_args!(
                        "{} {} {}",
                        gid,
                        "->".purple(),
                        con_rid(*rid),
                    )))
            )?;
        }

        Ok(())
    }
}
