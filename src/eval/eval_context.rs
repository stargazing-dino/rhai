//! Evaluation context.

use super::{Caches, GlobalRuntimeState};
use crate::{Dynamic, Engine, Module, Scope, SharedModule};
#[cfg(feature = "no_std")]
use std::prelude::v1::*;

/// Context of a script evaluation process.
#[derive(Debug)]
#[allow(dead_code)]
pub struct EvalContext<'a, 's, 'ps, 'g, 'c, 't> {
    /// The current [`Engine`].
    engine: &'a Engine,
    /// The current [`Scope`].
    scope: &'s mut Scope<'ps>,
    /// The current [`GlobalRuntimeState`].
    global: &'g mut GlobalRuntimeState,
    /// The current [caches][Caches], if available.
    caches: &'c mut Caches,
    /// The current bound `this` pointer, if any.
    this_ptr: &'t mut Dynamic,
}

impl<'a, 's, 'ps, 'g, 'c, 't> EvalContext<'a, 's, 'ps, 'g, 'c, 't> {
    /// Create a new [`EvalContext`].
    #[inline(always)]
    #[must_use]
    pub fn new(
        engine: &'a Engine,
        global: &'g mut GlobalRuntimeState,
        caches: &'c mut Caches,
        scope: &'s mut Scope<'ps>,
        this_ptr: &'t mut Dynamic,
    ) -> Self {
        Self {
            engine,
            scope,
            global,
            caches,
            this_ptr,
        }
    }
    /// The current [`Engine`].
    #[inline(always)]
    #[must_use]
    pub const fn engine(&self) -> &'a Engine {
        self.engine
    }
    /// The current source.
    #[inline(always)]
    #[must_use]
    pub fn source(&self) -> Option<&str> {
        self.global.source()
    }
    /// The current [`Scope`].
    #[inline(always)]
    #[must_use]
    pub const fn scope(&self) -> &Scope<'ps> {
        self.scope
    }
    /// Get a mutable reference to the current [`Scope`].
    #[inline(always)]
    #[must_use]
    pub fn scope_mut(&mut self) -> &mut &'s mut Scope<'ps> {
        &mut self.scope
    }
    /// Get an iterator over the current set of modules imported via `import` statements,
    /// in reverse order (i.e. modules imported last come first).
    #[cfg(not(feature = "no_module"))]
    #[inline(always)]
    pub fn iter_imports(&self) -> impl Iterator<Item = (&str, &Module)> {
        self.global.iter_imports()
    }
    /// Custom state kept in a [`Dynamic`].
    #[inline(always)]
    #[must_use]
    pub const fn tag(&self) -> &Dynamic {
        &self.global.tag
    }
    /// Mutable reference to the custom state kept in a [`Dynamic`].
    #[inline(always)]
    #[must_use]
    pub fn tag_mut(&mut self) -> &mut Dynamic {
        &mut self.global.tag
    }
    /// _(internals)_ The current [`GlobalRuntimeState`].
    /// Exported under the `internals` feature only.
    #[cfg(feature = "internals")]
    #[inline(always)]
    #[must_use]
    pub const fn global_runtime_state(&self) -> &GlobalRuntimeState {
        self.global
    }
    /// _(internals)_ Get a mutable reference to the current [`GlobalRuntimeState`].
    /// Exported under the `internals` feature only.
    #[cfg(feature = "internals")]
    #[inline(always)]
    #[must_use]
    pub fn global_runtime_state_mut(&mut self) -> &mut GlobalRuntimeState {
        self.global
    }
    /// Get an iterator over the namespaces containing definition of all script-defined functions.
    #[inline]
    pub fn iter_namespaces(&self) -> impl Iterator<Item = &Module> {
        self.global.lib.iter().map(|m| m.as_ref())
    }
    /// _(internals)_ The current set of namespaces containing definitions of all script-defined functions.
    /// Exported under the `internals` feature only.
    #[cfg(feature = "internals")]
    #[inline(always)]
    #[must_use]
    pub fn namespaces(&self) -> &[SharedModule] {
        &self.global.lib
    }
    /// The current bound `this` pointer, if any.
    #[inline]
    #[must_use]
    pub fn this_ptr(&self) -> Option<&Dynamic> {
        if self.this_ptr.is_null() {
            None
        } else {
            Some(self.this_ptr)
        }
    }
    /// Mutable reference to the current bound `this` pointer, if any.
    #[inline]
    #[must_use]
    pub fn this_ptr_mut(&mut self) -> Option<&mut Dynamic> {
        if self.this_ptr.is_null() {
            None
        } else {
            Some(self.this_ptr)
        }
    }
    /// The current nesting level of function calls.
    #[inline(always)]
    #[must_use]
    pub const fn call_level(&self) -> usize {
        self.global.level
    }

    /// Evaluate an [expression tree][crate::Expression] within this [evaluation context][`EvalContext`].
    ///
    /// # WARNING - Low Level API
    ///
    /// This function is very low level.  It evaluates an expression from an [`AST`][crate::AST].
    #[cfg(not(feature = "no_custom_syntax"))]
    #[inline(always)]
    pub fn eval_expression_tree(&mut self, expr: &crate::Expression) -> crate::RhaiResult {
        #[allow(deprecated)]
        self.eval_expression_tree_raw(expr, true)
    }
    /// Evaluate an [expression tree][crate::Expression] within this [evaluation context][`EvalContext`].
    ///
    /// The following option is available:
    ///
    /// * whether to rewind the [`Scope`] after evaluation if the expression is a [`StmtBlock`][crate::ast::StmtBlock]
    ///
    /// # WARNING - Unstable API
    ///
    /// This API is volatile and may change in the future.
    ///
    /// # WARNING - Low Level API
    ///
    /// This function is _extremely_ low level.  It evaluates an expression from an [`AST`][crate::AST].
    #[cfg(not(feature = "no_custom_syntax"))]
    #[deprecated = "This API is NOT deprecated, but it is considered volatile and may change in the future."]
    #[inline]
    pub fn eval_expression_tree_raw(
        &mut self,
        expr: &crate::Expression,
        rewind_scope: bool,
    ) -> crate::RhaiResult {
        let expr: &crate::ast::Expr = expr;

        match expr {
            crate::ast::Expr::Stmt(statements) => self.engine.eval_stmt_block(
                self.global,
                self.caches,
                self.scope,
                self.this_ptr,
                statements,
                rewind_scope,
            ),
            _ => self
                .engine
                .eval_expr(self.global, self.caches, self.scope, self.this_ptr, expr),
        }
    }
}
