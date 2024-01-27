use rhai::{CustomType, Engine, TypeBuilder, FLOAT, INT};

// Sanity check to make sure everything compiles

#[derive(Clone, CustomType)]
pub struct Bar(
    #[cfg(not(feature = "no_float"))] // check other attributes
    #[rhai_type_skip]
    FLOAT,
    INT,
    #[rhai_type_name("boo")]
    #[rhai_type_readonly]
    String,
    Vec<INT>,
);

#[derive(Clone, Default, CustomType)]
#[rhai_type_name("MyFoo")]
#[rhai_type_extra(Self::build_extra)]
pub struct Foo {
    #[rhai_type_skip]
    _dummy: FLOAT,
    #[rhai_type_get(get_bar)]
    pub bar: INT,
    #[rhai_type_name("boo")]
    #[rhai_type_readonly]
    pub(crate) baz: String,
    #[rhai_type_set(Self::set_qux)]
    pub qux: Vec<INT>,
}

impl Foo {
    pub fn set_qux(&mut self, value: Vec<INT>) {
        self.qux = value;
    }

    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder.with_fn("new_foo", || Self::default());
    }
}

fn get_bar(_this: &Foo) -> INT {
    42
}

#[test]
fn test() {
    let mut engine = Engine::new();
    engine.build_type::<Foo>().build_type::<Bar>();

    engine.run("new_foo()").unwrap();
}
