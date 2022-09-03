//! Module defining functions for evaluating an expression.

use super::{Caches, EvalContext, GlobalRuntimeState, Target};
use crate::ast::{Expr, FnCallExpr, OpAssignment};
use crate::engine::{KEYWORD_THIS, OP_CONCAT};
use crate::types::dynamic::AccessMode;
use crate::{Dynamic, Engine, Module, Position, RhaiResult, RhaiResultOf, Scope, ERR};
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
    ) -> Option<crate::Shared<Module>> {
        assert!(!namespace.is_empty());

        let root = namespace.root();

        // Qualified - check if the root module is directly indexed
        let index = if global.always_search_scope {
            None
        } else {
            namespace.index()
        };

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
        scope: &'s mut Scope,
        global: &mut GlobalRuntimeState,
        lib: &[&Module],
        this_ptr: &'s mut Option<&mut Dynamic>,
        expr: &Expr,
        level: usize,
    ) -> RhaiResultOf<(Target<'s>, Position)> {
        match expr {
            Expr::Variable(_, Some(_), _) => {
                self.search_scope_only(scope, global, lib, this_ptr, expr, level)
            }
            Expr::Variable(v, None, _var_pos) => match &**v {
                // Normal variable access
                #[cfg(not(feature = "no_module"))]
                (_, ns, ..) if ns.is_empty() => {
                    self.search_scope_only(scope, global, lib, this_ptr, expr, level)
                }
                #[cfg(feature = "no_module")]
                (_, (), ..) => self.search_scope_only(scope, global, lib, this_ptr, expr, level),

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
        scope: &'s mut Scope,
        global: &mut GlobalRuntimeState,
        lib: &[&Module],
        this_ptr: &'s mut Option<&mut Dynamic>,
        expr: &Expr,
        level: usize,
    ) -> RhaiResultOf<(Target<'s>, Position)> {
        // Make sure that the pointer indirection is taken only when absolutely necessary.

        let (index, var_pos) = match expr {
            // Check if the variable is `this`
            Expr::Variable(v, None, pos) if v.0.is_none() && v.3 == KEYWORD_THIS => {
                return this_ptr.as_mut().map_or_else(
                    || Err(ERR::ErrorUnboundThis(*pos).into()),
                    |val| Ok(((*val).into(), *pos)),
                )
            }
            _ if global.always_search_scope => (0, expr.start_position()),
            Expr::Variable(.., Some(i), pos) => (i.get() as usize, *pos),
            // Scripted function with the same name
            #[cfg(not(feature = "no_function"))]
            Expr::Variable(v, None, pos)
                if lib
                    .iter()
                    .flat_map(|&m| m.iter_script_fn())
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
            let context = EvalContext::new(self, scope, global, None, lib, this_ptr, level);
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

            match scope.get_index(var_name) {
                Some((index, _)) => index,
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

    /// Evaluate a function call expression.
    pub(crate) fn eval_fn_call_expr(
        &self,
        scope: &mut Scope,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        lib: &[&Module],
        this_ptr: &mut Option<&mut Dynamic>,
        expr: &FnCallExpr,
        pos: Position,
        level: usize,
    ) -> RhaiResult {
        let FnCallExpr {
            name,
            #[cfg(not(feature = "no_module"))]
            namespace,
            capture_parent_scope: capture,
            is_standard_operator,
            hashes,
            args,
            ..
        } = expr;

        #[cfg(feature = "fast_ops")]
        if *is_standard_operator {
            let mut lhs = self
                .get_arg_value(scope, global, caches, lib, this_ptr, &args[0], level)?
                .0
                .flatten();

            let mut rhs = self
                .get_arg_value(scope, global, caches, lib, this_ptr, &args[1], level)?
                .0
                .flatten();

            let arg_values = &mut [&mut lhs, &mut rhs];

            let hash = crate::func::combine_hashes(
                hashes.native,
                crate::func::calc_fn_params_hash(arg_values.iter().map(|a| a.type_id())),
            );

            let c = caches.fn_resolution_cache_mut();

            let entry = if c.contains_key(&hash) {
                match c.get(&hash).unwrap() {
                    Some(entry) => entry,
                    None => {
                        return Err(ERR::ErrorFunctionNotFound(
                            self.gen_fn_call_signature(
                                #[cfg(not(feature = "no_module"))]
                                &crate::ast::Namespace::NONE,
                                name,
                                arg_values,
                            ),
                            pos,
                        )
                        .into())
                    }
                }
            } else {
                match crate::func::get_builtin_binary_op_fn(&name, arg_values[0], arg_values[1]) {
                    Some(f) => {
                        let entry = crate::eval::FnResolutionCacheEntry {
                            func: crate::func::CallableFunction::from_method(
                                Box::new(f) as Box<crate::func::FnAny>
                            ),
                            source: None,
                        };
                        c.insert(hash, Some(entry));
                        c.get(&hash).unwrap().as_ref().unwrap()
                    }
                    None => {
                        return self
                            .exec_fn_call(
                                None, global, caches, lib, name, *hashes, arg_values, false, false,
                                pos, level,
                            )
                            .map(|(v, ..)| v)
                    }
                }
            };

            let func = entry.func.get_native_fn().unwrap();
            let context = (self, name, None, &*global, lib, pos, level).into();
            let result = (func)(context, arg_values);
            return self.check_return_value(result, pos);
        }

        #[cfg(not(feature = "no_module"))]
        if !namespace.is_empty() {
            // Qualified function call
            let hash = hashes.native;

            return self.make_qualified_function_call(
                scope, global, caches, lib, this_ptr, namespace, name, args, hash, pos, level,
            );
        }

        // Normal function call
        let (first_arg, args) = args.split_first().map_or_else(
            || (None, args.as_ref()),
            |(first, rest)| (Some(first), rest),
        );

        self.make_function_call(
            scope, global, caches, lib, this_ptr, name, first_arg, args, *hashes, *capture, pos,
            level,
        )
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
        scope: &mut Scope,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        lib: &[&Module],
        this_ptr: &mut Option<&mut Dynamic>,
        expr: &Expr,
        level: usize,
    ) -> RhaiResult {
        // Coded this way for better branch prediction.
        // Popular branches are lifted out of the `match` statement into their own branches.

        // Function calls should account for a relatively larger portion of expressions because
        // binary operators are also function calls.
        if let Expr::FnCall(x, ..) = expr {
            #[cfg(feature = "debugging")]
            let reset_debugger =
                self.run_debugger_with_reset(scope, global, lib, this_ptr, expr, level)?;

            #[cfg(not(feature = "unchecked"))]
            self.inc_operations(&mut global.num_operations, expr.position())?;

            let result =
                self.eval_fn_call_expr(scope, global, caches, lib, this_ptr, x, x.pos, level);

            #[cfg(feature = "debugging")]
            global.debugger.reset_status(reset_debugger);

            return result;
        }

        // Then variable access.
        // We shouldn't do this for too many variants because, soon or later, the added comparisons
        // will cost more than the mis-predicted `match` branch.
        if let Expr::Variable(x, index, var_pos) = expr {
            #[cfg(feature = "debugging")]
            self.run_debugger(scope, global, lib, this_ptr, expr, level)?;

            #[cfg(not(feature = "unchecked"))]
            self.inc_operations(&mut global.num_operations, expr.position())?;

            return if index.is_none() && x.0.is_none() && x.3 == KEYWORD_THIS {
                this_ptr
                    .as_deref()
                    .cloned()
                    .ok_or_else(|| ERR::ErrorUnboundThis(*var_pos).into())
            } else {
                self.search_namespace(scope, global, lib, this_ptr, expr, level)
                    .map(|(val, ..)| val.take_or_clone())
            };
        }

        #[cfg(feature = "debugging")]
        let reset_debugger =
            self.run_debugger_with_reset(scope, global, lib, this_ptr, expr, level)?;

        #[cfg(not(feature = "unchecked"))]
        self.inc_operations(&mut global.num_operations, expr.position())?;

        let result = match expr {
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
                let mut concat = self.get_interned_string("").into();
                let target = &mut concat;
                let mut result = Ok(Dynamic::UNIT);

                let mut op_info = OpAssignment::new_op_assignment(OP_CONCAT, Position::NONE);
                let root = ("", Position::NONE);

                for expr in &**x {
                    let item =
                        match self.eval_expr(scope, global, caches, lib, this_ptr, expr, level) {
                            Ok(r) => r,
                            err => {
                                result = err;
                                break;
                            }
                        };

                    op_info.pos = expr.start_position();

                    if let Err(err) = self
                        .eval_op_assignment(global, caches, lib, op_info, target, root, item, level)
                    {
                        result = Err(err);
                        break;
                    }
                }

                self.check_return_value(
                    result.map(|_| concat.take_or_clone()),
                    expr.start_position(),
                )
            }

            #[cfg(not(feature = "no_index"))]
            Expr::Array(x, ..) => {
                let mut array = crate::Array::with_capacity(x.len());
                let mut result = Ok(Dynamic::UNIT);

                #[cfg(not(feature = "unchecked"))]
                let mut sizes = (0, 0, 0);

                for item_expr in &**x {
                    let value = match self
                        .eval_expr(scope, global, caches, lib, this_ptr, item_expr, level)
                    {
                        Ok(r) => r.flatten(),
                        err => {
                            result = err;
                            break;
                        }
                    };

                    #[cfg(not(feature = "unchecked"))]
                    let val_sizes = Self::calc_data_sizes(&value, true);

                    array.push(value);

                    #[cfg(not(feature = "unchecked"))]
                    if self.has_data_size_limit() {
                        sizes = (
                            sizes.0 + val_sizes.0,
                            sizes.1 + val_sizes.1,
                            sizes.2 + val_sizes.2,
                        );
                        self.raise_err_if_over_data_size_limit(sizes, item_expr.position())?;
                    }
                }

                result.map(|_| array.into())
            }

            #[cfg(not(feature = "no_object"))]
            Expr::Map(x, ..) => {
                let mut map = x.1.clone();
                let mut result = Ok(Dynamic::UNIT);

                #[cfg(not(feature = "unchecked"))]
                let mut sizes = (0, 0, 0);

                for (key, value_expr) in &x.0 {
                    let value = match self
                        .eval_expr(scope, global, caches, lib, this_ptr, value_expr, level)
                    {
                        Ok(r) => r.flatten(),
                        err => {
                            result = err;
                            break;
                        }
                    };

                    #[cfg(not(feature = "unchecked"))]
                    let delta = Self::calc_data_sizes(&value, true);

                    *map.get_mut(key.as_str()).unwrap() = value;

                    #[cfg(not(feature = "unchecked"))]
                    if self.has_data_size_limit() {
                        sizes = (sizes.0 + delta.0, sizes.1 + delta.1, sizes.2 + delta.2);
                        self.raise_err_if_over_data_size_limit(sizes, value_expr.position())?;
                    }
                }

                result.map(|_| map.into())
            }

            Expr::And(x, ..) => {
                let lhs = self
                    .eval_expr(scope, global, caches, lib, this_ptr, &x.lhs, level)
                    .and_then(|v| {
                        v.as_bool().map_err(|typ| {
                            self.make_type_mismatch_err::<bool>(typ, x.lhs.position())
                        })
                    });

                if let Ok(true) = lhs {
                    self.eval_expr(scope, global, caches, lib, this_ptr, &x.rhs, level)
                        .and_then(|v| {
                            v.as_bool()
                                .map_err(|typ| {
                                    self.make_type_mismatch_err::<bool>(typ, x.rhs.position())
                                })
                                .map(Into::into)
                        })
                } else {
                    lhs.map(Into::into)
                }
            }

            Expr::Or(x, ..) => {
                let lhs = self
                    .eval_expr(scope, global, caches, lib, this_ptr, &x.lhs, level)
                    .and_then(|v| {
                        v.as_bool().map_err(|typ| {
                            self.make_type_mismatch_err::<bool>(typ, x.lhs.position())
                        })
                    });

                if let Ok(false) = lhs {
                    self.eval_expr(scope, global, caches, lib, this_ptr, &x.rhs, level)
                        .and_then(|v| {
                            v.as_bool()
                                .map_err(|typ| {
                                    self.make_type_mismatch_err::<bool>(typ, x.rhs.position())
                                })
                                .map(Into::into)
                        })
                } else {
                    lhs.map(Into::into)
                }
            }

            Expr::Coalesce(x, ..) => {
                let lhs = self.eval_expr(scope, global, caches, lib, this_ptr, &x.lhs, level);

                match lhs {
                    Ok(value) if value.is::<()>() => {
                        self.eval_expr(scope, global, caches, lib, this_ptr, &x.rhs, level)
                    }
                    Ok(_) | Err(_) => lhs,
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
                let mut context =
                    EvalContext::new(self, scope, global, Some(caches), lib, this_ptr, level);

                let result = (custom_def.func)(&mut context, &expressions);

                self.check_return_value(result, expr.start_position())
            }

            Expr::Stmt(x) if x.is_empty() => Ok(Dynamic::UNIT),
            Expr::Stmt(x) => {
                self.eval_stmt_block(scope, global, caches, lib, this_ptr, x, true, level)
            }

            #[cfg(not(feature = "no_index"))]
            Expr::Index(..) => {
                self.eval_dot_index_chain(scope, global, caches, lib, this_ptr, expr, level, None)
            }

            #[cfg(not(feature = "no_object"))]
            Expr::Dot(..) => {
                self.eval_dot_index_chain(scope, global, caches, lib, this_ptr, expr, level, None)
            }

            _ => unreachable!("expression cannot be evaluated: {:?}", expr),
        };

        #[cfg(feature = "debugging")]
        global.debugger.reset_status(reset_debugger);

        result
    }
}
