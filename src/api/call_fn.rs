//! Module that defines the `call_fn` API of [`Engine`].
#![cfg(not(feature = "no_function"))]

use crate::eval::{Caches, GlobalRuntimeState};
use crate::types::dynamic::Variant;
use crate::types::RestoreOnDrop;
use crate::{
    reify, Dynamic, Engine, FuncArgs, Position, RhaiResult, RhaiResultOf, Scope, Shared, StaticVec,
    AST, ERR,
};
use std::any::{type_name, TypeId};
#[cfg(feature = "no_std")]
use std::prelude::v1::*;

impl Engine {
    /// Call a script function defined in an [`AST`] with multiple arguments.
    ///
    /// Not available under `no_function`.
    ///
    /// The [`AST`] is evaluated before calling the function.
    /// This allows a script to load the necessary modules.
    /// This is usually desired. If not, a specialized [`AST`] can be prepared that contains only
    /// function definitions without any body script via [`AST::clear_statements`].
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), Box<rhai::EvalAltResult>> {
    /// # #[cfg(not(feature = "no_function"))]
    /// # {
    /// use rhai::{Engine, Scope};
    ///
    /// let engine = Engine::new();
    ///
    /// let ast = engine.compile("
    ///     fn add(x, y) { len(x) + y + foo }
    ///     fn add1(x)   { len(x) + 1 + foo }
    ///     fn bar()     { foo/2 }
    /// ")?;
    ///
    /// let mut scope = Scope::new();
    /// scope.push("foo", 42_i64);
    ///
    /// // Call the script-defined function
    /// let result = engine.call_fn::<i64>(&mut scope, &ast, "add", ( "abc", 123_i64 ) )?;
    /// assert_eq!(result, 168);
    ///
    /// let result = engine.call_fn::<i64>(&mut scope, &ast, "add1", ( "abc", ) )?;
    /// //                                                           ^^^^^^^^^^ tuple of one
    /// assert_eq!(result, 46);
    ///
    /// let result = engine.call_fn::<i64>(&mut scope, &ast, "bar", () )?;
    /// assert_eq!(result, 21);
    /// # }
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn call_fn<T: Variant + Clone>(
        &self,
        scope: &mut Scope,
        ast: &AST,
        name: impl AsRef<str>,
        args: impl FuncArgs,
    ) -> RhaiResultOf<T> {
        let mut arg_values = StaticVec::new_const();
        args.parse(&mut arg_values);

        let result = self.call_fn_raw(scope, ast, true, true, name, None, arg_values)?;

        // Bail out early if the return type needs no cast
        if TypeId::of::<T>() == TypeId::of::<Dynamic>() {
            return Ok(reify!(result => T));
        }
        if TypeId::of::<T>() == TypeId::of::<()>() {
            return Ok(reify!(() => T));
        }

        // Cast return type
        let typ = self.map_type_name(result.type_name());

        result.try_cast().ok_or_else(|| {
            let t = self.map_type_name(type_name::<T>()).into();
            ERR::ErrorMismatchOutputType(t, typ.into(), Position::NONE).into()
        })
    }
    /// Call a script function defined in an [`AST`] with multiple [`Dynamic`] arguments.
    ///
    /// The following options are available:
    ///
    /// * whether to evaluate the [`AST`] to load necessary modules before calling the function
    /// * whether to rewind the [`Scope`] after the function call
    /// * a value for binding to the `this` pointer (if any)
    ///
    /// Not available under `no_function`.
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
    /// Do not use the arguments after this call. If they are needed afterwards, clone them _before_
    /// calling this function.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), Box<rhai::EvalAltResult>> {
    /// # #[cfg(not(feature = "no_function"))]
    /// # {
    /// use rhai::{Engine, Scope, Dynamic};
    ///
    /// let engine = Engine::new();
    ///
    /// let ast = engine.compile("
    ///     fn add(x, y) { len(x) + y + foo }
    ///     fn add1(x)   { len(x) + 1 + foo }
    ///     fn bar()     { foo/2 }
    ///     fn action(x) { this += x; }         // function using 'this' pointer
    ///     fn decl(x)   { let hello = x; }     // declaring variables
    /// ")?;
    ///
    /// let mut scope = Scope::new();
    /// scope.push("foo", 42_i64);
    ///
    /// // Call the script-defined function
    /// let result = engine.call_fn_raw(&mut scope, &ast, true, true, "add", None, [ "abc".into(), 123_i64.into() ])?;
    /// //                                                                   ^^^^ no 'this' pointer
    /// assert_eq!(result.cast::<i64>(), 168);
    ///
    /// let result = engine.call_fn_raw(&mut scope, &ast, true, true, "add1", None, [ "abc".into() ])?;
    /// assert_eq!(result.cast::<i64>(), 46);
    ///
    /// let result = engine.call_fn_raw(&mut scope, &ast, true, true, "bar", None, [])?;
    /// assert_eq!(result.cast::<i64>(), 21);
    ///
    /// let mut value = 1_i64.into();
    /// let result = engine.call_fn_raw(&mut scope, &ast, true, true, "action", Some(&mut value), [ 41_i64.into() ])?;
    /// //                                                                      ^^^^^^^^^^^^^^^^ binding the 'this' pointer
    /// assert_eq!(value.as_int().unwrap(), 42);
    ///
    /// engine.call_fn_raw(&mut scope, &ast, true, false, "decl", None, [ 42_i64.into() ])?;
    /// //                                         ^^^^^ do not rewind scope
    /// assert_eq!(scope.get_value::<i64>("hello").unwrap(), 42);
    /// # }
    /// # Ok(())
    /// # }
    /// ```
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

        self._call_fn(
            scope,
            &mut GlobalRuntimeState::new(self),
            &mut Caches::new(),
            ast,
            eval_ast,
            rewind_scope,
            name.as_ref(),
            this_ptr,
            arg_values.as_mut(),
        )
    }
    /// _(internals)_ Call a script function defined in an [`AST`] with multiple [`Dynamic`] arguments.
    /// Exported under the `internals` feature only.
    ///
    /// The following options are available:
    ///
    /// * whether to evaluate the [`AST`] to load necessary modules before calling the function
    /// * whether to rewind the [`Scope`] after the function call
    /// * a value for binding to the `this` pointer (if any)
    ///
    /// Not available under `no_function`.
    ///
    /// # WARNING - Unstable API
    ///
    /// This API is volatile and may change in the future.
    ///
    /// # WARNING - Low Level API
    ///
    /// This function is _extremely_ low level.
    ///
    /// A [`GlobalRuntimeState`] and [`Caches`] need to be passed into the function, which can be
    /// created via [`GlobalRuntimeState::new`] and [`Caches::new`].
    ///
    /// This makes repeatedly calling particular functions more efficient as the functions
    /// resolution cache is kept intact.
    ///
    /// # Arguments
    ///
    /// All the arguments are _consumed_, meaning that they're replaced by `()`. This is to avoid
    /// unnecessarily cloning the arguments.
    ///
    /// Do not use the arguments after this call. If they are needed afterwards, clone them _before_
    /// calling this function.
    #[cfg(feature = "internals")]
    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    #[inline(always)]
    pub fn call_fn_raw_raw(
        &self,
        scope: &mut Scope,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        ast: &AST,
        eval_ast: bool,
        rewind_scope: bool,
        name: &str,
        this_ptr: Option<&mut Dynamic>,
        arg_values: &mut [Dynamic],
    ) -> RhaiResult {
        self._call_fn(
            scope,
            global,
            caches,
            ast,
            eval_ast,
            rewind_scope,
            name,
            this_ptr,
            arg_values,
        )
    }

    /// Call a script function defined in an [`AST`] with multiple [`Dynamic`] arguments.
    fn _call_fn(
        &self,
        scope: &mut Scope,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        ast: &AST,
        eval_ast: bool,
        rewind_scope: bool,
        name: &str,
        this_ptr: Option<&mut Dynamic>,
        arg_values: &mut [Dynamic],
    ) -> RhaiResult {
        let statements = ast.statements();
        let lib = &[AsRef::<Shared<_>>::as_ref(ast).clone()];
        let mut this_ptr = this_ptr;

        let orig_scope_len = scope.len();

        #[cfg(not(feature = "no_module"))]
        let orig_embedded_module_resolver = std::mem::replace(
            &mut global.embedded_module_resolver,
            ast.resolver().cloned(),
        );
        #[cfg(not(feature = "no_module"))]
        let global = &mut *RestoreOnDrop::new(global, move |g| {
            g.embedded_module_resolver = orig_embedded_module_resolver
        });

        let result = if eval_ast && !statements.is_empty() {
            let r = self.eval_global_statements(global, caches, lib, 0, scope, statements);

            if rewind_scope {
                scope.rewind(orig_scope_len);
            }

            r
        } else {
            Ok(Dynamic::UNIT)
        }
        .and_then(|_| {
            let mut args: StaticVec<_> = arg_values.iter_mut().collect();

            // Check for data race.
            #[cfg(not(feature = "no_closure"))]
            crate::func::ensure_no_data_race(name, &args, false).map(|_| Dynamic::UNIT)?;

            if let Some(fn_def) = ast.shared_lib().get_script_fn(name, args.len()) {
                self.call_script_fn(
                    global,
                    caches,
                    lib,
                    0,
                    scope,
                    &mut this_ptr,
                    fn_def,
                    &mut args,
                    rewind_scope,
                    Position::NONE,
                )
            } else {
                Err(ERR::ErrorFunctionNotFound(name.into(), Position::NONE).into())
            }
        })?;

        #[cfg(feature = "debugging")]
        if self.debugger.is_some() {
            global.debugger.status = crate::eval::DebuggerStatus::Terminate;
            let node = &crate::ast::Stmt::Noop(Position::NONE);
            self.run_debugger(global, caches, lib, 0, scope, &mut this_ptr, node)?;
        }

        Ok(result)
    }
}
