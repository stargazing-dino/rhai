use rhai::{CustomType, TypeBuilder, INT};

// Sanity check to make sure everything compiles
#[derive(Clone, CustomType)]
pub struct Foo {
    #[rhai_custom_type_skip]
    _dummy: INT,
    #[rhai_custom_type_get(get_bar)]
    bar: INT,
    #[rhai_custom_type_name("boo")]
    #[rhai_custom_type_readonly]
    baz: String,
    #[rhai_custom_type_set(Self::set_qux)]
    qux: Vec<INT>,
}

impl Foo {
    pub fn set_qux(&mut self, value: Vec<INT>) {
        self.qux = value;
    }
}

fn get_bar(_this: &mut Foo) -> INT {
    42
}
