use super::*;
use itertools::Itertools;
use std::fmt;

#[derive(Debug, Clone)]
pub struct Product<D: TypeData> {
    identifier: D::Concrete,
    fields: TypesBuf<D>,
}

impl<D: TypeData> Product<D> {
    pub fn new(identifier: D::Concrete, fields: TypesBuf<D>) -> Self {
        Self { identifier, fields }
    }

    pub fn to_foreign(self, generics: Generics<D>) -> ForeignProduct<D> {
        // TODO: perform validation that we cannot use any non-existant generics
        ForeignProduct { generics, product: self }
    }
}

#[derive(Debug, Clone)]
pub struct ForeignProduct<D: TypeData> {
    pub generics: Generics<D>,
    pub product: Product<D>,
}

impl<D: TypeData> ForeignProduct<D> {
    pub fn instantiate(&self, tenv: &mut TEnv<D>) -> InstantiatedProduct<D> {
        let mapping = self.generics.to_mapping(tenv);
        InstantiatedProduct {
            mapping,
            product: &self.product,
        }
    }
}

pub struct InstantiatedProduct<'a, D: TypeData> {
    mapping: Mapping<D>,
    product: &'a Product<D>,
}

impl<'a, D: TypeData> InstantiatedProduct<'a, D> {
    pub fn field(&self, field: usize) -> Type<D> {
        self.mapping.apply_type(
            self.product
                .fields
                .get(field)
                .expect("field does not exist"),
        )
    }

    pub fn accessor(&self, ret_meta: D::Meta, field: usize) -> Function<D> {
        let params = vec![self.field(field)];
        let returns = self.to_type(ret_meta);
        Function::new(params, returns)
    }

    pub fn mapping_mut(&mut self) -> &mut Mapping<D> {
        &mut self.mapping
    }
}

impl_to_type!(InstantiatedProduct<'a, D>, product);

impl<D: TypeData> fmt::Display for ForeignProduct<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.generics.is_empty() {
            self.product.fmt(f)
        } else {
            write!(f, "∀{}. {}", &self.generics, &self.product)
        }
    }
}

impl<D: TypeData> fmt::Display for Product<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{{ {} . {} }}",
            self.identifier,
            self.fields.iter().format(", ")
        )
    }
}
