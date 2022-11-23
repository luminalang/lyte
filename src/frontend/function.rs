use super::*;
use crate::{infer::Lift, TypeVisitor};
use itertools::Itertools;
use std::fmt;

#[derive(Debug, Clone)]
pub struct Function<D: TypeData> {
    pub ptypes: TypesBuf<D>,
    pub returns: Type<D>,
}

#[derive(Debug, Clone)]
pub struct ForeignFunction<D: TypeData> {
    pub generics: Generics<D>,
    pub function: Function<D>,
}

impl<D: TypeData> TypeVisitor<D> for Function<D> {
    fn map_types<F: FnMut(D::Meta, &TypeKind<D>, TypesBuf<D>) -> Type<D>>(&self, mut f: F) -> Self {
        Function {
            ptypes: self.ptypes.iter().map(|t| t.map_type(&mut f)).collect(),
            returns: self.returns.map_type(&mut f),
        }
    }
}

impl<D: TypeData> Function<D> {
    /// Lift to a standalone instantiable function.
    ///
    /// Any types pending to be infered in the parameters or return type will be lifted into
    /// generics implicitly declared in `generics`
    pub fn to_foreign(&self, tenv: &TEnv<D>, mut generics: Generics<D>) -> ForeignFunction<D> {
        let ptypes = tenv.concretify_types(&self.ptypes);
        let returns = tenv.concretify_type(&self.returns);

        let mut lifter = Lift::new(tenv, &mut generics);

        ForeignFunction {
            function: Function {
                ptypes: lifter.types(ptypes),
                returns: lifter.type_(returns),
            },
            generics,
        }
    }
}

impl<D: TypeData> ForeignFunction<D> {
    pub fn with_extra_generics(mut self, generics: &Generics<D>) -> Self {
        self.generics.extend(generics);
        self
    }

    pub fn instantiate(&self, tenv: &mut TEnv<D>) -> InstantiatedFunction<D> {
        let mapping = self.generics.to_mapping(tenv);
        InstantiatedFunction {
            mapping,
            function: &self.function,
        }
    }
}

impl<D: TypeData> Function<D> {
    pub fn new(ptypes: TypesBuf<D>, returns: Type<D>) -> Self {
        Function { ptypes, returns }
    }
}

pub struct InstantiatedFunction<'a, D: TypeData> {
    mapping: Mapping<D>,
    function: &'a Function<D>,
}

impl<'a, D: TypeData> InstantiatedFunction<'a, D> {
    pub fn function(&self) -> Function<D> {
        let ptypes = self.mapping.apply_types(&self.function.ptypes);
        let returns = self.mapping.apply_type(&self.function.returns);
        Function { ptypes, returns }
    }

    pub fn mapping_mut(&mut self) -> &mut Mapping<D> {
        &mut self.mapping
    }
}

impl<D: TypeData> Function<D> {
    pub fn call(
        &self,
        tenv: &mut TEnv<D>,
        traits: &TraitIndex<D>,
        params: &Types<D>,
    ) -> Result<(), CallError<D>> {
        let expected = &self.ptypes;

        let got = params.len();
        let exp = params.len();

        if got != exp {
            return Err(CallError::ParamCount { got, exp });
        }

        let mut tctx = TypeContext::new(tenv, traits, ErrorHandler::Expensive);
        let mut errors = Vec::new();
        for pid in 0..got {
            if let Err(err) = tctx.check(&params[pid], &expected[pid]) {
                errors.push((pid, err));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(CallError::CheckErrors(errors))
        }
    }
}

#[derive(Clone, Debug)]
pub enum CallError<D: TypeData> {
    CheckErrors(Vec<(usize, check::Error<D>)>),
    ParamCount { got: usize, exp: usize },
}

impl<D: TypeData> fmt::Display for ForeignFunction<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.generics.is_empty() {
            self.function.fmt(f)
        } else {
            write!(f, "∀{}. {}", &self.generics, &self.function)
        }
    }
}

impl<D: TypeData> fmt::Display for Function<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "({} -> {})",
            self.ptypes.iter().format(", "),
            &self.returns
        )
    }
}
