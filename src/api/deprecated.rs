//! Module containing all deprecated API that will be removed in the next major version.

use crate::func::RegisterNativeFunction;
use crate::types::dynamic::Variant;
use crate::{
    Dynamic, Engine, EvalAltResult, FnPtr, Identifier, ImmutableString, Module, NativeCallContext,
    Position, RhaiResult, RhaiResultOf, Scope, SharedModule, TypeBuilder, AST,
};
#[cfg(feature = "no_std")]
use std::prelude::v1::*;

#[cfg(any(not(feature = "no_index"), not(feature = "no_object")))]
use crate::func::register::Mut;

#[cfg(not(feature = "no_std"))]
#[cfg(not(target_family = "wasm"))]
use std::path::PathBuf;

impl Engine {
    /// Evaluate a file, but throw away the result and only return error (if any).
    /// Useful for when you don't need the result, but still need to keep track of possible errors.
    ///
    /// Not available under `no_std` or `WASM`.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`run_file`][Engine::run_file] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.1.0", note = "use `run_file` instead")]
    #[cfg(not(feature = "no_std"))]
    #[cfg(not(target_family = "wasm"))]
    #[inline(always)]
    pub fn consume_file(&self, path: PathBuf) -> RhaiResultOf<()> {
        self.run_file(path)
    }

    /// Evaluate a file with own scope, but throw away the result and only return error (if any).
    /// Useful for when you don't need the result, but still need to keep track of possible errors.
    ///
    /// Not available under `no_std` or `WASM`.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`run_file_with_scope`][Engine::run_file_with_scope] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.1.0", note = "use `run_file_with_scope` instead")]
    #[cfg(not(feature = "no_std"))]
    #[cfg(not(target_family = "wasm"))]
    #[inline(always)]
    pub fn consume_file_with_scope(&self, scope: &mut Scope, path: PathBuf) -> RhaiResultOf<()> {
        self.run_file_with_scope(scope, path)
    }

    /// Evaluate a string, but throw away the result and only return error (if any).
    /// Useful for when you don't need the result, but still need to keep track of possible errors.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`run`][Engine::run] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.1.0", note = "use `run` instead")]
    #[inline(always)]
    pub fn consume(&self, script: &str) -> RhaiResultOf<()> {
        self.run(script)
    }

    /// Evaluate a string with own scope, but throw away the result and only return error (if any).
    /// Useful for when you don't need the result, but still need to keep track of possible errors.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`run_with_scope`][Engine::run_with_scope] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.1.0", note = "use `run_with_scope` instead")]
    #[inline(always)]
    pub fn consume_with_scope(&self, scope: &mut Scope, script: &str) -> RhaiResultOf<()> {
        self.run_with_scope(scope, script)
    }

    /// Evaluate an [`AST`], but throw away the result and only return error (if any).
    /// Useful for when you don't need the result, but still need to keep track of possible errors.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`run_ast`][Engine::run_ast] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.1.0", note = "use `run_ast` instead")]
    #[inline(always)]
    pub fn consume_ast(&self, ast: &AST) -> RhaiResultOf<()> {
        self.run_ast(ast)
    }

    /// Evaluate an [`AST`] with own scope, but throw away the result and only return error (if any).
    /// Useful for when you don't need the result, but still need to keep track of possible errors.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`run_ast_with_scope`][Engine::run_ast_with_scope] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.1.0", note = "use `run_ast_with_scope` instead")]
    #[inline(always)]
    pub fn consume_ast_with_scope(&self, scope: &mut Scope, ast: &AST) -> RhaiResultOf<()> {
        self.run_ast_with_scope(scope, ast)
    }
    /// Call a script function defined in an [`AST`] with multiple [`Dynamic`] arguments
    /// and optionally a value for binding to the `this` pointer.
    ///
    /// Not available under `no_function`.
    ///
    /// There is an option to evaluate the [`AST`] to load necessary modules before calling the function.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`call_fn_with_options`][Engine::call_fn_with_options] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.1.0", note = "use `call_fn_with_options` instead")]
    #[cfg(not(feature = "no_function"))]
    #[inline(always)]
    pub fn call_fn_dynamic(
        &self,
        scope: &mut Scope,
        ast: &AST,
        eval_ast: bool,
        name: impl AsRef<str>,
        this_ptr: Option<&mut Dynamic>,
        arg_values: impl AsMut<[Dynamic]>,
    ) -> RhaiResult {
        #[allow(deprecated)]
        self.call_fn_raw(scope, ast, eval_ast, true, name, this_ptr, arg_values)
    }
    /// Call a script function defined in an [`AST`] with multiple [`Dynamic`] arguments.
    ///
    /// Not available under `no_function`.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`call_fn_with_options`][Engine::call_fn_with_options] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.12.0", note = "use `call_fn_with_options` instead")]
    #[cfg(not(feature = "no_function"))]
    #[inline(always)]
    pub fn call_fn_raw(
        &self,
        scope: &mut Scope,
        ast: &AST,
        eval_ast: bool,
        rewind_scope: bool,
        name: impl AsRef<str>,
        this_ptr: Option<&mut Dynamic>,
        arg_values: impl AsMut<[Dynamic]>,
    ) -> RhaiResult {
        let mut arg_values = arg_values;

        let options = crate::CallFnOptions {
            this_ptr,
            eval_ast,
            rewind_scope,
            ..Default::default()
        };

        self._call_fn(
            options,
            scope,
            &mut crate::eval::GlobalRuntimeState::new(self),
            &mut crate::eval::Caches::new(),
            ast,
            name.as_ref(),
            arg_values.as_mut(),
        )
    }
    /// Register a custom fallible function with the [`Engine`].
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`register_fn`][Engine::register_fn] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.9.1", note = "use `register_fn` instead")]
    #[inline(always)]
    pub fn register_result_fn<A: 'static, const N: usize, const C: bool, R: Variant + Clone>(
        &mut self,
        name: impl AsRef<str> + Into<Identifier>,
        func: impl RegisterNativeFunction<A, N, C, R, true>,
    ) -> &mut Self {
        self.register_fn(name, func)
    }
    /// Register a getter function for a member of a registered type with the [`Engine`].
    ///
    /// The function signature must start with `&mut self` and not `&self`.
    ///
    /// Not available under `no_object`.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`register_get`][Engine::register_get] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.9.1", note = "use `register_get` instead")]
    #[cfg(not(feature = "no_object"))]
    #[inline(always)]
    pub fn register_get_result<T: Variant + Clone, const C: bool, V: Variant + Clone>(
        &mut self,
        name: impl AsRef<str>,
        get_fn: impl RegisterNativeFunction<(Mut<T>,), 1, C, V, true> + crate::func::SendSync + 'static,
    ) -> &mut Self {
        self.register_get(name, get_fn)
    }
    /// Register a setter function for a member of a registered type with the [`Engine`].
    ///
    /// Not available under `no_object`.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`register_set`][Engine::register_set] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.9.1", note = "use `register_set` instead")]
    #[cfg(not(feature = "no_object"))]
    #[inline(always)]
    pub fn register_set_result<T: Variant + Clone, V: Variant + Clone, const C: bool, S>(
        &mut self,
        name: impl AsRef<str>,
        set_fn: impl RegisterNativeFunction<(Mut<T>, V), 2, C, (), true>
            + crate::func::SendSync
            + 'static,
    ) -> &mut Self {
        self.register_set(name, set_fn)
    }
    /// Register an index getter for a custom type with the [`Engine`].
    ///
    /// The function signature must start with `&mut self` and not `&self`.
    ///
    /// Not available under both `no_index` and `no_object`.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`register_indexer_get`][Engine::register_indexer_get] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.9.1", note = "use `register_indexer_get` instead")]
    #[cfg(any(not(feature = "no_index"), not(feature = "no_object")))]
    #[inline(always)]
    pub fn register_indexer_get_result<
        T: Variant + Clone,
        X: Variant + Clone,
        V: Variant + Clone,
        const C: bool,
    >(
        &mut self,
        get_fn: impl RegisterNativeFunction<(Mut<T>, X), 2, C, V, true>
            + crate::func::SendSync
            + 'static,
    ) -> &mut Self {
        self.register_indexer_get(get_fn)
    }
    /// Register an index setter for a custom type with the [`Engine`].
    ///
    /// Not available under both `no_index` and `no_object`.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`register_indexer_set`][Engine::register_indexer_set] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.9.1", note = "use `register_indexer_set` instead")]
    #[cfg(any(not(feature = "no_index"), not(feature = "no_object")))]
    #[inline(always)]
    pub fn register_indexer_set_result<
        T: Variant + Clone,
        X: Variant + Clone,
        V: Variant + Clone,
        const C: bool,
    >(
        &mut self,
        set_fn: impl RegisterNativeFunction<(Mut<T>, X, V), 3, C, (), true>
            + crate::func::SendSync
            + 'static,
    ) -> &mut Self {
        self.register_indexer_set(set_fn)
    }
    /// Register a custom syntax with the [`Engine`].
    ///
    /// Not available under `no_custom_syntax`.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated.
    /// Use [`register_custom_syntax_with_state_raw`][Engine::register_custom_syntax_with_state_raw] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(
        since = "1.11.0",
        note = "use `register_custom_syntax_with_state_raw` instead"
    )]
    #[inline(always)]
    #[cfg(not(feature = "no_custom_syntax"))]
    pub fn register_custom_syntax_raw(
        &mut self,
        key: impl Into<Identifier>,
        parse: impl Fn(&[ImmutableString], &str) -> crate::parser::ParseResult<Option<ImmutableString>>
            + crate::func::SendSync
            + 'static,
        scope_may_be_changed: bool,
        func: impl Fn(&mut crate::EvalContext, &[crate::Expression]) -> RhaiResult
            + crate::func::SendSync
            + 'static,
    ) -> &mut Self {
        self.register_custom_syntax_with_state_raw(
            key,
            move |keywords, look_ahead, _| parse(keywords, look_ahead),
            scope_may_be_changed,
            move |context, expressions, _| func(context, expressions),
        )
    }
    /// _(internals)_ Evaluate a list of statements with no `this` pointer.
    /// Exported under the `internals` feature only.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. It will be removed in the next major version.
    #[cfg(feature = "internals")]
    #[inline(always)]
    #[deprecated(since = "1.12.0")]
    pub fn eval_statements_raw(
        &self,
        global: &mut crate::eval::GlobalRuntimeState,
        caches: &mut crate::eval::Caches,
        scope: &mut Scope,
        statements: &[crate::ast::Stmt],
    ) -> RhaiResult {
        self.eval_global_statements(global, caches, scope, statements)
    }
}

impl Dynamic {
    /// Convert the [`Dynamic`] into a [`String`] and return it.
    /// If there are other references to the same string, a cloned copy is returned.
    /// Returns the name of the actual type if the cast fails.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`into_string`][Dynamic::into_string] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.1.0", note = "use `into_string` instead")]
    #[inline(always)]
    pub fn as_string(self) -> Result<String, &'static str> {
        self.into_string()
    }

    /// Convert the [`Dynamic`] into an [`ImmutableString`] and return it.
    /// Returns the name of the actual type if the cast fails.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`into_immutable_string`][Dynamic::into_immutable_string] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.1.0", note = "use `into_immutable_string` instead")]
    #[inline(always)]
    pub fn as_immutable_string(self) -> Result<ImmutableString, &'static str> {
        self.into_immutable_string()
    }
}

impl NativeCallContext<'_> {
    /// Create a new [`NativeCallContext`].
    ///
    /// # Unimplemented
    ///
    /// This method is deprecated. It is no longer implemented and always panics.
    ///
    /// Use [`FnPtr::call`] to call a function pointer directly.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(
        since = "1.3.0",
        note = "use `FnPtr::call` to call a function pointer directly."
    )]
    #[inline(always)]
    #[must_use]
    #[allow(unused_variables)]
    pub fn new(engine: &Engine, fn_name: &str, lib: &[SharedModule]) -> Self {
        unimplemented!("`NativeCallContext::new` is deprecated");
    }

    /// Call a function inside the call context.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`call_fn_raw`][NativeCallContext::call_fn_raw] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.2.0", note = "use `call_fn_raw` instead")]
    #[inline(always)]
    pub fn call_fn_dynamic_raw(
        &self,
        fn_name: impl AsRef<str>,
        is_method_call: bool,
        args: &mut [&mut Dynamic],
    ) -> RhaiResult {
        self.call_fn_raw(fn_name.as_ref(), is_method_call, is_method_call, args)
    }
}

#[allow(useless_deprecated)]
#[deprecated(since = "1.2.0", note = "explicitly wrap `EvalAltResult` in `Err`")]
impl<T> From<EvalAltResult> for RhaiResultOf<T> {
    #[inline(always)]
    fn from(err: EvalAltResult) -> Self {
        Err(err.into())
    }
}

impl FnPtr {
    /// Get the number of curried arguments.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`curry().len()`][`FnPtr::curry`] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.8.0", note = "use `curry().len()` instead")]
    #[inline(always)]
    #[must_use]
    pub fn num_curried(&self) -> usize {
        self.curry().len()
    }
    /// Call the function pointer with curried arguments (if any).
    /// The function may be script-defined (not available under `no_function`) or native Rust.
    ///
    /// This method is intended for calling a function pointer that is passed into a native Rust
    /// function as an argument.  Therefore, the [`AST`] is _NOT_ evaluated before calling the
    /// function.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`call_within_context`][FnPtr::call_within_context] or
    /// [`call_raw`][FnPtr::call_raw] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(
        since = "1.3.0",
        note = "use `call_within_context` or `call_raw` instead"
    )]
    #[inline(always)]
    pub fn call_dynamic(
        &self,
        context: &NativeCallContext,
        this_ptr: Option<&mut Dynamic>,
        arg_values: impl AsMut<[Dynamic]>,
    ) -> RhaiResult {
        self.call_raw(context, this_ptr, arg_values)
    }
}

#[cfg(not(feature = "no_custom_syntax"))]
impl crate::Expression<'_> {
    /// If this expression is a variable name, return it.  Otherwise [`None`].
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use [`get_string_value`][crate::Expression::get_string_value] instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.4.0", note = "use `get_string_value` instead")]
    #[inline(always)]
    #[must_use]
    pub fn get_variable_name(&self) -> Option<&str> {
        self.get_string_value()
    }
}

impl Position {
    /// Create a new [`Position`].
    ///
    /// If `line` is zero, then [`None`] is returned.
    ///
    /// If `position` is zero, then it is at the beginning of a line.
    ///
    /// # Deprecated
    ///
    /// This function is deprecated. Use [`new`][Position::new] (which panics when `line` is zero) instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.6.0", note = "use `new` instead")]
    #[inline(always)]
    #[must_use]
    pub const fn new_const(line: u16, position: u16) -> Option<Self> {
        if line == 0 {
            None
        } else {
            Some(Self::new(line, position))
        }
    }
}

#[allow(deprecated)]
impl<'a, T: Variant + Clone> TypeBuilder<'a, T> {
    /// Register a custom fallible function.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use `with_fn` instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.9.1", note = "use `with_fn` instead")]
    #[inline(always)]
    pub fn with_result_fn<S, A: 'static, const N: usize, const C: bool, R, F>(
        &mut self,
        name: S,
        method: F,
    ) -> &mut Self
    where
        S: AsRef<str> + Into<Identifier>,
        R: Variant + Clone,
        F: RegisterNativeFunction<A, N, C, R, true>,
    {
        self.with_fn(name, method)
    }

    /// Register a fallible getter function.
    ///
    /// The function signature must start with `&mut self` and not `&self`.
    ///
    /// Not available under `no_object`.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use `with_get` instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.9.1", note = "use `with_get` instead")]
    #[cfg(not(feature = "no_object"))]
    #[inline(always)]
    pub fn with_get_result<V: Variant + Clone, const C: bool>(
        &mut self,
        name: impl AsRef<str>,
        get_fn: impl RegisterNativeFunction<(Mut<T>,), 1, C, V, true> + crate::func::SendSync + 'static,
    ) -> &mut Self {
        self.with_get(name, get_fn)
    }

    /// Register a fallible setter function.
    ///
    /// Not available under `no_object`.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use `with_set` instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.9.1", note = "use `with_set` instead")]
    #[cfg(not(feature = "no_object"))]
    #[inline(always)]
    pub fn with_set_result<V: Variant + Clone, const C: bool>(
        &mut self,
        name: impl AsRef<str>,
        set_fn: impl RegisterNativeFunction<(Mut<T>, V), 2, C, (), true>
            + crate::func::SendSync
            + 'static,
    ) -> &mut Self {
        self.with_set(name, set_fn)
    }

    /// Register an fallible index getter.
    ///
    /// The function signature must start with `&mut self` and not `&self`.
    ///
    /// Not available under both `no_index` and `no_object`.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use `with_indexer_get` instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.9.1", note = "use `with_indexer_get` instead")]
    #[cfg(any(not(feature = "no_index"), not(feature = "no_object")))]
    #[inline(always)]
    pub fn with_indexer_get_result<X: Variant + Clone, V: Variant + Clone, const C: bool>(
        &mut self,
        get_fn: impl RegisterNativeFunction<(Mut<T>, X), 2, C, V, true>
            + crate::func::SendSync
            + 'static,
    ) -> &mut Self {
        self.with_indexer_get(get_fn)
    }

    /// Register an fallible index setter.
    ///
    /// Not available under both `no_index` and `no_object`.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use `with_indexer_set` instead.
    ///
    /// This method will be removed in the next major version.
    #[deprecated(since = "1.9.1", note = "use `with_indexer_set` instead")]
    #[cfg(any(not(feature = "no_index"), not(feature = "no_object")))]
    #[inline(always)]
    pub fn with_indexer_set_result<X: Variant + Clone, V: Variant + Clone, const C: bool>(
        &mut self,
        set_fn: impl RegisterNativeFunction<(Mut<T>, X, V), 3, C, (), true>
            + crate::func::SendSync
            + 'static,
    ) -> &mut Self {
        self.with_indexer_set(set_fn)
    }
}

impl Module {
    /// Create a new [`Module`] with a pre-sized capacity for functions.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use `new` instead.
    ///
    /// This method will be removed in the next major version.
    #[inline(always)]
    #[must_use]
    #[deprecated(since = "1.12.0", note = "use `new` instead")]
    pub fn with_capacity(_capacity: usize) -> Self {
        Self::new()
    }
}
