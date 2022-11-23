use crate::{Generics, RefID, TypesBuf};
use itertools::Itertools;
use std::fmt;
use std::hash::Hash;

pub trait TypeData: Sized + fmt::Debug + Clone + PartialEq {
    type Concrete: Key;
    type Generic: Key;
    type Trait: Key;
    type Association: Key;

    type Meta: Clone + fmt::Debug + Default;

    /// Gets instantiated during formatting so that fancy containers like tuples can be pretty printed
    fn fmt_specific(
        constr: &Self::Concrete,
        t: &Type<Self>,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        t.fmt_with_params(constr, f)
    }

    /// Generate the next unused generic. Used when lifting infered types into top-level declerations
    fn first_available(forall: &Generics<Self>) -> Self::Generic;
}

pub trait Key: PartialEq + Eq + fmt::Debug + Clone + Hash + fmt::Display {}

impl<'a> Key for &'a str {}
impl Key for usize {}
impl Key for u32 {}
impl Key for u16 {}
impl Key for u8 {}
impl Key for char {}
impl Key for String {}

#[derive(Debug, Clone)]
pub struct Type<D: TypeData> {
    pub constr: TypeKind<D>,
    pub meta: D::Meta,
    pub params: Vec<Self>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind<D: TypeData> {
    Generic(D::Generic),
    Concrete(D::Concrete),
    Ref(RefID),
    Self_,
    // Association(usize),
    // REMEMBER: Object-safety rules must be rather strict
    //
    // fn foo(self: Option<&Self>);
    //
    // this would not be allowed because there's no V-table for `None`
    Object(D::Trait),
}

impl<D: TypeData> Type<D> {
    #[must_use]
    pub fn with_params(mut self, params: TypesBuf<D>) -> Self {
        assert!(
            std::mem::replace(&mut self.params, params).is_empty(),
            "`with_params` called onto type already containing parameters"
        );
        self
    }
    pub fn concrete(meta: D::Meta, constr: D::Concrete, params: TypesBuf<D>) -> Self {
        Self {
            params,
            meta,
            constr: TypeKind::Concrete(constr),
        }
    }

    pub fn generic(meta: D::Meta, constr: D::Generic, params: TypesBuf<D>) -> Self {
        Self {
            params,
            meta,
            constr: TypeKind::Generic(constr),
        }
    }

    pub fn object(meta: D::Meta, trait_: D::Trait, params: TypesBuf<D>) -> Self {
        Self {
            constr: TypeKind::Object(trait_),
            meta,
            params,
        }
    }

    pub fn self_(meta: D::Meta, params: TypesBuf<D>) -> Self {
        Self {
            constr: TypeKind::Self_,
            meta,
            params,
        }
    }

    pub fn reference(meta: D::Meta, rid: RefID, hkt: TypesBuf<D>) -> Self {
        Self {
            params: hkt,
            meta,
            constr: TypeKind::Ref(rid),
        }
    }

    pub fn direct_eq(&self, other: &Self) -> bool {
        self.constr == other.constr
            && self
                .params
                .iter()
                .zip(&other.params)
                .all(|(s, o)| s.direct_eq(o))
    }
}

impl<D: TypeData> TypeKind<D> {
    pub fn describe(&self) -> &'static str {
        match self {
            TypeKind::Concrete(_) => "concrete",
            TypeKind::Ref(_) => "inference",
            TypeKind::Object(_) => "trait object",
            TypeKind::Generic(_) => "generic",
            TypeKind::Self_ => "self",
        }
    }
}

impl<D: TypeData> fmt::Display for Type<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.constr {
            TypeKind::Concrete(c) => D::fmt_specific(c, self, f),
            TypeKind::Generic(g) => self.fmt_with_params(g, f),
            TypeKind::Object(trid) => self.fmt_with_params(format!("dyn {}", trid), f),
            TypeKind::Ref(rid) if *rid > 25 => self.fmt_with_params(format!("'{}", rid), f),
            TypeKind::Ref(rid) => {
                self.fmt_with_params(format!("'{}", (*rid as u8 + b'a') as char), f)
            }
            TypeKind::Self_ => self.fmt_with_params("self", f),
        }
    }
}

impl<D: TypeData> fmt::Display for TypeKind<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            TypeKind::Concrete(c) => c.fmt(f),
            TypeKind::Generic(g) => g.fmt(f),
            TypeKind::Object(trid) => trid.fmt(f),
            TypeKind::Ref(rid) if *rid > 25 => write!(f, "'{}", rid),
            TypeKind::Ref(rid) => {
                write!(f, "'{}", (*rid as u8 + b'a') as char)
            }
            TypeKind::Self_ => "self".fmt(f),
        }
    }
}

impl<D: TypeData> Type<D> {
    // TODO: we don't want stuff like ((f a) -> a) when we can have (f a -> a)
    //
    // i guess we can handle that in fmt_specific?
    pub fn fmt_with_params(&self, v: impl fmt::Display, f: &mut fmt::Formatter) -> fmt::Result {
        if self.params.is_empty() {
            v.fmt(f)
        } else {
            write!(f, "({} {})", v, self.params.iter().format(" "))
        }
    }
}
