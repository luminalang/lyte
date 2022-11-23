mod forall;
pub use forall::{Constraint, Generics};

mod r#type;
pub use r#type::{Key, Type, TypeData, TypeKind};

pub type TypesBuf<D> = Vec<Type<D>>;
pub type Types<D> = [Type<D>];

mod infer;
pub use infer::RefID;
pub use infer::TEnv;

mod check;
pub use check::{Error, ErrorHandler, TypeContext};

mod query;
pub use query::{Impl, TraitIndex};

mod mapping;
pub use mapping::Mapping;

pub mod frontend;

mod visitor;
pub use visitor::TypeVisitor;

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ImplID(usize);
