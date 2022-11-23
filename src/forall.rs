use crate::{Mapping, TEnv, Type, TypeData, TypeKind, TypeVisitor, TypesBuf};
use itertools::Itertools;
use std::fmt;
use std::iter::FromIterator;

#[derive(Debug, Clone)]
pub struct Constraint<D: TypeData> {
    pub trid: D::Trait,
    // do we need to store some kind of HKT info here?
    pub params: TypesBuf<D>,
}

#[derive(Debug, Clone, Default)]
pub struct Generics<D: TypeData> {
    inner: Vec<(D::Generic, Vec<Constraint<D>>)>, // constrs?
}

impl<D: TypeData> Generics<D> {
    pub fn new() -> Self {
        Self { inner: vec![] }
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn contains(&self, gid: D::Generic) -> bool {
        self.try_constraints(&gid).is_some()
    }

    pub fn position(&self, gid: D::Generic) -> Option<usize> {
        self.inner.iter().position(|(g, _)| g == &gid)
    }

    pub fn insert(&mut self, gid: D::Generic) {
        self.inner.push((gid, vec![]));
    }
    pub fn insert_with_con(&mut self, gid: D::Generic, con: Vec<Constraint<D>>) {
        self.inner.push((gid, con));
    }
    pub fn update_with_cons(&mut self, gid: D::Generic, cons: Vec<Constraint<D>>) {
        if let Some(i) = self.inner.iter().position(|(g, _)| *g == gid) {
            self.inner[i].1.extend(cons.into_iter());
        } else {
            self.insert_with_con(gid, cons);
        }
    }
    pub fn update_with_con(&mut self, gid: D::Generic, con: Constraint<D>) {
        if let Some(i) = self.inner.iter().position(|(g, _)| *g == gid) {
            self.inner[i].1.push(con);
        } else {
            self.insert_with_con(gid, vec![con]);
        }
    }

    pub fn constraints(&self, gid: &D::Generic) -> &[Constraint<D>] {
        self.try_constraints(gid)
            .expect("generic not declared in forall")
    }

    pub fn try_constraints(&self, gid: &D::Generic) -> Option<&[Constraint<D>]> {
        self.inner.iter().find_map(|(g, constr)| {
            if g == gid {
                Some(constr.as_slice())
            } else {
                None
            }
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &(D::Generic, Vec<Constraint<D>>)> {
        self.inner.iter()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn to_mapping(&self, tenv: &mut TEnv<D>) -> Mapping<D> {
        let mut mapping = Mapping::default();
        self.append_to_mapping(tenv, &mut mapping);
        mapping
    }

    pub fn append_to_mapping(&self, tenv: &mut TEnv<D>, mapping: &mut Mapping<D>) {
        for (gid, _) in self.iter() {
            let rid = tenv.spawn();
            mapping.assign(gid.clone(), rid);
        }

        for (gid, constrs) in self.iter() {
            let constrs = constrs
                .iter()
                .map(|constr| constr.map_constr(|_, kind| mapping.apply(kind)))
                .collect::<Vec<Constraint<_>>>();

            let rid = mapping.resolve_gid(gid).unwrap();

            tenv.constraints_mut(rid).extend(constrs);
        }
    }

    pub fn extend(&mut self, other: &Self) {
        self.inner.extend(other.inner.iter().cloned())
    }
}

impl<D: TypeData> FromIterator<D::Generic> for Generics<D> {
    fn from_iter<I: IntoIterator<Item = D::Generic>>(iter: I) -> Self {
        Generics {
            inner: iter.into_iter().map(|gid| (gid, vec![])).collect(),
        }
    }
}

impl<D: TypeData> Constraint<D> {
    pub fn new(trid: D::Trait, params: Vec<Type<D>>) -> Self {
        Self { trid, params }
    }
}

impl<D: TypeData> TypeVisitor<D> for Generics<D> {
    fn map_types<F: FnMut(D::Meta, &TypeKind<D>, TypesBuf<D>) -> Type<D>>(&self, mut f: F) -> Self {
        Generics {
            inner: self
                .iter()
                .map(|(gid, constrs)| {
                    (
                        gid.clone(),
                        constrs
                            .iter()
                            .map(|constr| constr.map_types(&mut f))
                            .collect(),
                    )
                })
                .collect(),
        }
    }
}

impl<D: TypeData> TypeVisitor<D> for Constraint<D> {
    fn map_types<F: FnMut(D::Meta, &TypeKind<D>, TypesBuf<D>) -> Type<D>>(&self, mut f: F) -> Self {
        Constraint {
            trid: self.trid.clone(),
            params: self.params.iter().map(|t| t.map_type(&mut f)).collect(),
        }
    }
}

impl<D: TypeData> fmt::Display for Generics<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner
            .iter()
            .format_with(", ", |(gen, constrs), f| {
                if constrs.is_empty() {
                    f(gen)
                } else {
                    f(&format!("{} is {}", &gen, constrs.iter().format(" & ")))
                }
            })
            .fmt(f)
    }
}

impl<D: TypeData> fmt::Display for Constraint<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.params.is_empty() {
            self.trid.fmt(f)
        } else {
            write!(f, "{} {}", &self.trid, self.params.iter().format(" "))
        }
    }
}
