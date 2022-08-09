use core::marker::PhantomData;

use crate::{
    func::SendSync, types::dynamic::Variant, Engine, Identifier, RegisterNativeFunction,
    RhaiResultOf,
};

/// Trait to build a custom type for use with the [`Engine`].
/// i.e. register the type, getters, setters, methods, etc...
///
/// # Example
///
/// ```
/// use rhai::{CustomType, TypeBuilder, Engine};
///
/// #[derive(Debug, Clone, Eq, PartialEq)]
/// struct TestStruct {
///     field: i64
/// }
///
/// impl TestStruct {
///     fn new() -> Self {
///         Self { field: 1 }
///     }
///     fn update(&mut self, offset: i64) {
///         self.field += offset;
///     }
///     fn get_value(&mut self) -> i64 {
///         self.field
///     }
///     fn set_value(&mut self, value: i64) {
///        self.field = value;
///     }
/// }
///
/// impl CustomType for TestStruct {
///     fn build(mut builder: TypeBuilder<Self>) {
///         builder
///             .with_name("TestStruct")
///             .with_fn("new_ts", Self::new)
///             .with_fn("update", Self::update)
///             .with_get_set("value", Self::get_value, Self::set_value);
///     }
/// }
///
/// # fn main() -> Result<(), Box<rhai::EvalAltResult>> {
///
/// let mut engine = Engine::new();
///
/// // Register API for the custom type.
/// engine.build_type::<TestStruct>();
///
///
/// # #[cfg(not(feature = "no_object"))]
/// assert_eq!(
///     engine.eval::<TestStruct>("let x = new_ts(); x.update(41); x")?,
///     TestStruct { field: 42 }
/// );
///
/// # #[cfg(not(feature = "no_object"))]
/// assert_eq!(
///     engine.eval::<TestStruct>("let x = new_ts(); x.value = 5 + x.value; x")?,
///     TestStruct { field: 6 }
/// );
/// # Ok(())
/// # }
/// ```
pub trait CustomType: Variant + Clone {
    /// Builds the custom type for use with the [`Engine`].
    /// i.e. register the type, getters, setters, methods, etc...
    fn build(builder: TypeBuilder<Self>);
}

impl Engine {
    /// Build a custom type for use with the [`Engine`].
    /// i.e. register the type, getters, setters, methods, etc...
    ///
    /// See [`CustomType`].
    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    pub fn build_type<T>(&mut self) -> &mut Self
    where
        T: CustomType,
    {
        T::build(TypeBuilder::new(self));
        self
    }
}

#[allow(missing_docs)] // TODO: add docs
pub struct TypeBuilder<'a, T>
where
    T: Variant + Clone,
{
    engine: &'a mut Engine,
    name: Option<&'static str>,
    _marker: PhantomData<T>,
}

#[allow(missing_docs)] // TODO: add docs
impl<'a, T> TypeBuilder<'a, T>
where
    T: Variant + Clone,
{
    fn new(engine: &'a mut Engine) -> Self {
        Self {
            engine,
            name: None,
            _marker: PhantomData::default(),
        }
    }

    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    pub fn with_name(&mut self, name: &'static str) -> &mut Self {
        self.name = Some(name);
        self
    }

    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    pub fn with_fn<N, A, F>(&mut self, name: N, method: F) -> &mut Self
    where
        N: AsRef<str> + Into<Identifier>,
        F: RegisterNativeFunction<A, ()>,
    {
        self.engine.register_fn(name, method);
        self
    }

    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    pub fn with_result_fn<N, A, F, R>(&mut self, name: N, method: F) -> &mut Self
    where
        N: AsRef<str> + Into<Identifier>,
        F: RegisterNativeFunction<A, RhaiResultOf<R>>,
    {
        self.engine.register_result_fn(name, method);
        self
    }

    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    pub fn with_get<V: Variant + Clone>(
        &mut self,
        name: impl AsRef<str>,
        get_fn: impl Fn(&mut T) -> V + SendSync + 'static,
    ) -> &mut Self {
        self.engine.register_get(name, get_fn);
        self
    }

    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    pub fn with_get_result<V: Variant + Clone>(
        &mut self,
        name: impl AsRef<str>,
        get_fn: impl Fn(&mut T) -> RhaiResultOf<V> + SendSync + 'static,
    ) -> &mut Self {
        self.engine.register_get_result(name, get_fn);
        self
    }

    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    pub fn with_set<V: Variant + Clone>(
        &mut self,
        name: impl AsRef<str>,
        set_fn: impl Fn(&mut T, V) + SendSync + 'static,
    ) -> &mut Self {
        self.engine.register_set(name, set_fn);
        self
    }

    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    pub fn with_set_result<V: Variant + Clone>(
        &mut self,
        name: impl AsRef<str>,
        set_fn: impl Fn(&mut T, V) -> RhaiResultOf<()> + SendSync + 'static,
    ) -> &mut Self {
        self.engine.register_set_result(name, set_fn);
        self
    }

    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    pub fn with_get_set<V: Variant + Clone>(
        &mut self,
        name: impl AsRef<str>,
        get_fn: impl Fn(&mut T) -> V + SendSync + 'static,
        set_fn: impl Fn(&mut T, V) + SendSync + 'static,
    ) -> &mut Self {
        self.engine.register_get_set(name, get_fn, set_fn);
        self
    }

    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    pub fn with_indexer_get<X: Variant + Clone, V: Variant + Clone>(
        &mut self,
        get_fn: impl Fn(&mut T, X) -> V + SendSync + 'static,
    ) -> &mut Self {
        self.engine.register_indexer_get(get_fn);
        self
    }

    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    pub fn with_indexer_get_result<X: Variant + Clone, V: Variant + Clone>(
        &mut self,
        get_fn: impl Fn(&mut T, X) -> RhaiResultOf<V> + SendSync + 'static,
    ) -> &mut Self {
        self.engine.register_indexer_get_result(get_fn);
        self
    }

    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    pub fn with_indexer_set<X: Variant + Clone, V: Variant + Clone>(
        &mut self,
        set_fn: impl Fn(&mut T, X, V) + SendSync + 'static,
    ) -> &mut Self {
        self.engine.register_indexer_set(set_fn);
        self
    }

    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    pub fn with_indexer_set_result<X: Variant + Clone, V: Variant + Clone>(
        &mut self,
        set_fn: impl Fn(&mut T, X, V) -> RhaiResultOf<()> + SendSync + 'static,
    ) -> &mut Self {
        self.engine.register_indexer_set_result(set_fn);
        self
    }

    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    pub fn with_indexer_get_set<X: Variant + Clone, V: Variant + Clone>(
        &mut self,
        get_fn: impl Fn(&mut T, X) -> V + SendSync + 'static,
        set_fn: impl Fn(&mut T, X, V) + SendSync + 'static,
    ) -> &mut Self {
        self.engine.register_indexer_get_set(get_fn, set_fn);
        self
    }
}

impl<'a, T> Drop for TypeBuilder<'a, T>
where
    T: Variant + Clone,
{
    fn drop(&mut self) {
        if let Some(name) = self.name {
            self.engine.register_type_with_name::<T>(name);
        } else {
            self.engine.register_type::<T>();
        }
    }
}
