use super::*;
use itertools::Itertools;
use std::fmt;

#[derive(Debug, Clone)]
pub struct Sum<D: TypeData> {
    pub identifier: D::Concrete,
    pub variants: Vec<TypesBuf<D>>,
}

impl<D: TypeData> Sum<D> {
    pub fn new(identifier: D::Concrete, variants: Vec<TypesBuf<D>>) -> Self {
        Self { identifier, variants }
    }

    pub fn to_foreign(self, generics: Generics<D>) -> ForeignSum<D> {
        ForeignSum { generics, sum: self }
    }
}

#[derive(Debug, Clone)]
pub struct ForeignSum<D: TypeData> {
    pub generics: Generics<D>,
    pub sum: Sum<D>,
}

pub struct InstantiatedSum<'a, D: TypeData> {
    mapping: Mapping<D>,
    pub sum: &'a Sum<D>,
}

impl<D: TypeData> ForeignSum<D> {
    pub fn instantiate<'s>(&'s self, tenv: &mut TEnv<D>) -> InstantiatedSum<'s, D> {
        let mapping = self.generics.to_mapping(tenv);
        InstantiatedSum { mapping, sum: &self.sum }
    }
}

impl<'a, D: TypeData> InstantiatedSum<'a, D> {
    pub fn variant(&self, variant: usize) -> TypesBuf<D> {
        let variant = self
            .sum
            .variants
            .get(variant)
            .expect("variant does not exist");

        self.mapping.apply_types(variant)
    }

    pub fn constructor(&self, ret_meta: D::Meta, variant: usize) -> Function<D> {
        let params = self.variant(variant);
        let returns = self.to_type(ret_meta);

        Function::new(params, returns)
    }

    pub fn mapping_mut(&mut self) -> &mut Mapping<D> {
        &mut self.mapping
    }
}

impl_to_type!(InstantiatedSum<'a, D>, sum);

impl<D: TypeData> fmt::Display for ForeignSum<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.generics.is_empty() {
            self.sum.fmt(f)
        } else {
            write!(f, "∀{}. {}", &self.generics, &self.sum)
        }
    }
}

impl<D: TypeData> fmt::Display for Sum<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} = {}",
            self.identifier,
            self.variants
                .iter()
                .format_with(" | ", |params, f| f(&format_args!(
                    "_ {}",
                    params.iter().format(" ")
                )))
        )
    }
}
