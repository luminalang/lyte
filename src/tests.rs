use super::*;
use frontend::{ForeignFunction, ForeignTrait, Product, Sum};
use insta::assert_display_snapshot as snap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
struct TestTypeData;

impl TypeData for TestTypeData {
    type Concrete = &'static str;
    type Generic = Generic;
    type Trait = &'static str;
    type Association = &'static str;

    type Meta = ();

    fn fmt_specific(
        constr: &Self::Concrete,
        t: &Type<Self>,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        t.fmt_with_params(constr, f)
    }

    fn first_available(forall: &Generics<Self>) -> Generic {
        let mut n = 0;
        loop {
            if forall.iter().any(|(g, _)| g.0 == n) {
                n += 1;
            } else {
                break Generic(n);
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash)]
struct Generic(u8);

impl Key for Generic {}

macro_rules! forall {
    () => {Generics::new()};
    (forall $($gid:ident),*) => {
        forall!($($gid []),*)
    };
    ($($gid:ident $([$($trid:literal $($cparams:ident) *),*])?),*) => {{
        let mut forall: Generics<TestTypeData> = Generics::new();
        $(
            #[allow(unused_assignments)]
            #[allow(unused_mut)]
            let mut constrs = vec![];
            $(
                constrs = vec![$( Constraint::new($trid, vec![$($cparams()),*]) ),*];
            )?
            forall.insert_with_con(gids::$gid, constrs);
        )*
        forall
    }};
}

macro_rules! func {
    (forall $($gid:ident $([$($trid:literal $($cparams:ident) *),*])?),*. ($($p:ident),* => $r:ident)) => {{
        let mut f = func!($($p),* => $r);
        f.generics = forall!( $($gid $([$($trid $($cparams) *),*])?),* );
        f
    }};
    (forall $($gid:ident $([$($trid:literal $($cparams:ident) *),*])?),*. ($($p:expr),* => $r:expr)) => {{
        let mut f = func!($($p),* => $r);
        f.generics = forall!( $($gid $([$($trid $($cparams) *),*])?),* );
        f
    }};
    ($($p:ident),* => $r:ident) => {{
        func! ($($p()),* => $r())
    }};
    ($($p:expr),* => $r:expr) => {{
       ForeignFunction {
           function: Function::new(vec![$($p),*], $r),
           generics: Generics::new()
      }
    }};
}

impl fmt::Display for Generic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", (self.0 + b'a') as char)
    }
}

// fn resolve_impl(implid: ImplID) -> &'static str {
//     match implid.0 {
//         0 => "impl Intable for int",
//         1 => "impl From int for float",
//         2 => "impl Into b for a when b is From a",
//         3 => "impl Functor for option",
//         _ => "error",
//     }
// }

fn trait_from() -> ForeignTrait<TestTypeData> {
    let mut trait_ = ForeignTrait::with_capacity("From", forall!(a), 1);
    trait_.push_method(func!( a => self_ ));
    trait_
}

fn trait_into() -> ForeignTrait<TestTypeData> {
    let mut trait_ = ForeignTrait::with_capacity("Into", forall!(a), 1);
    trait_.push_method(func!( self_ => a ));
    trait_
}

fn trait_mixed_gen() -> ForeignTrait<TestTypeData> {
    let mut trait_ = ForeignTrait::with_capacity("HasGen", forall!(a), 1);
    trait_.push_method(func!( forall b . (self_(), a(), b() => a()) ));
    trait_
}

fn trait_index() -> TraitIndex<TestTypeData> {
    let mut index = TraitIndex::new();
    index.implement(forall!(), "Intable", vec![], int(), vec![]);
    index.implement(forall!(), "From", vec![int()], float(), vec![]);
    index.implement(forall!( a ["From" b], b ), "Into", vec![a()], b(), vec![]);
    index.implement(
        forall!(a),
        "Functor",
        vec![],
        Type::concrete((), "option", vec![]),
        vec![],
    );
    index
}

fn tenv<T, F>(f: F) -> T
where
    F: for<'a> FnOnce(&mut TEnv<TestTypeData>, &TraitIndex<TestTypeData>) -> T,
{
    let mut tenv = TEnv::new();
    let traits = trait_index();
    f(&mut tenv, &traits)
}

fn option<const N: usize>(params: [Type<TestTypeData>; N]) -> Type<TestTypeData> {
    Type::concrete((), "option", params.to_vec())
}
fn int() -> Type<TestTypeData> {
    Type::concrete((), "int", vec![])
}
fn float() -> Type<TestTypeData> {
    Type::concrete((), "float", vec![])
}
fn a() -> Type<TestTypeData> {
    Type::generic((), Generic(0), vec![])
}
fn b() -> Type<TestTypeData> {
    Type::generic((), Generic(1), vec![])
}
fn self_() -> Type<TestTypeData> {
    Type {
        constr: TypeKind::Self_,
        meta: (),
        params: vec![],
    }
}

#[allow(non_upper_case_globals)]
mod gids {
    use super::*;

    pub(super) static a: Generic = Generic(0);
    pub(super) static b: Generic = Generic(1);
    pub(super) static f: Generic = Generic(b'f' - b'a');
}

use frontend::Function;

macro_rules! snap_f {
    ($f:expr, $params:expr) => {
        tenv(|tenv, traits| {
            let inst = $f.instantiate(tenv).function();
            inst.clone().call(tenv, &traits, $params).unwrap();
            snap!(inst.to_foreign(&tenv, Generics::new()));
        })
    };
}

#[test]
fn inst_generics() {
    snap_f!(func!( forall a, b. (a, b => a) ), &[int(), float()]);
}

#[test]
fn inst_intable_for_int() {
    snap_f!(
        func!( forall a ["Intable"], b. (a, b => a) ),
        &[int(), float()]
    );
}

#[test]
fn inst_into_float_for_int() {
    snap_f!(func!( forall a ["Into" float]. (a => float) ), &[int()]);
}

#[test]
#[should_panic]
fn inst_recursive_query() {
    tenv(|tenv, traits| {
        let f = func!( forall a [ "From" a ]. (a => a) );
        let inst = f.instantiate(tenv).function();
        inst.call(tenv, traits, &[int()]).unwrap();
        let a = inst.to_foreign(tenv, Generics::new());
        println!("false positive:\n  {}", a);
    })
}

#[test]
fn inst_functor() {
    let map = func!( forall f ["Functor"]. (Type::generic((), gids::f, vec![int()]) => int()) );
    snap_f!(map, &[option([int()])]);
}

#[test]
fn inst_into_method() {
    tenv(|tenv, traits| {
        let inst = trait_into().instantiate(tenv).method(0, tenv);
        inst.call(tenv, traits, &[int()]).unwrap();
        snap!(inst.to_foreign(tenv, Generics::new()));

        let into = trait_into();
        let inst = into.instantiate(tenv);
        inst.set_self_check_constraint(tenv, traits, float())
            .unwrap();
        let minst = inst.method(0, tenv);
        minst.call(tenv, traits, &[float()]).unwrap();
        snap!(minst.to_foreign(tenv, Generics::new()));
    });
}

#[test]
fn inst_from_method_infer_self() {
    tenv(|tenv, traits| {
        // since there's only *one* implementation of `From int` it should be able to infer the `self_`
        //
        // although; I guess it could also just infer `r when From int` which will also be valid
        let inst = trait_from().instantiate(tenv).method(0, tenv);
        inst.call(tenv, traits, &[int()]).unwrap();
        snap!(inst.to_foreign(tenv, Generics::new()));
    })
}

#[test]
fn inst_product_field() {
    tenv(|tenv, traits| {
        let prod = Product::new("point", vec![a(), a()]).to_foreign(forall!(a));
        let inst = prod.instantiate(tenv);
        TypeContext::new(tenv, traits, ErrorHandler::Expensive)
            .check(&int(), &inst.field(1))
            .unwrap();
        snap!(format!("{}\n{}", inst.to_type(()), tenv));
    })
}

#[test]
fn inst_sum_variant() {
    tenv(|tenv, traits| {
        let sum = Sum::new("option", vec![vec![], vec![a()]]).to_foreign(forall!(a));
        let inst = sum.instantiate(tenv);
        inst.constructor((), 1)
            .call(tenv, traits, &[int()])
            .unwrap();
        snap!(format!("{}\n{}", inst.to_type(()), tenv));
    })
}

#[test]
fn impl_method_matches_trait() {
    tenv(|tenv, traits| {
        let trait_ = trait_mixed_gen();

        let mut inst = trait_.instantiate(tenv);
        inst.mapping_mut()
            .annotate_types(tenv, traits, &[a()])
            .unwrap();
        inst.set_self(float(), tenv);

        let mut gforall = Generics::new();
        gforall.insert(Generic(2));
        let gptypes = vec![float(), a(), b()];
        let greturns = a();

        let given = Function::new(gptypes, greturns).to_foreign(tenv, gforall);

        let (_, failures) = inst.verify_method_annotation(0, given, tenv);

        if !failures.is_empty() {
            panic!("{:#?}", &failures);
        }
    })
}

#[test]
fn impl_generate_typing() {
    tenv(|tenv, traits| {
        let trait_ = trait_mixed_gen();
        let mut inst = trait_.instantiate(tenv);
        inst.mapping_mut()
            .annotate_types(tenv, traits, &[int()])
            .unwrap();
        inst.set_self(float(), tenv);
        let m = inst.generate_method_annotation(0, tenv);
        println!("{}\n{}", m, &tenv);
    })
}
