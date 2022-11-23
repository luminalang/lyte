use lumina_typesystem::frontend::Sum;
use lumina_typesystem::{TEnv, TraitIndex};

#[path = "shared.rs"]
mod shared;
use shared::*;

fn main() {
    // State for trait implementations
    let index = TraitIndex::new();

    // State for inference engine
    let mut tenv = TEnv::new();

    // The product type we want to work with
    let sum = Sum::new("Result", vec![vec![generic('a')], vec![generic('e')]])
        .to_foreign(vec!['a', 'e'].into_iter().collect());

    // Instantiate the product type
    let mut inst = sum.instantiate(&mut tenv);

    // If the type parameter is annotated, then we set that via the mapping.
    inst.mapping_mut()
        .annotate_gid(&mut tenv, &index, &'a', concrete("instead_of_a"))
        .expect("constraint not satisfied");

    // Constructing a variant of the sum type
    let f = inst.constructor((), 0);
    f.call(&mut tenv, &index, &[concrete("instead_of_a")])
        .unwrap();

    println!(
        "Ok({}) :: {}",
        concrete("instead_of_a"),
        tenv.concretify_type(&f.returns)
    );
}
