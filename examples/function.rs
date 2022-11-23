use lumina_typesystem::frontend::Function;
use lumina_typesystem::{TEnv, TraitIndex};

#[path = "shared.rs"]
mod shared;
use shared::*;

fn main() {
    // State for trait implementations
    let index = TraitIndex::new();

    // State for inference engine
    let mut tenv = TEnv::new();

    // The function we want to call
    let f = Function::new(vec![generic('a'), generic('b')], generic('a'))
        .to_foreign(&tenv, vec!['a', 'b'].into_iter().collect());

    // Calling the function
    let inst = f.instantiate(&mut tenv).function();
    inst.call(&mut tenv, &index, &[concrete("int"), concrete("float")])
        .unwrap();

    // Remove any tvar indirection
    let returns = tenv.concretify_type(&inst.returns);

    assert_eq!(returns.constr, TypeKind::Concrete("int"));

    println!(
        "the function `{}` returns `{}` when called with `[int, float]`",
        &f, &returns
    );
}
