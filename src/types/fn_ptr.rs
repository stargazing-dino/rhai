//! The `FnPtr` type.

use crate::eval::GlobalRuntimeState;
use crate::tokenizer::is_valid_function_name;
use crate::types::dynamic::Variant;
use crate::{
    Dynamic, Engine, FuncArgs, ImmutableString, NativeCallContext, Position, RhaiError, RhaiResult,
    RhaiResultOf, StaticVec, AST, ERR,
};
#[cfg(feature = "no_std")]
use std::prelude::v1::*;
use std::{
    any::type_name,
    convert::{TryFrom, TryInto},
    fmt, mem,
};

/// A general function pointer, which may carry additional (i.e. curried) argument values
/// to be passed onto a function during a call.
#[derive(Clone, Hash)]
pub struct FnPtr {
    name: ImmutableString,
    curry: StaticVec<Dynamic>,
}

impl fmt::Debug for FnPtr {
    #[cold]
    #[inline(never)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_curried() {
            self.curry
                .iter()
                .fold(f.debug_tuple("Fn").field(&self.name), |f, curry| {
                    f.field(curry)
                })
                .finish()
        } else {
            write!(f, "Fn({})", self.fn_name())
        }
    }
}

impl FnPtr {
    /// Create a new function pointer.
    #[inline(always)]
    pub fn new(name: impl Into<ImmutableString>) -> RhaiResultOf<Self> {
        name.into().try_into()
    }
    /// Create a new function pointer without checking its parameters.
    #[inline(always)]
    #[must_use]
    pub(crate) fn new_unchecked(
        name: impl Into<ImmutableString>,
        curry: StaticVec<Dynamic>,
    ) -> Self {
        Self {
            name: name.into(),
            curry,
        }
    }
    /// Get the name of the function.
    #[inline(always)]
    #[must_use]
    pub fn fn_name(&self) -> &str {
        self.fn_name_raw().as_str()
    }
    /// Get the name of the function.
    #[inline(always)]
    #[must_use]
    pub(crate) const fn fn_name_raw(&self) -> &ImmutableString {
        &self.name
    }
    /// Get the underlying data of the function pointer.
    #[inline(always)]
    #[must_use]
    pub(crate) fn take_data(self) -> (ImmutableString, StaticVec<Dynamic>) {
        (self.name, self.curry)
    }
    /// Get the curried arguments.
    #[inline(always)]
    #[must_use]
    pub fn curry(&self) -> &[Dynamic] {
        self.curry.as_ref()
    }
    /// Add a new curried argument.
    #[inline(always)]
    pub fn add_curry(&mut self, value: Dynamic) -> &mut Self {
        self.curry.push(value);
        self
    }
    /// Set curried arguments to the function pointer.
    #[inline]
    pub fn set_curry(&mut self, values: impl IntoIterator<Item = Dynamic>) -> &mut Self {
        self.curry = values.into_iter().collect();
        self
    }
    /// Is the function pointer curried?
    #[inline(always)]
    #[must_use]
    pub fn is_curried(&self) -> bool {
        !self.curry.is_empty()
    }
    /// Does the function pointer refer to an anonymous function?
    ///
    /// Not available under `no_function`.
    #[cfg(not(feature = "no_function"))]
    #[inline(always)]
    #[must_use]
    pub fn is_anonymous(&self) -> bool {
        crate::func::is_anonymous_fn(&self.name)
    }
    /// Call the function pointer with curried arguments (if any).
    /// The function may be script-defined (not available under `no_function`) or native Rust.
    ///
    /// This method is intended for calling a function pointer directly, possibly on another [`Engine`].
    /// Therefore, the [`AST`] is _NOT_ evaluated before calling the function.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), Box<rhai::EvalAltResult>> {
    /// # #[cfg(not(feature = "no_function"))]
    /// # {
    /// use rhai::{Engine, FnPtr};
    ///
    /// let engine = Engine::new();
    ///
    /// let ast = engine.compile("fn foo(x, y) { len(x) + y }")?;
    ///
    /// let mut fn_ptr = FnPtr::new("foo")?;
    ///
    /// // Curry values into the function pointer
    /// fn_ptr.set_curry(vec!["abc".into()]);
    ///
    /// // Values are only needed for non-curried parameters
    /// let result: i64 = fn_ptr.call(&engine, &ast, ( 39_i64, ) )?;
    ///
    /// assert_eq!(result, 42);
    /// # }
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn call<T: Variant + Clone>(
        &self,
        engine: &Engine,
        ast: &AST,
        args: impl FuncArgs,
    ) -> RhaiResultOf<T> {
        let _ast = ast;
        let mut arg_values = crate::StaticVec::new_const();
        args.parse(&mut arg_values);

        let global = &mut GlobalRuntimeState::new(engine);

        #[cfg(not(feature = "no_function"))]
        if !_ast.functions().is_empty() {
            global.lib.push(_ast.functions().clone());
        }

        let ctx = (engine, self.fn_name(), None, &*global, Position::NONE).into();

        let result = self.call_raw(&ctx, None, arg_values)?;

        let typ = engine.map_type_name(result.type_name());

        result.try_cast().ok_or_else(|| {
            let t = engine.map_type_name(type_name::<T>()).into();
            ERR::ErrorMismatchOutputType(t, typ.into(), Position::NONE).into()
        })
    }
    /// Call the function pointer with curried arguments (if any).
    /// The function may be script-defined (not available under `no_function`) or native Rust.
    ///
    /// This method is intended for calling a function pointer that is passed into a native Rust
    /// function as an argument.  Therefore, the [`AST`] is _NOT_ evaluated before calling the
    /// function.
    #[inline]
    pub fn call_within_context<T: Variant + Clone>(
        &self,
        context: &NativeCallContext,
        args: impl FuncArgs,
    ) -> RhaiResultOf<T> {
        let mut arg_values = crate::StaticVec::new_const();
        args.parse(&mut arg_values);

        let result = self.call_raw(context, None, arg_values)?;

        let typ = context.engine().map_type_name(result.type_name());

        result.try_cast().ok_or_else(|| {
            let t = context.engine().map_type_name(type_name::<T>()).into();
            ERR::ErrorMismatchOutputType(t, typ.into(), Position::NONE).into()
        })
    }
    /// Call the function pointer with curried arguments (if any).
    /// The function may be script-defined (not available under `no_function`) or native Rust.
    ///
    /// This method is intended for calling a function pointer that is passed into a native Rust
    /// function as an argument.  Therefore, the [`AST`] is _NOT_ evaluated before calling the
    /// function.
    ///
    /// # WARNING - Low Level API
    ///
    /// This function is very low level.
    ///
    /// # Arguments
    ///
    /// All the arguments are _consumed_, meaning that they're replaced by `()`.
    /// This is to avoid unnecessarily cloning the arguments.
    ///
    /// Do not use the arguments after this call. If they are needed afterwards,
    /// clone them _before_ calling this function.
    #[inline]
    pub fn call_raw(
        &self,
        context: &NativeCallContext,
        this_ptr: Option<&mut Dynamic>,
        arg_values: impl AsMut<[Dynamic]>,
    ) -> RhaiResult {
        let mut arg_values = arg_values;
        let mut arg_values = arg_values.as_mut();
        let mut args_data;

        if self.is_curried() {
            args_data = StaticVec::with_capacity(self.curry().len() + arg_values.len());
            args_data.extend(self.curry().iter().cloned());
            args_data.extend(arg_values.iter_mut().map(mem::take));
            arg_values = &mut *args_data;
        };

        let is_method = this_ptr.is_some();

        let mut args = StaticVec::with_capacity(arg_values.len() + 1);
        if let Some(obj) = this_ptr {
            args.push(obj);
        }
        args.extend(arg_values.iter_mut());

        context.call_fn_raw(self.fn_name(), is_method, is_method, &mut args)
    }
}

impl fmt::Display for FnPtr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fn({})", self.fn_name())
    }
}

impl TryFrom<ImmutableString> for FnPtr {
    type Error = RhaiError;

    #[inline(always)]
    fn try_from(value: ImmutableString) -> RhaiResultOf<Self> {
        if is_valid_function_name(&value) {
            Ok(Self {
                name: value,
                curry: StaticVec::new_const(),
            })
        } else {
            Err(ERR::ErrorFunctionNotFound(value.to_string(), Position::NONE).into())
        }
    }
}
