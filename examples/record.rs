use lumina_typesystem::frontend::Product;
use lumina_typesystem::{ErrorHandler, TEnv, TraitIndex, TypeContext};

#[path = "shared.rs"]
mod shared;
use shared::*;

fn main() {
    // State for trait implementations
    let index = TraitIndex::new();

    // State for inference engine
    let mut tenv = TEnv::new();

    // The product type we want to work with
    let product = Product::new(
        "User",
        vec![concrete("string"), concrete("int"), generic('a')],
    )
    .to_foreign(vec!['a'].into_iter().collect());

    // Instantiate the product type
    let mut inst = product.instantiate(&mut tenv);

    // If the type parameter is annotated, then we set that via the mapping.
    inst.mapping_mut()
        .annotate_types(&mut tenv, &index, &[concrete("instead_of_a")])
        .expect("constraint not satisfied");

    // Constructing the product type
    {
        let this_product = inst.to_type(());
        println!("{}", tenv.concretify_type(&this_product));
    }

    // Access fields from the product type
    {
        let name = inst.field(0);
        let age = inst.field(1);
        let data = inst.field(2);

        let mut tctx = TypeContext::new(&mut tenv, &index, ErrorHandler::Expensive);
        tctx.check_types(
            &[name.clone(), age.clone(), data.clone()],
            &[
                concrete("string"),
                concrete("int"),
                concrete("instead_of_a"),
            ],
        )
        .unwrap();

        let fmt = |t| tenv.concretify_type(&t);
        println!(
            "User {{ name: {}, age: {}, data: {} }}",
            fmt(name),
            fmt(age),
            fmt(data)
        );
    }
}
