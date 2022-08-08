use crate::Engine;

/// Trait to build a custom type for use with the [`Engine`].
/// i.e. register the type, getters, setters, methods, etc...
///
/// # Example
///
/// ```
/// use rhai::{Engine, CustomType};
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
/// }
///
/// impl CustomType for TestStruct {
///     fn build(engine: &mut Engine) {
///         engine
///             .register_type::<Self>()
///             .register_fn("new_ts", Self::new)
///             .register_fn("update", Self::update);
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
/// # #[cfg(not(feature = "no_object"))]
/// assert_eq!(
///     engine.eval::<TestStruct>("let x = new_ts(); x.update(41); x")?,
///     TestStruct { field: 42 }
/// );
/// # Ok(())
/// # }
/// ```
pub trait CustomType {
    /// Builds the custom type for use with the [`Engine`].
    /// i.e. register the type, getters, setters, methods, etc...
    fn build(engine: &mut Engine);
}

impl Engine {
    /// Build a custom type for use with the [`Engine`].
    /// i.e. register the type, getters, setters, methods, etc...
    ///
    /// See [`CustomType`].
    pub fn build_type<T>(&mut self) -> &mut Self
    where
        T: CustomType,
    {
        T::build(self);
        self
    }
}
