use crate::{
    ErrorHandler, Generics, ImplID, Mapping, TEnv, Type, TypeContext, TypeData, TypeKind, Types,
    TypesBuf,
};
use itertools::Itertools;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::fmt;

pub type AssociatedTypes<D> = Vec<(<D as TypeData>::Generic, Type<D>)>;

#[derive(Debug)]
pub struct Impl<D: TypeData> {
    pub forall: Generics<D>,
    pub trait_type_params: TypesBuf<D>,
    pub impltor: Type<D>,

    pub implid: ImplID,
    pub associated: AssociatedTypes<D>,
}

#[derive(Default)]
pub struct TraitIndex<D: TypeData> {
    trids: HashMap<D::Trait, Variants<D>>,
    count: usize,
}

#[derive(Debug)]
struct Variants<D: TypeData> {
    concrete: HashMap<D::Concrete, Vec<Impl<D>>>,
    object: HashMap<D::Trait, Vec<Impl<D>>>,
    blanked: Vec<Impl<D>>,

    default: Option<Impl<D>>,
}

pub struct QuerySuccess<'a, D: TypeData> {
    pub impl_: &'a Impl<D>,
    pub mapping: Mapping<D>,
    pub tenv: TEnv<D>,
    pub unified_impltor: Type<D>,
}

impl<D: TypeData> TraitIndex<D> {
    pub fn new() -> Self {
        TraitIndex {
            trids: HashMap::new(),
            count: 0,
        }
    }

    pub fn implement(
        &mut self,
        generics: Generics<D>,
        trid: D::Trait,
        trtp: TypesBuf<D>,
        impltor: Type<D>,
        associated: AssociatedTypes<D>,
    ) {
        let tvariant = self.trids.entry(trid).or_insert_with(Variants::new);

        let constr = impltor.constr.clone();

        #[cfg(debug_assertions)]
        impltor.map_constr(&mut |_, con| match con {
            TypeKind::Ref(_) | TypeKind::Self_ => {
                panic!("invalid constructor for implementor of trait: {:?}", con)
            }
            other => other.clone(),
        });

        let implid = ImplID(self.count);
        self.count += 1;

        let impl_ = Impl {
            forall: generics,
            trait_type_params: trtp,
            impltor,
            associated,
            implid,
        };

        match constr {
            TypeKind::Generic(_) => tvariant.blanked.push(impl_),
            TypeKind::Concrete(c) => tvariant
                .concrete
                .entry(c)
                .or_insert_with(Vec::new)
                .push(impl_),
            TypeKind::Object(trid) => tvariant
                .object
                .entry(trid)
                .or_insert_with(Vec::new)
                .push(impl_),
            _ => unreachable!(),
        }
    }

    pub fn select(
        &self,
        tenv: &TEnv<D>,
        trait_: D::Trait,
        trait_params: &Types<D>,
        impltor: &Type<D>,
    ) -> Result<SmallVec<[QuerySuccess<'_, D>; 1]>, Vec<Contender>> {
        let variants = match self.trids.get(&trait_) {
            None => return Err(vec![]),
            Some(variants) => variants,
        };

        Selection {
            tenv,
            // trait_,
            trait_params,
            traits: self,
        }
        .run(impltor, variants)
    }
}

struct Selection<'a, D: TypeData> {
    tenv: &'a TEnv<D>,
    traits: &'a TraitIndex<D>,
    // trait_: D::Trait,
    trait_params: &'a Types<D>,
}

impl<'a, D: TypeData> Selection<'a, D> {
    fn run<'s>(
        &mut self,
        impltor: &Type<D>,
        variants: &'s Variants<D>,
    ) -> Result<SmallVec<[QuerySuccess<'s, D>; 1]>, Vec<Contender>> {
        let mut results = SmallVec::new();
        let mut contenders = Vec::new();

        match &impltor.constr {
            TypeKind::Generic(_) => self.filter_suitible(&variants.blanked, impltor, &mut results, &mut contenders),
            TypeKind::Concrete(c) => {
                if let Some(impls) = variants.concrete.get(c) {
                    self.filter_suitible(impls, impltor, &mut results, &mut contenders);
                }
                self.filter_suitible(&variants.blanked, impltor, &mut results, &mut contenders);
            },
            TypeKind::Self_ => todo!("I think this should just be a panicky branch? or perhaps we want to get the assigned in tenv?"),
            TypeKind::Object(trid) => {
                if let Some(impls) = variants.object.get(trid) {
                    self.filter_suitible(impls, impltor, &mut results, &mut contenders);
                }
                self.filter_suitible(&variants.blanked, impltor,  &mut results, &mut contenders);
            }

            TypeKind::Ref(rid) => match self.tenv.get_type(*rid) {
                    Some(impltor) => return self.run(impltor, variants),
                    None => todo!("here we need to check more aggressively in a slow-path to see whichever can be compatible and thereby infer?"),
                }
        }

        if results.is_empty() {
            if let Some(default) = variants.default.as_ref() {
                match self.is_suitible(default, impltor) {
                    Ok(success) => results.push(success),
                    Err(contender) => contenders.push(contender),
                }
            }
        }

        if results.is_empty() {
            Err(contenders)
        } else {
            Ok(results)
        }
    }

    fn filter_suitible<'i>(
        &self,
        impls: &'i [Impl<D>],
        impltor: &Type<D>,
        results: &mut SmallVec<[QuerySuccess<'i, D>; 1]>,
        contenders: &mut Vec<Contender>,
    ) {
        // I guess it might make sense to not allocate contenders and
        // re-iterate the impls as a cold path on errors?
        for impl_ in impls {
            match self.is_suitible(impl_, impltor) {
                Ok(success) => results.push(success),
                Err(contender) => contenders.push(contender),
            }
        }
    }

    fn is_suitible<'i>(
        &self,
        impl_: &'i Impl<D>,
        impltor: &Type<D>,
    ) -> Result<QuerySuccess<'i, D>, Contender> {
        let mut tenv = self.tenv.clone();

        let mapping = impl_.forall.to_mapping(&mut tenv);
        let unified_trtp = mapping.apply_types(&impl_.trait_type_params);
        let unified_impltor = mapping.apply_type(&impl_.impltor);

        #[cfg(debug_assertions)]
        if self.trait_params.len() != unified_trtp.len() {
            panic!("missmatched amount of trait parameters");
        }

        let mut checker = TypeContext::new(&mut tenv, self.traits, ErrorHandler::Cheap);
        checker
            .check_types(self.trait_params, &unified_trtp)
            .map_err(|_| Contender::InvalidTraitParams)?;
        checker
            .check(impltor, &unified_impltor)
            .map_err(|_| Contender::InvalidImpltor)?;

        Ok(QuerySuccess {
            impl_,
            mapping,
            tenv,
            unified_impltor,
        })
    }
}

impl<D: TypeData> Variants<D> {
    fn new() -> Self {
        Self {
            default: None,
            concrete: HashMap::new(),
            object: HashMap::new(),
            blanked: Vec::new(),
        }
    }
}

/// Why the comparison against an implementation failed
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Contender {
    InvalidTraitParams,
    InvalidImpltor,
}

impl<D: TypeData> fmt::Debug for TraitIndex<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.trids.is_empty() {
            "![]!".fmt(f)
        } else {
            writeln!(f, "![")?;
            for (trid, variants) in &self.trids {
                if !variants.concrete.is_empty() {
                    "concrete:".fmt(f)?;
                }
                for impls in variants.concrete.values() {
                    for impl_ in impls {
                        fmt_impl(trid, impl_, f)?;
                    }
                }

                if !variants.blanked.is_empty() {
                    "blanked:".fmt(f)?;
                }
                for impl_ in &variants.blanked {
                    fmt_impl(trid, impl_, f)?;
                }
            }
            write!(f, "\n]!")
        }
    }
}

fn fmt_impl<D: TypeData>(trid: &D::Trait, impl_: &Impl<D>, f: &mut fmt::Formatter) -> fmt::Result {
    writeln!(
        f,
        "  impl {}{} for {}",
        trid,
        if impl_.trait_type_params.is_empty() {
            String::new()
        } else {
            impl_.trait_type_params.iter().format(" ").to_string()
        },
        impl_.impltor
    )
}
