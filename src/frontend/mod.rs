use crate::*;

mod function;
mod product;
mod sum;
mod r#trait;

pub use function::{ForeignFunction, Function};
pub use product::{ForeignProduct, InstantiatedProduct, Product};
pub use r#trait::{ForeignTrait, InstantiatedTrait};
pub use sum::{ForeignSum, InstantiatedSum, Sum};

#[macro_export]
macro_rules! impl_to_type {
    ($inst:ty, $data:ident) => {
        impl<'a, D: TypeData> $inst {
            /// Returns the instantiated form of this type
            pub fn to_type(&self, meta: D::Meta) -> Type<D> {
                Type::concrete(
                    meta.clone(),
                    self.$data.identifier.clone(),
                    self.mapping.to_types(meta),
                )
            }
        }
    };
}

#[macro_export]
macro_rules! define_key {
    ($name:ident) => {
        define_key!($name, "");
    };
    ($name:ident, $comment:literal) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        #[doc=$comment]
        pub struct $name(usize);

        impl From<usize> for $name {
            fn from(i: usize) -> $name {
                $name(i)
            }
        }

        impl $name {
            pub fn index(self) -> usize {
                self.0
            }
        }
    };
}
