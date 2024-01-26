use rhai::{CustomType, TypeBuilder, FLOAT, INT};

// Sanity check to make sure everything compiles

#[derive(Clone, CustomType)]
pub struct Bar(
    #[cfg(not(feature = "no_float"))] // check other attributes
    #[rhai_custom_type_skip]
    FLOAT,
    INT,
    #[rhai_custom_type_name("boo")]
    #[rhai_custom_type_readonly]
    String,
    Vec<INT>,
);

#[derive(Clone, CustomType)]
pub struct Foo {
    #[cfg(not(feature = "no_float"))] // check other attributes
    #[rhai_custom_type_skip]
    _dummy: FLOAT,
    #[rhai_custom_type_get(get_bar)]
    pub bar: INT,
    #[rhai_custom_type_name("boo")]
    #[rhai_custom_type_readonly]
    pub(crate) baz: String,
    #[rhai_custom_type_set(Self::set_qux)]
    pub qux: Vec<INT>,
}

impl Foo {
    pub fn set_qux(&mut self, value: Vec<INT>) {
        self.qux = value;
    }
}

fn get_bar(_this: &mut Foo) -> INT {
    42
}

#[test]
fn test() {}
