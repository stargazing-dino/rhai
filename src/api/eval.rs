//! Module that defines the public evaluation API of [`Engine`].

use crate::eval::{Caches, GlobalRuntimeState};
use crate::func::native::locked_write;
use crate::parser::ParseState;
use crate::types::dynamic::Variant;
use crate::{
    reify, Dynamic, Engine, OptimizationLevel, Position, RhaiResult, RhaiResultOf, Scope, AST, ERR,
};
#[cfg(feature = "no_std")]
use std::prelude::v1::*;
use std::{
    any::{type_name, TypeId},
    mem,
};

impl Engine {
    /// Evaluate a string as a script, returning the result value or an error.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), Box<rhai::EvalAltResult>> {
    /// use rhai::Engine;
    ///
    /// let engine = Engine::new();
    ///
    /// assert_eq!(engine.eval::<i64>("40 + 2")?, 42);
    /// # Ok(())
    /// # }
    /// ```
    #[inline(always)]
    pub fn eval<T: Variant + Clone>(&self, script: &str) -> RhaiResultOf<T> {
        self.eval_with_scope(&mut Scope::new(), script)
    }
    /// Evaluate a string as a script with own scope, returning the result value or an error.
    ///
    /// ## Constants Propagation
    ///
    /// If not [`OptimizationLevel::None`][crate::OptimizationLevel::None], constants defined within
    /// the scope are propagated throughout the script _including_ functions.
    ///
    /// This allows functions to be optimized based on dynamic global constants.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), Box<rhai::EvalAltResult>> {
    /// use rhai::{Engine, Scope};
    ///
    /// let engine = Engine::new();
    ///
    /// // Create initialized scope
    /// let mut scope = Scope::new();
    /// scope.push("x", 40_i64);
    ///
    /// assert_eq!(engine.eval_with_scope::<i64>(&mut scope, "x += 2; x")?, 42);
    /// assert_eq!(engine.eval_with_scope::<i64>(&mut scope, "x += 2; x")?, 44);
    ///
    /// // The variable in the scope is modified
    /// assert_eq!(scope.get_value::<i64>("x").expect("variable x should exist"), 44);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn eval_with_scope<T: Variant + Clone>(
        &self,
        scope: &mut Scope,
        script: &str,
    ) -> RhaiResultOf<T> {
        let ast = self.compile_with_scope_and_optimization_level(
            scope,
            [script],
            self.optimization_level,
        )?;
        self.eval_ast_with_scope(scope, &ast)
    }
    /// Evaluate a string containing an expression, returning the result value or an error.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), Box<rhai::EvalAltResult>> {
    /// use rhai::Engine;
    ///
    /// let engine = Engine::new();
    ///
    /// assert_eq!(engine.eval_expression::<i64>("40 + 2")?, 42);
    /// # Ok(())
    /// # }
    /// ```
    #[inline(always)]
    pub fn eval_expression<T: Variant + Clone>(&self, script: &str) -> RhaiResultOf<T> {
        self.eval_expression_with_scope(&mut Scope::new(), script)
    }
    /// Evaluate a string containing an expression with own scope, returning the result value or an error.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), Box<rhai::EvalAltResult>> {
    /// use rhai::{Engine, Scope};
    ///
    /// let engine = Engine::new();
    ///
    /// // Create initialized scope
    /// let mut scope = Scope::new();
    /// scope.push("x", 40_i64);
    ///
    /// assert_eq!(engine.eval_expression_with_scope::<i64>(&mut scope, "x + 2")?, 42);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn eval_expression_with_scope<T: Variant + Clone>(
        &self,
        scope: &mut Scope,
        script: &str,
    ) -> RhaiResultOf<T> {
        let scripts = [script];
        let ast = {
            let interned_strings = &mut *locked_write(&self.interned_strings);

            let (stream, tokenizer_control) =
                self.lex_raw(&scripts, self.token_mapper.as_ref().map(<_>::as_ref));

            let mut state = ParseState::new(scope, interned_strings, tokenizer_control);

            // No need to optimize a lone expression
            self.parse_global_expr(
                &mut stream.peekable(),
                &mut state,
                |_| {},
                #[cfg(not(feature = "no_optimize"))]
                OptimizationLevel::None,
                #[cfg(feature = "no_optimize")]
                OptimizationLevel::default(),
            )?
        };

        self.eval_ast_with_scope(scope, &ast)
    }
    /// Evaluate an [`AST`], returning the result value or an error.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), Box<rhai::EvalAltResult>> {
    /// use rhai::Engine;
    ///
    /// let engine = Engine::new();
    ///
    /// // Compile a script to an AST and store it for later evaluation
    /// let ast = engine.compile("40 + 2")?;
    ///
    /// // Evaluate it
    /// assert_eq!(engine.eval_ast::<i64>(&ast)?, 42);
    /// # Ok(())
    /// # }
    /// ```
    #[inline(always)]
    pub fn eval_ast<T: Variant + Clone>(&self, ast: &AST) -> RhaiResultOf<T> {
        self.eval_ast_with_scope(&mut Scope::new(), ast)
    }
    /// Evaluate an [`AST`] with own scope, returning the result value or an error.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), Box<rhai::EvalAltResult>> {
    /// use rhai::{Engine, Scope};
    ///
    /// let engine = Engine::new();
    ///
    /// // Create initialized scope
    /// let mut scope = Scope::new();
    /// scope.push("x", 40_i64);
    ///
    /// // Compile a script to an AST and store it for later evaluation
    /// let ast = engine.compile("x += 2; x")?;
    ///
    /// // Evaluate it
    /// assert_eq!(engine.eval_ast_with_scope::<i64>(&mut scope, &ast)?, 42);
    /// assert_eq!(engine.eval_ast_with_scope::<i64>(&mut scope, &ast)?, 44);
    ///
    /// // The variable in the scope is modified
    /// assert_eq!(scope.get_value::<i64>("x").expect("variable x should exist"), 44);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn eval_ast_with_scope<T: Variant + Clone>(
        &self,
        scope: &mut Scope,
        ast: &AST,
    ) -> RhaiResultOf<T> {
        let global = &mut GlobalRuntimeState::new(self);
        let caches = &mut Caches::new();

        let result = self.eval_ast_with_scope_raw(global, caches, scope, ast)?;

        // Bail out early if the return type needs no cast
        if TypeId::of::<T>() == TypeId::of::<Dynamic>() {
            return Ok(reify!(result => T));
        }

        let typ = self.map_type_name(result.type_name());

        result.try_cast::<T>().ok_or_else(|| {
            let t = self.map_type_name(type_name::<T>()).into();
            ERR::ErrorMismatchOutputType(t, typ.into(), Position::NONE).into()
        })
    }
    /// Evaluate an [`AST`] with own scope, returning the result value or an error.
    #[inline]
    pub(crate) fn eval_ast_with_scope_raw<'a>(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        scope: &mut Scope,
        ast: &'a AST,
    ) -> RhaiResult {
        let orig_source = mem::replace(&mut global.source, ast.source_raw().cloned());

        #[cfg(not(feature = "no_function"))]
        let orig_lib_len = global.lib.len();

        #[cfg(not(feature = "no_function"))]
        global.lib.push(ast.shared_lib().clone());

        #[cfg(not(feature = "no_module"))]
        let orig_embedded_module_resolver = mem::replace(
            &mut global.embedded_module_resolver,
            ast.resolver().cloned(),
        );

        let statements = ast.statements();

        if statements.is_empty() {
            return Ok(Dynamic::UNIT);
        }

        let result = self.eval_global_statements(global, caches, scope, statements);

        #[cfg(feature = "debugging")]
        if self.debugger.is_some() {
            global.debugger.status = crate::eval::DebuggerStatus::Terminate;
            let mut this = Dynamic::NULL;
            let node = &crate::ast::Stmt::Noop(Position::NONE);

            self.run_debugger(global, caches, scope, &mut this, node)?;
        }

        #[cfg(not(feature = "no_module"))]
        {
            global.embedded_module_resolver = orig_embedded_module_resolver;
        }

        #[cfg(not(feature = "no_function"))]
        global.lib.truncate(orig_lib_len);

        global.source = orig_source;

        result
    }
    /// _(internals)_ Evaluate a list of statements with no `this` pointer.
    /// Exported under the `internals` feature only.
    ///
    /// This is commonly used to evaluate a list of statements in an [`AST`] or a script function body.
    ///
    /// # WARNING - Low Level API
    ///
    /// This function is very low level.
    #[cfg(feature = "internals")]
    #[inline(always)]
    pub fn eval_statements_raw(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        scope: &mut Scope,
        statements: &[crate::ast::Stmt],
    ) -> RhaiResult {
        self.eval_global_statements(global, caches, scope, statements)
    }
}

/// Evaluate a string as a script, returning the result value or an error.
///
/// # Example
///
/// ```
/// # fn main() -> Result<(), Box<rhai::EvalAltResult>> {
/// let result: i64 = rhai::eval("40 + 2")?;
///
/// assert_eq!(result, 42);
/// # Ok(())
/// # }
/// ```
#[inline(always)]
pub fn eval<T: Variant + Clone>(script: &str) -> RhaiResultOf<T> {
    Engine::new().eval(script)
}
