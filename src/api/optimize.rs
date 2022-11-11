//! Module that defines the script optimization API of [`Engine`].
#![cfg(not(feature = "no_optimize"))]

use crate::{Engine, OptimizationLevel, Scope, AST};

impl Engine {
    /// Control whether and how the [`Engine`] will optimize an [`AST`] after compilation.
    ///
    /// Not available under `no_optimize`.
    #[inline(always)]
    pub fn set_optimization_level(&mut self, optimization_level: OptimizationLevel) -> &mut Self {
        self.optimization_level = optimization_level;
        self
    }

    /// The current optimization level.
    /// It controls whether and how the [`Engine`] will optimize an [`AST`] after compilation.
    ///
    /// Not available under `no_optimize`.
    #[inline(always)]
    #[must_use]
    pub const fn optimization_level(&self) -> OptimizationLevel {
        self.optimization_level
    }

    /// Optimize the [`AST`] with constants defined in an external Scope.
    /// An optimized copy of the [`AST`] is returned while the original [`AST`] is consumed.
    ///
    /// Not available under `no_optimize`.
    ///
    /// Although optimization is performed by default during compilation, sometimes it is necessary
    /// to _re_-optimize an [`AST`].
    ///
    /// For example, when working with constants that are passed in via an external scope,
    /// it will be more efficient to optimize the [`AST`] once again to take advantage of the new constants.
    ///
    /// With this method, it is no longer necessary to recompile a large script.
    /// The script [`AST`] can be compiled just once.
    ///
    /// Before evaluation, constants are passed into the [`Engine`] via an external scope
    /// (i.e. with [`Scope::push_constant`][Scope::push_constant]).
    ///
    /// Then, the [`AST`] is cloned and the copy re-optimized before running.
    #[inline]
    #[must_use]
    pub fn optimize_ast(
        &self,
        scope: &Scope,
        ast: AST,
        optimization_level: OptimizationLevel,
    ) -> AST {
        let mut ast = ast;

        #[cfg(not(feature = "no_function"))]
        let functions = ast
            .shared_lib()
            .iter_fn()
            .filter(|f| f.func.is_script())
            .map(|f| f.func.get_script_fn_def().unwrap().clone())
            .collect();

        let mut _new_ast = crate::optimizer::optimize_into_ast(
            self,
            scope,
            ast.take_statements(),
            #[cfg(not(feature = "no_function"))]
            functions,
            optimization_level,
        );

        #[cfg(feature = "metadata")]
        _new_ast.set_doc(std::mem::take(ast.doc_mut()));

        _new_ast
    }
}
