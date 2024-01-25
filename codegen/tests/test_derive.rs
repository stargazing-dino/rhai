use rhai_codegen::CustomType;

// Sanity check to make sure everything compiles
#[derive(Clone, CustomType)]
pub struct Foo {
    #[get(get_bar)]
    bar: i32,
    #[readonly]
    baz: String,
    qux: Vec<i32>,
}

fn get_bar(_this: &mut Foo) -> i32 {
    42
}
