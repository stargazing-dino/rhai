//! Module defining functions for evaluating an expression.

use super::{Caches, EvalContext, GlobalRuntimeState, Target};
use crate::ast::{Expr, OpAssignment};
use crate::engine::{KEYWORD_THIS, OP_CONCAT};
use crate::types::dynamic::AccessMode;
use crate::{Dynamic, Engine, Position, RhaiResult, RhaiResultOf, Scope, SharedModule, ERR};
use std::num::NonZeroUsize;
#[cfg(feature = "no_std")]
use std::prelude::v1::*;

impl Engine {
    /// Search for a module within an imports stack.
    #[cfg(not(feature = "no_module"))]
    #[inline]
    #[must_use]
    pub(crate) fn search_imports(
        &self,
        global: &GlobalRuntimeState,
        namespace: &crate::ast::Namespace,
    ) -> Option<SharedModule> {
        assert!(!namespace.is_empty());

        let root = namespace.root();

        let index = if global.always_search_scope {
            None
        } else {
            namespace.index()
        };

        // Qualified - check if the root module is directly indexed
        if let Some(index) = index {
            let offset = global.num_imports() - index.get();

            if let m @ Some(_) = global.get_shared_import(offset) {
                return m;
            }
        }

        // Do a text-match search if the index doesn't work
        global.find_import(root).map_or_else(
            || self.global_sub_modules.get(root).cloned(),
            |offset| global.get_shared_import(offset),
        )
    }

    /// Search for a variable within the scope or within imports,
    /// depending on whether the variable name is namespace-qualified.
    pub(crate) fn search_namespace<'s>(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        lib: &[SharedModule],
        scope: &'s mut Scope,
        this_ptr: &'s mut Dynamic,
        expr: &Expr,
    ) -> RhaiResultOf<(Target<'s>, Position)> {
        match expr {
            Expr::Variable(_, Some(_), _) => {
                self.search_scope_only(global, caches, lib, scope, this_ptr, expr)
            }
            Expr::Variable(v, None, _var_pos) => match &**v {
                // Normal variable access
                #[cfg(not(feature = "no_module"))]
                (_, ns, ..) if ns.is_empty() => {
                    self.search_scope_only(global, caches, lib, scope, this_ptr, expr)
                }
                #[cfg(feature = "no_module")]
                (_, (), ..) => self.search_scope_only(global, caches, lib, scope, this_ptr, expr),

                // Qualified variable access
                #[cfg(not(feature = "no_module"))]
                (_, namespace, hash_var, var_name) => {
                    // foo:bar::baz::VARIABLE
                    if let Some(module) = self.search_imports(global, namespace) {
                        return module.get_qualified_var(*hash_var).map_or_else(
                            || {
                                let sep = crate::tokenizer::Token::DoubleColon.literal_syntax();

                                Err(ERR::ErrorVariableNotFound(
                                    format!("{namespace}{sep}{var_name}"),
                                    namespace.position(),
                                )
                                .into())
                            },
                            |mut target| {
                                // Module variables are constant
                                target.set_access_mode(AccessMode::ReadOnly);
                                Ok((target.into(), *_var_pos))
                            },
                        );
                    }

                    // global::VARIABLE
                    #[cfg(not(feature = "no_function"))]
                    if namespace.len() == 1 && namespace.root() == crate::engine::KEYWORD_GLOBAL {
                        if let Some(ref constants) = global.constants {
                            if let Some(value) =
                                crate::func::locked_write(constants).get_mut(var_name.as_str())
                            {
                                let mut target: Target = value.clone().into();
                                // Module variables are constant
                                target.set_access_mode(AccessMode::ReadOnly);
                                return Ok((target, *_var_pos));
                            }
                        }

                        let sep = crate::tokenizer::Token::DoubleColon.literal_syntax();

                        return Err(ERR::ErrorVariableNotFound(
                            format!("{namespace}{sep}{var_name}"),
                            namespace.position(),
                        )
                        .into());
                    }

                    Err(
                        ERR::ErrorModuleNotFound(namespace.to_string(), namespace.position())
                            .into(),
                    )
                }
            },
            _ => unreachable!("Expr::Variable expected but gets {:?}", expr),
        }
    }

    /// Search for a variable within the scope
    ///
    /// # Panics
    ///
    /// Panics if `expr` is not [`Expr::Variable`].
    pub(crate) fn search_scope_only<'s>(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        lib: &[SharedModule],
        scope: &'s mut Scope,
        this_ptr: &'s mut Dynamic,
        expr: &Expr,
    ) -> RhaiResultOf<(Target<'s>, Position)> {
        // Make sure that the pointer indirection is taken only when absolutely necessary.

        let (index, var_pos) = match expr {
            // Check if the variable is `this`
            Expr::Variable(v, None, pos) if v.0.is_none() && v.3 == KEYWORD_THIS => {
                return if this_ptr.is_null() {
                    Err(ERR::ErrorUnboundThis(*pos).into())
                } else {
                    Ok((this_ptr.into(), *pos))
                };
            }
            _ if global.always_search_scope => (0, expr.start_position()),
            Expr::Variable(.., Some(i), pos) => (i.get() as usize, *pos),
            // Scripted function with the same name
            #[cfg(not(feature = "no_function"))]
            Expr::Variable(v, None, pos)
                if lib
                    .iter()
                    .flat_map(|m| m.iter_script_fn())
                    .any(|(_, _, f, ..)| f == v.3.as_str()) =>
            {
                let val: Dynamic =
                    crate::FnPtr::new_unchecked(v.3.as_str(), Default::default()).into();
                return Ok((val.into(), *pos));
            }
            Expr::Variable(v, None, pos) => (v.0.map_or(0, NonZeroUsize::get), *pos),
            _ => unreachable!("Expr::Variable expected but gets {:?}", expr),
        };

        // Check the variable resolver, if any
        if let Some(ref resolve_var) = self.resolve_var {
            let context = EvalContext::new(self, global, caches, lib, scope, this_ptr);
            let var_name = expr.get_variable_name(true).expect("`Expr::Variable`");
            match resolve_var(var_name, index, context) {
                Ok(Some(mut result)) => {
                    result.set_access_mode(AccessMode::ReadOnly);
                    return Ok((result.into(), var_pos));
                }
                Ok(None) => (),
                Err(err) => return Err(err.fill_position(var_pos)),
            }
        }

        let index = if index > 0 {
            scope.len() - index
        } else {
            // Find the variable in the scope
            let var_name = expr.get_variable_name(true).expect("`Expr::Variable`");

            match scope.search(var_name) {
                Some(index) => index,
                None => {
                    return match self.global_modules.iter().find_map(|m| m.get_var(var_name)) {
                        Some(val) => Ok((val.into(), var_pos)),
                        None => {
                            Err(ERR::ErrorVariableNotFound(var_name.to_string(), var_pos).into())
                        }
                    }
                }
            }
        };

        let val = scope.get_mut_by_index(index);

        Ok((val.into(), var_pos))
    }

    /// Evaluate an expression.
    //
    // # Implementation Notes
    //
    // Do not use the `?` operator within the main body as it makes this function return early,
    // possibly by-passing important cleanup tasks at the end.
    //
    // Errors that are not recoverable, such as system errors or safety errors, can use `?`.
    pub(crate) fn eval_expr(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        lib: &[SharedModule],
        scope: &mut Scope,
        this_ptr: &mut Dynamic,
        expr: &Expr,
    ) -> RhaiResult {
        // Coded this way for better branch prediction.
        // Popular branches are lifted out of the `match` statement into their own branches.

        // Function calls should account for a relatively larger portion of expressions because
        // binary operators are also function calls.
        if let Expr::FnCall(x, pos) = expr {
            #[cfg(feature = "debugging")]
            let reset = self.run_debugger_with_reset(global, caches, lib, scope, this_ptr, expr)?;
            #[cfg(feature = "debugging")]
            let global = &mut *crate::types::RestoreOnDrop::lock(global, move |g| {
                g.debugger.reset_status(reset)
            });

            self.track_operation(global, expr.position())?;

            return self.eval_fn_call_expr(global, caches, lib, scope, this_ptr, x, *pos);
        }

        // Then variable access.
        // We shouldn't do this for too many variants because, soon or later, the added comparisons
        // will cost more than the mis-predicted `match` branch.
        if let Expr::Variable(x, index, var_pos) = expr {
            #[cfg(feature = "debugging")]
            self.run_debugger(global, caches, lib, scope, this_ptr, expr)?;

            self.track_operation(global, expr.position())?;

            return if index.is_none() && x.0.is_none() && x.3 == KEYWORD_THIS {
                if this_ptr.is_null() {
                    ERR::ErrorUnboundThis(*var_pos).into()
                } else {
                    Ok(this_ptr.clone())
                }
            } else {
                self.search_namespace(global, caches, lib, scope, this_ptr, expr)
                    .map(|(val, ..)| val.take_or_clone())
            };
        }

        #[cfg(feature = "debugging")]
        let reset = self.run_debugger_with_reset(global, caches, lib, scope, this_ptr, expr)?;
        #[cfg(feature = "debugging")]
        let global = &mut *crate::types::RestoreOnDrop::lock(global, move |g| {
            g.debugger.reset_status(reset)
        });

        self.track_operation(global, expr.position())?;

        match expr {
            // Constants
            Expr::DynamicConstant(x, ..) => Ok(x.as_ref().clone()),
            Expr::IntegerConstant(x, ..) => Ok((*x).into()),
            #[cfg(not(feature = "no_float"))]
            Expr::FloatConstant(x, ..) => Ok((*x).into()),
            Expr::StringConstant(x, ..) => Ok(x.clone().into()),
            Expr::CharConstant(x, ..) => Ok((*x).into()),
            Expr::BoolConstant(x, ..) => Ok((*x).into()),
            Expr::Unit(..) => Ok(Dynamic::UNIT),

            // `... ${...} ...`
            Expr::InterpolatedString(x, _) => {
                let mut concat = self.const_empty_string().into();
                let target = &mut concat;

                let mut op_info = OpAssignment::new_op_assignment(OP_CONCAT, Position::NONE);
                let root = ("", Position::NONE);

                let result = x
                    .iter()
                    .try_for_each(|expr| {
                        let item = self
                            .eval_expr(global, caches, lib, scope, this_ptr, expr)?
                            .flatten();

                        op_info.pos = expr.start_position();

                        self.eval_op_assignment(global, caches, lib, &op_info, target, root, item)
                    })
                    .map(|_| concat.take_or_clone());

                self.check_return_value(result, expr.start_position())
            }

            #[cfg(not(feature = "no_index"))]
            Expr::Array(x, ..) => {
                #[cfg(not(feature = "unchecked"))]
                let mut total_data_sizes = (0, 0, 0);

                x.iter()
                    .try_fold(
                        crate::Array::with_capacity(x.len()),
                        |mut array, item_expr| {
                            let value = self
                                .eval_expr(global, caches, lib, scope, this_ptr, item_expr)?
                                .flatten();

                            #[cfg(not(feature = "unchecked"))]
                            if self.has_data_size_limit() {
                                let val_sizes = Self::calc_data_sizes(&value, true);

                                total_data_sizes = (
                                    total_data_sizes.0 + val_sizes.0,
                                    total_data_sizes.1 + val_sizes.1,
                                    total_data_sizes.2 + val_sizes.2,
                                );
                                self.raise_err_if_over_data_size_limit(total_data_sizes)
                                    .map_err(|err| err.fill_position(item_expr.position()))?;
                            }

                            array.push(value);

                            Ok(array)
                        },
                    )
                    .map(Into::into)
            }

            #[cfg(not(feature = "no_object"))]
            Expr::Map(x, ..) => {
                #[cfg(not(feature = "unchecked"))]
                let mut total_data_sizes = (0, 0, 0);

                x.0.iter()
                    .try_fold(x.1.clone(), |mut map, (key, value_expr)| {
                        let value = self
                            .eval_expr(global, caches, lib, scope, this_ptr, value_expr)?
                            .flatten();

                        #[cfg(not(feature = "unchecked"))]
                        if self.has_data_size_limit() {
                            let delta = Self::calc_data_sizes(&value, true);
                            total_data_sizes = (
                                total_data_sizes.0 + delta.0,
                                total_data_sizes.1 + delta.1,
                                total_data_sizes.2 + delta.2,
                            );
                            self.raise_err_if_over_data_size_limit(total_data_sizes)
                                .map_err(|err| err.fill_position(value_expr.position()))?;
                        }

                        *map.get_mut(key.as_str()).unwrap() = value;

                        Ok(map)
                    })
                    .map(Into::into)
            }

            Expr::And(x, ..) => Ok((self
                .eval_expr(global, caches, lib, scope, this_ptr, &x.lhs)?
                .as_bool()
                .map_err(|typ| self.make_type_mismatch_err::<bool>(typ, x.lhs.position()))?
                && self
                    .eval_expr(global, caches, lib, scope, this_ptr, &x.rhs)?
                    .as_bool()
                    .map_err(|typ| self.make_type_mismatch_err::<bool>(typ, x.rhs.position()))?)
            .into()),

            Expr::Or(x, ..) => Ok((self
                .eval_expr(global, caches, lib, scope, this_ptr, &x.lhs)?
                .as_bool()
                .map_err(|typ| self.make_type_mismatch_err::<bool>(typ, x.lhs.position()))?
                || self
                    .eval_expr(global, caches, lib, scope, this_ptr, &x.rhs)?
                    .as_bool()
                    .map_err(|typ| self.make_type_mismatch_err::<bool>(typ, x.rhs.position()))?)
            .into()),

            Expr::Coalesce(x, ..) => {
                let value = self.eval_expr(global, caches, lib, scope, this_ptr, &x.lhs)?;

                if value.is_unit() {
                    self.eval_expr(global, caches, lib, scope, this_ptr, &x.rhs)
                } else {
                    Ok(value)
                }
            }

            #[cfg(not(feature = "no_custom_syntax"))]
            Expr::Custom(custom, pos) => {
                let expressions: crate::StaticVec<_> =
                    custom.inputs.iter().map(Into::into).collect();
                // The first token acts as the custom syntax's key
                let key_token = custom.tokens.first().unwrap();
                // The key should exist, unless the AST is compiled in a different Engine
                let custom_def = self.custom_syntax.get(key_token.as_str()).ok_or_else(|| {
                    Box::new(ERR::ErrorCustomSyntax(
                        format!("Invalid custom syntax prefix: {key_token}"),
                        custom.tokens.iter().map(<_>::to_string).collect(),
                        *pos,
                    ))
                })?;
                let mut context = EvalContext::new(self, global, caches, lib, scope, this_ptr);

                let result = (custom_def.func)(&mut context, &expressions, &custom.state);

                self.check_return_value(result, expr.start_position())
            }

            Expr::Stmt(x) if x.is_empty() => Ok(Dynamic::UNIT),
            Expr::Stmt(x) => self.eval_stmt_block(global, caches, lib, scope, this_ptr, x, true),

            #[cfg(not(feature = "no_index"))]
            Expr::Index(..) => {
                self.eval_dot_index_chain(global, caches, lib, scope, this_ptr, expr, &mut None)
            }

            #[cfg(not(feature = "no_object"))]
            Expr::Dot(..) => {
                self.eval_dot_index_chain(global, caches, lib, scope, this_ptr, expr, &mut None)
            }

            _ => unreachable!("expression cannot be evaluated: {:?}", expr),
        }
    }
}
