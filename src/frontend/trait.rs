use super::*;
use crate::mapping::{ImplHeaderFailure, ImplToTrait};

#[derive(Debug, Clone)]
pub struct ForeignTrait<D: TypeData> {
    pub identifier: D::Trait,
    pub generics: Generics<D>,
    methods: Vec<ForeignFunction<D>>,
    // TODO: associated types
}

impl<D: TypeData> ForeignTrait<D> {
    pub fn new(identifier: D::Trait, generics: Generics<D>) -> Self {
        if !generics.iter().all(|(_, constr)| constr.is_empty()) {
            panic!(
                "Constraining the type parameters of a trait declaration is currently unsupported"
            );
        }
        Self::with_capacity(identifier, generics, 0)
    }

    pub fn with_capacity(identifier: D::Trait, generics: Generics<D>, cap: usize) -> Self {
        ForeignTrait {
            identifier,
            generics,
            methods: Vec::with_capacity(cap),
        }
    }

    pub fn push_method(&mut self, method: ForeignFunction<D>) {
        self.methods.push(method);
    }

    pub fn instantiate(&self, tenv: &mut TEnv<D>) -> InstantiatedTrait<D> {
        let mut mapping = self.generics.to_mapping(tenv);

        // TODO: This prevents us from having constraints on the generics of an trait declaration's generics.
        let rid = tenv.spawn_with_cons(vec![Constraint {
            trid: self.identifier.clone(),
            params: mapping.to_types(D::Meta::default()), // TODO: meta
        }]);
        mapping.assign_self(rid);

        InstantiatedTrait {
            trid: self.identifier.clone(),
            mapping,
            methods: &self.methods,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstantiatedTrait<'a, D: TypeData> {
    pub trid: D::Trait,
    mapping: Mapping<D>,
    // maybe we should have the self assignment here instead?
    pub methods: &'a Vec<ForeignFunction<D>>,
}

impl<'a, D: TypeData> InstantiatedTrait<'a, D> {
    pub fn set_self(&self, t: Type<D>, tenv: &mut TEnv<D>) {
        let rid = self.mapping.resolve_self().unwrap();
        tenv.assign(rid, t);
    }
    pub fn set_self_check_constraint(
        &self,
        tenv: &mut TEnv<D>,
        traits: &TraitIndex<D>,
        t: Type<D>,
    ) -> Result<(), check::Error<D>> {
        let rid = self.mapping.resolve_self().unwrap();
        TypeContext::new(tenv, traits, ErrorHandler::Expensive).check_constraints_then_assign(
            rid,
            &[],
            t,
        )
    }

    pub fn method(&self, mid: usize, tenv: &mut TEnv<D>) -> Function<D> {
        let mut mapping = self.mapping.clone();

        let f = &self.methods[mid];

        // there's also generic attached to the function that aren't declared for the trait
        f.generics.append_to_mapping(tenv, &mut mapping);

        let ptypes = mapping.apply_types(&f.function.ptypes);
        let returns = mapping.apply_type(&f.function.returns);

        Function::new(ptypes, returns)
    }

    pub fn generate_method_annotation(&self, mid: usize, tenv: &TEnv<D>) -> ForeignFunction<D> {
        let mut tenv = tenv.clone();
        let instantiated = self.method(mid, &mut tenv);
        instantiated.to_foreign(&tenv, Generics::new())
    }

    pub fn mapping_mut(&mut self) -> &mut Mapping<D> {
        &mut self.mapping
    }

    /// Verify that `method` is a valid declaration of this trait
    ///
    /// Returns a valid form of this trait's method
    pub fn verify_method_annotation(
        &self,
        mid: usize,
        method: ForeignFunction<D>,
        tenv: &TEnv<D>,
    ) -> (ForeignFunction<D>, Vec<ImplHeaderFailure<D>>) {
        assert!(self.mapping.resolve_self().is_some());

        let expected = &self.methods[mid];

        let mut converter = ImplToTrait::new(method.generics, &self.mapping, tenv);
        let ptypes = converter.types(method.function.ptypes, &expected.function.ptypes);
        let returns = converter.type_(method.function.returns, &expected.function.returns);

        let (generics, errors) = converter.finish();

        (
            ForeignFunction {
                generics,
                function: Function { ptypes, returns },
            },
            errors,
        )
    }
}
