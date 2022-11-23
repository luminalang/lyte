#![allow(unused)]
fn main() {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeData;

pub type Type = lumina_typesystem::Type<TypeData>;
pub type TypeKind = lumina_typesystem::TypeKind<TypeData>;

impl lumina_typesystem::TypeData for TypeData {
    type Concrete = &'static str;
    type Generic = char;
    type Trait = &'static str;
    type Association = &'static str;

    type Meta = ();

    fn first_available(forall: &lumina_typesystem::Generics<Self>) -> char {
        (0 as char..).find(|n: &char| !forall.contains(*n)).unwrap()
    }
}

pub fn concrete(name: &'static str) -> Type {
    Type::concrete((), name, vec![])
}

pub fn generic(c: char) -> Type {
    Type::generic((), c, vec![])
}
