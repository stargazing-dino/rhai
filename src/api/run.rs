//! Module that defines the public evaluation API of [`Engine`].

use crate::eval::{Caches, GlobalRuntimeState};
use crate::parser::ParseState;
use crate::{Engine, RhaiResultOf, Scope, AST};
#[cfg(feature = "no_std")]
use std::prelude::v1::*;

impl Engine {
    /// Evaluate a string as a script.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), Box<rhai::EvalAltResult>> {
    /// use rhai::Engine;
    ///
    /// let engine = Engine::new();
    ///
    /// engine.run("print(40 + 2);")?;
    /// # Ok(())
    /// # }
    /// ```
    #[inline(always)]
    pub fn run(&self, script: &str) -> RhaiResultOf<()> {
        self.run_with_scope(&mut Scope::new(), script)
    }
    /// Evaluate a string as a script with own scope.
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
    /// engine.run_with_scope(&mut scope, "x += 2; print(x);")?;
    ///
    /// // The variable in the scope is modified
    /// assert_eq!(scope.get_value::<i64>("x").expect("variable x should exist"), 42);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn run_with_scope(&self, scope: &mut Scope, script: &str) -> RhaiResultOf<()> {
        let scripts = [script];
        let (stream, tokenizer_control) =
            self.lex_raw(&scripts, self.token_mapper.as_ref().map(<_>::as_ref));
        let mut state = ParseState::new(self, scope, Default::default(), tokenizer_control);
        let ast = self.parse(&mut stream.peekable(), &mut state, self.optimization_level)?;
        self.run_ast_with_scope(scope, &ast)
    }
    /// Evaluate an [`AST`].
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
    /// let ast = engine.compile("print(40 + 2);")?;
    ///
    /// // Evaluate it
    /// engine.run_ast(&ast)?;
    /// # Ok(())
    /// # }
    /// ```
    #[inline(always)]
    pub fn run_ast(&self, ast: &AST) -> RhaiResultOf<()> {
        self.run_ast_with_scope(&mut Scope::new(), ast)
    }
    /// Evaluate an [`AST`] with own scope.
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
    /// engine.run_ast_with_scope(&mut scope, &ast)?;
    ///
    /// // The variable in the scope is modified
    /// assert_eq!(scope.get_value::<i64>("x").expect("variable x should exist"), 42);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn run_ast_with_scope(&self, scope: &mut Scope, ast: &AST) -> RhaiResultOf<()> {
        let caches = &mut Caches::new();
        let global = &mut GlobalRuntimeState::new(self);
        global.source = ast.source_raw().cloned();

        #[cfg(not(feature = "no_function"))]
        if !ast.functions().is_empty() {
            global.lib.push(ast.functions().clone());
        }
        #[cfg(not(feature = "no_module"))]
        {
            global.embedded_module_resolver = ast.resolver().cloned();
        }

        let statements = ast.statements();
        if !statements.is_empty() {
            self.eval_global_statements(global, caches, scope, statements)?;
        }

        #[cfg(feature = "debugging")]
        if self.debugger.is_some() {
            global.debugger.status = crate::eval::DebuggerStatus::Terminate;
            let mut this = crate::Dynamic::NULL;
            let node = &crate::ast::Stmt::Noop(crate::Position::NONE);
            self.run_debugger(global, caches, scope, &mut this, node)?;
        }

        Ok(())
    }
}

/// Evaluate a string as a script.
///
/// # Example
///
/// ```
/// # fn main() -> Result<(), Box<rhai::EvalAltResult>> {
/// rhai::run("print(40 + 2);")?;
/// # Ok(())
/// # }
/// ```
#[inline(always)]
pub fn run(script: &str) -> RhaiResultOf<()> {
    Engine::new().run(script)
}
