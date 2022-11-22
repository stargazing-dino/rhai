//! Module defining functions for evaluating a statement.

use super::{Caches, EvalContext, GlobalRuntimeState, Target};
use crate::api::events::VarDefInfo;
use crate::ast::{
    ASTFlags, BinaryExpr, Expr, Ident, OpAssignment, Stmt, SwitchCasesCollection, TryCatchBlock,
};
use crate::func::{get_builtin_op_assignment_fn, get_hasher};
use crate::types::dynamic::AccessMode;
use crate::types::RestoreOnDrop;
use crate::{Dynamic, Engine, RhaiResult, RhaiResultOf, Scope, ERR, INT};
use std::hash::{Hash, Hasher};
#[cfg(feature = "no_std")]
use std::prelude::v1::*;

impl Engine {
    /// Evaluate a statements block.
    pub(crate) fn eval_stmt_block(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        scope: &mut Scope,
        this_ptr: &mut Dynamic,
        statements: &[Stmt],
        restore_orig_state: bool,
    ) -> RhaiResult {
        if statements.is_empty() {
            return Ok(Dynamic::UNIT);
        }

        // Restore scope at end of block if necessary
        let orig_scope_len = scope.len();
        let scope = &mut *RestoreOnDrop::lock_if(restore_orig_state, scope, move |s| {
            s.rewind(orig_scope_len);
        });

        // Restore global state at end of block if necessary
        let orig_always_search_scope = global.always_search_scope;
        #[cfg(not(feature = "no_module"))]
        let orig_imports_len = global.num_imports();

        if restore_orig_state {
            global.scope_level += 1;
        }

        let global = &mut *RestoreOnDrop::lock_if(restore_orig_state, global, move |g| {
            g.scope_level -= 1;

            #[cfg(not(feature = "no_module"))]
            g.truncate_imports(orig_imports_len);

            // The impact of new local variables goes away at the end of a block
            // because any new variables introduced will go out of scope
            g.always_search_scope = orig_always_search_scope;
        });

        // Pop new function resolution caches at end of block
        let orig_fn_resolution_caches_len = caches.fn_resolution_caches_len();
        let caches = &mut *RestoreOnDrop::lock(caches, move |c| {
            c.rewind_fn_resolution_caches(orig_fn_resolution_caches_len)
        });

        // Run the statements
        statements.iter().try_fold(Dynamic::UNIT, |_, stmt| {
            #[cfg(not(feature = "no_module"))]
            let imports_len = global.num_imports();

            let result =
                self.eval_stmt(global, caches, scope, this_ptr, stmt, restore_orig_state)?;

            #[cfg(not(feature = "no_module"))]
            if matches!(stmt, Stmt::Import(..)) {
                // Get the extra modules - see if any functions are marked global.
                // Without global functions, the extra modules never affect function resolution.
                if global
                    .scan_imports_raw()
                    .skip(imports_len)
                    .any(|(.., m)| m.contains_indexed_global_functions())
                {
                    // Different scenarios where the cache must be cleared - notice that this is
                    // expensive as all function resolutions must start again
                    if caches.fn_resolution_caches_len() > orig_fn_resolution_caches_len {
                        // When new module is imported with global functions and there is already
                        // a new cache, just clear it
                        caches.fn_resolution_cache_mut().clear();
                    } else if restore_orig_state {
                        // When new module is imported with global functions, push a new cache
                        caches.push_fn_resolution_cache();
                    } else {
                        // When the block is to be evaluated in-place, just clear the current cache
                        caches.fn_resolution_cache_mut().clear();
                    }
                }
            }

            Ok(result)
        })
    }

    /// Evaluate an op-assignment statement.
    pub(crate) fn eval_op_assignment(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        op_info: &OpAssignment,
        root: &Expr,
        target: &mut Target,
        mut new_val: Dynamic,
    ) -> RhaiResultOf<()> {
        // Assignment to constant variable?
        if target.is_read_only() {
            let name = root.get_variable_name(false).unwrap_or_default();
            let pos = root.start_position();
            return Err(ERR::ErrorAssignmentToConstant(name.to_string(), pos).into());
        }

        if op_info.is_op_assignment() {
            let OpAssignment {
                hash_op_assign,
                hash_op,
                op_assign: op_assign_token,
                op: op_token,
                pos: op_pos,
            } = op_info;

            let mut lock_guard = target.write_lock::<Dynamic>().unwrap();

            let hash = *hash_op_assign;
            let args = &mut [&mut *lock_guard, &mut new_val];

            if self.fast_operators() {
                if let Some(func) = get_builtin_op_assignment_fn(op_assign_token, args[0], args[1])
                {
                    // Built-in found
                    let op = op_assign_token.literal_syntax();

                    let orig_level = global.level;
                    global.level += 1;
                    let global = &*RestoreOnDrop::lock(global, move |g| g.level = orig_level);

                    let context = (self, op, None, global, *op_pos).into();
                    return func(context, args).map(|_| ());
                }
            }

            let op_assign = op_assign_token.literal_syntax();
            let op = op_token.literal_syntax();
            let token = Some(op_assign_token);

            match self
                .exec_native_fn_call(global, caches, op_assign, token, hash, args, true, *op_pos)
            {
                Ok(_) => (),
                Err(err) if matches!(*err, ERR::ErrorFunctionNotFound(ref f, ..) if f.starts_with(op_assign)) =>
                {
                    // Expand to `var = var op rhs`
                    let token = Some(op_token);

                    *args[0] = self
                        .exec_native_fn_call(
                            global, caches, op, token, *hash_op, args, true, *op_pos,
                        )?
                        .0;
                }
                Err(err) => return Err(err),
            }

            self.check_data_size(&*args[0], root.position())?;
        } else {
            // Normal assignment

            // If value is a string, intern it
            if new_val.is_string() {
                let value = new_val.into_immutable_string().expect("`ImmutableString`");
                new_val = self.get_interned_string(value).into();
            }

            *target.write_lock::<Dynamic>().unwrap() = new_val;
        }

        target.propagate_changed_value(op_info.pos)
    }

    /// Evaluate a statement.
    pub(crate) fn eval_stmt(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        scope: &mut Scope,
        this_ptr: &mut Dynamic,
        stmt: &Stmt,
        rewind_scope: bool,
    ) -> RhaiResult {
        #[cfg(feature = "debugging")]
        let reset = self.run_debugger_with_reset(global, caches, scope, this_ptr, stmt)?;
        #[cfg(feature = "debugging")]
        let global = &mut *RestoreOnDrop::lock(global, move |g| g.debugger.reset_status(reset));

        // Coded this way for better branch prediction.
        // Popular branches are lifted out of the `match` statement into their own branches.

        // Function calls should account for a relatively larger portion of statements.
        if let Stmt::FnCall(x, pos) = stmt {
            self.track_operation(global, stmt.position())?;

            return self.eval_fn_call_expr(global, caches, scope, this_ptr, x, *pos);
        }

        // Then assignments.
        // We shouldn't do this for too many variants because, soon or later, the added comparisons
        // will cost more than the mis-predicted `match` branch.
        if let Stmt::Assignment(x, ..) = stmt {
            let (op_info, BinaryExpr { lhs, rhs }) = &**x;

            self.track_operation(global, stmt.position())?;

            if let Expr::Variable(x, ..) = lhs {
                let rhs_val = self
                    .eval_expr(global, caches, scope, this_ptr, rhs)?
                    .flatten();

                let mut target = self.search_namespace(global, caches, scope, this_ptr, lhs)?;

                let var_name = x.3.as_str();

                #[cfg(not(feature = "no_closure"))]
                // Also handle case where target is a `Dynamic` shared value
                // (returned by a variable resolver, for example)
                let is_temp_result = !target.is_ref() && !target.is_shared();
                #[cfg(feature = "no_closure")]
                let is_temp_result = !target.is_ref();

                // Cannot assign to temp result from expression
                if is_temp_result {
                    return Err(ERR::ErrorAssignmentToConstant(
                        var_name.to_string(),
                        lhs.position(),
                    )
                    .into());
                }

                self.track_operation(global, lhs.position())?;

                self.eval_op_assignment(global, caches, op_info, lhs, &mut target, rhs_val)?;

                return Ok(Dynamic::UNIT);
            }

            #[cfg(any(not(feature = "no_index"), not(feature = "no_object")))]
            {
                let mut rhs_val = self
                    .eval_expr(global, caches, scope, this_ptr, rhs)?
                    .flatten();

                // If value is a string, intern it
                if rhs_val.is_string() {
                    let value = rhs_val.into_immutable_string().expect("`ImmutableString`");
                    rhs_val = self.get_interned_string(value).into();
                }

                let _new_val = Some((rhs_val, op_info));

                // Must be either `var[index] op= val` or `var.prop op= val`
                match lhs {
                    // name op= rhs (handled above)
                    Expr::Variable(..) => {
                        unreachable!("Expr::Variable case is already handled")
                    }
                    // idx_lhs[idx_expr] op= rhs
                    #[cfg(not(feature = "no_index"))]
                    Expr::Index(..) => {
                        self.eval_dot_index_chain(global, caches, scope, this_ptr, lhs, _new_val)
                    }
                    // dot_lhs.dot_rhs op= rhs
                    #[cfg(not(feature = "no_object"))]
                    Expr::Dot(..) => {
                        self.eval_dot_index_chain(global, caches, scope, this_ptr, lhs, _new_val)
                    }
                    _ => unreachable!("cannot assign to expression: {:?}", lhs),
                }?;

                return Ok(Dynamic::UNIT);
            }
        }

        self.track_operation(global, stmt.position())?;

        match stmt {
            // No-op
            Stmt::Noop(..) => Ok(Dynamic::UNIT),

            // Expression as statement
            Stmt::Expr(expr) => self
                .eval_expr(global, caches, scope, this_ptr, expr)
                .map(Dynamic::flatten),

            // Block scope
            Stmt::Block(statements, ..) if statements.is_empty() => Ok(Dynamic::UNIT),
            Stmt::Block(statements, ..) => {
                self.eval_stmt_block(global, caches, scope, this_ptr, statements, true)
            }

            // If statement
            Stmt::If(x, ..) => {
                let (expr, if_block, else_block) = &**x;

                let guard_val = self
                    .eval_expr(global, caches, scope, this_ptr, expr)?
                    .as_bool()
                    .map_err(|typ| self.make_type_mismatch_err::<bool>(typ, expr.position()))?;

                if guard_val && !if_block.is_empty() {
                    self.eval_stmt_block(global, caches, scope, this_ptr, if_block, true)
                } else if !guard_val && !else_block.is_empty() {
                    self.eval_stmt_block(global, caches, scope, this_ptr, else_block, true)
                } else {
                    Ok(Dynamic::UNIT)
                }
            }

            // Switch statement
            Stmt::Switch(x, ..) => {
                let (
                    expr,
                    SwitchCasesCollection {
                        expressions,
                        cases,
                        def_case,
                        ranges,
                    },
                ) = &**x;

                let mut result = None;

                let value = self.eval_expr(global, caches, scope, this_ptr, expr)?;

                if value.is_hashable() {
                    let hasher = &mut get_hasher();
                    value.hash(hasher);
                    let hash = hasher.finish();

                    // First check hashes
                    if let Some(case_blocks_list) = cases.get(&hash) {
                        assert!(!case_blocks_list.is_empty());

                        for &index in case_blocks_list {
                            let block = &expressions[index];

                            let cond_result = match block.condition {
                                Expr::BoolConstant(b, ..) => b,
                                ref c => self
                                    .eval_expr(global, caches, scope, this_ptr, c)?
                                    .as_bool()
                                    .map_err(|typ| {
                                        self.make_type_mismatch_err::<bool>(typ, c.position())
                                    })?,
                            };

                            if cond_result {
                                result = Some(&block.expr);
                                break;
                            }
                        }
                    } else if value.is_int() && !ranges.is_empty() {
                        // Then check integer ranges
                        let value = value.as_int().expect("`INT`");

                        for r in ranges.iter().filter(|r| r.contains(value)) {
                            let block = &expressions[r.index()];

                            let cond_result = match block.condition {
                                Expr::BoolConstant(b, ..) => b,
                                ref c => self
                                    .eval_expr(global, caches, scope, this_ptr, c)?
                                    .as_bool()
                                    .map_err(|typ| {
                                        self.make_type_mismatch_err::<bool>(typ, c.position())
                                    })?,
                            };

                            if cond_result {
                                result = Some(&block.expr);
                                break;
                            }
                        }
                    }
                }

                result
                    .or_else(|| def_case.as_ref().map(|&index| &expressions[index].expr))
                    .map_or(Ok(Dynamic::UNIT), |expr| {
                        self.eval_expr(global, caches, scope, this_ptr, expr)
                    })
            }

            // Loop
            Stmt::While(x, ..) if matches!(x.0, Expr::Unit(..) | Expr::BoolConstant(true, ..)) => {
                let (.., body) = &**x;

                if body.is_empty() {
                    loop {
                        self.track_operation(global, body.position())?;
                    }
                }

                loop {
                    if let Err(err) =
                        self.eval_stmt_block(global, caches, scope, this_ptr, body, true)
                    {
                        match *err {
                            ERR::LoopBreak(false, ..) => (),
                            ERR::LoopBreak(true, value, ..) => break Ok(value),
                            _ => break Err(err),
                        }
                    }
                }
            }

            // While loop
            Stmt::While(x, ..) => {
                let (expr, body) = &**x;

                loop {
                    let condition = self
                        .eval_expr(global, caches, scope, this_ptr, expr)?
                        .as_bool()
                        .map_err(|typ| self.make_type_mismatch_err::<bool>(typ, expr.position()))?;

                    if !condition {
                        break Ok(Dynamic::UNIT);
                    }

                    if body.is_empty() {
                        continue;
                    }

                    if let Err(err) =
                        self.eval_stmt_block(global, caches, scope, this_ptr, body, true)
                    {
                        match *err {
                            ERR::LoopBreak(false, ..) => (),
                            ERR::LoopBreak(true, value, ..) => break Ok(value),
                            _ => break Err(err),
                        }
                    }
                }
            }

            // Do loop
            Stmt::Do(x, options, ..) => {
                let (expr, body) = &**x;
                let is_while = !options.contains(ASTFlags::NEGATED);

                loop {
                    if !body.is_empty() {
                        if let Err(err) =
                            self.eval_stmt_block(global, caches, scope, this_ptr, body, true)
                        {
                            match *err {
                                ERR::LoopBreak(false, ..) => continue,
                                ERR::LoopBreak(true, value, ..) => break Ok(value),
                                _ => break Err(err),
                            }
                        }
                    }

                    let condition = self
                        .eval_expr(global, caches, scope, this_ptr, expr)?
                        .as_bool()
                        .map_err(|typ| self.make_type_mismatch_err::<bool>(typ, expr.position()))?;

                    if condition ^ is_while {
                        break Ok(Dynamic::UNIT);
                    }
                }
            }

            // For loop
            Stmt::For(x, ..) => {
                let (var_name, counter, expr, statements) = &**x;

                let iter_obj = self
                    .eval_expr(global, caches, scope, this_ptr, expr)?
                    .flatten();

                let iter_type = iter_obj.type_id();

                // lib should only contain scripts, so technically they cannot have iterators

                // Search order:
                // 1) Global namespace - functions registered via Engine::register_XXX
                // 2) Global modules - packages
                // 3) Imported modules - functions marked with global namespace
                // 4) Global sub-modules - functions marked with global namespace
                let func = self
                    .global_modules
                    .iter()
                    .find_map(|m| m.get_iter(iter_type));

                #[cfg(not(feature = "no_module"))]
                let func = func.or_else(|| global.get_iter(iter_type)).or_else(|| {
                    self.global_sub_modules
                        .values()
                        .find_map(|m| m.get_qualified_iter(iter_type))
                });

                let func = func.ok_or_else(|| ERR::ErrorFor(expr.start_position()))?;

                // Restore scope at end of statement
                let orig_scope_len = scope.len();
                let scope = &mut *RestoreOnDrop::lock(scope, move |s| {
                    s.rewind(orig_scope_len);
                });

                // Add the loop variables
                let counter_index = if counter.is_empty() {
                    usize::MAX
                } else {
                    scope.push(counter.name.clone(), 0 as INT);
                    scope.len() - 1
                };

                scope.push(var_name.name.clone(), ());
                let index = scope.len() - 1;

                let mut result = Dynamic::UNIT;

                for (x, iter_value) in func(iter_obj).enumerate() {
                    // Increment counter
                    if counter_index < usize::MAX {
                        // As the variable increments from 0, this should always work
                        // since any overflow will first be caught below.
                        let index_value = x as INT;

                        #[cfg(not(feature = "unchecked"))]
                        if index_value > crate::MAX_USIZE_INT {
                            return Err(ERR::ErrorArithmetic(
                                format!("for-loop counter overflow: {x}"),
                                counter.pos,
                            )
                            .into());
                        }

                        *scope.get_mut_by_index(counter_index).write_lock().unwrap() =
                            Dynamic::from_int(index_value);
                    }

                    // Set loop value
                    let value = iter_value
                        .map_err(|err| err.fill_position(expr.position()))?
                        .flatten();

                    *scope.get_mut_by_index(index).write_lock().unwrap() = value;

                    // Run block
                    self.track_operation(global, statements.position())?;

                    if statements.is_empty() {
                        continue;
                    }

                    match self.eval_stmt_block(global, caches, scope, this_ptr, statements, true) {
                        Ok(_) => (),
                        Err(err) => match *err {
                            ERR::LoopBreak(false, ..) => (),
                            ERR::LoopBreak(true, value, ..) => {
                                result = value;
                                break;
                            }
                            _ => return Err(err),
                        },
                    }
                }

                Ok(result)
            }

            // Continue/Break statement
            Stmt::BreakLoop(expr, options, pos) => {
                let is_break = options.contains(ASTFlags::BREAK);

                let value = if let Some(ref expr) = expr {
                    self.eval_expr(global, caches, scope, this_ptr, expr)?
                } else {
                    Dynamic::UNIT
                };

                Err(ERR::LoopBreak(is_break, value, *pos).into())
            }

            // Try/Catch statement
            Stmt::TryCatch(x, ..) => {
                let TryCatchBlock {
                    try_block,
                    catch_var:
                        Ident {
                            name: catch_var, ..
                        },
                    catch_block,
                } = &**x;

                match self.eval_stmt_block(global, caches, scope, this_ptr, try_block, true) {
                    r @ Ok(_) => r,
                    Err(err) if err.is_pseudo_error() => Err(err),
                    Err(err) if !err.is_catchable() => Err(err),
                    Err(mut err) => {
                        let err_value = match err.unwrap_inner() {
                            ERR::ErrorRuntime(x, ..) => x.clone(),

                            #[cfg(feature = "no_object")]
                            _ => {
                                let _ = err.take_position();
                                err.to_string().into()
                            }
                            #[cfg(not(feature = "no_object"))]
                            _ => {
                                let mut err_map = crate::Map::new();
                                let err_pos = err.take_position();

                                err_map.insert("message".into(), err.to_string().into());

                                if let Some(ref source) = global.source {
                                    err_map.insert("source".into(), source.into());
                                }

                                if !err_pos.is_none() {
                                    err_map.insert(
                                        "line".into(),
                                        (err_pos.line().unwrap() as INT).into(),
                                    );
                                    err_map.insert(
                                        "position".into(),
                                        (err_pos.position().unwrap_or(0) as INT).into(),
                                    );
                                }

                                err.dump_fields(&mut err_map);
                                err_map.into()
                            }
                        };

                        // Restore scope at end of block
                        let orig_scope_len = scope.len();
                        let scope =
                            &mut *RestoreOnDrop::lock_if(!catch_var.is_empty(), scope, move |s| {
                                s.rewind(orig_scope_len);
                            });

                        if !catch_var.is_empty() {
                            scope.push(catch_var.clone(), err_value);
                        }

                        self.eval_stmt_block(global, caches, scope, this_ptr, catch_block, true)
                            .map(|_| Dynamic::UNIT)
                            .map_err(|result_err| match *result_err {
                                // Re-throw exception
                                ERR::ErrorRuntime(v, pos) if v.is_unit() => {
                                    err.set_position(pos);
                                    err
                                }
                                _ => result_err,
                            })
                    }
                }
            }

            // Throw value
            Stmt::Return(Some(expr), options, pos) if options.contains(ASTFlags::BREAK) => self
                .eval_expr(global, caches, scope, this_ptr, expr)
                .and_then(|v| Err(ERR::ErrorRuntime(v.flatten(), *pos).into())),

            // Empty throw
            Stmt::Return(None, options, pos) if options.contains(ASTFlags::BREAK) => {
                Err(ERR::ErrorRuntime(Dynamic::UNIT, *pos).into())
            }

            // Return value
            Stmt::Return(Some(expr), .., pos) => self
                .eval_expr(global, caches, scope, this_ptr, expr)
                .and_then(|v| Err(ERR::Return(v.flatten(), *pos).into())),

            // Empty return
            Stmt::Return(None, .., pos) => Err(ERR::Return(Dynamic::UNIT, *pos).into()),

            // Let/const statement - shadowing disallowed
            Stmt::Var(x, .., pos) if !self.allow_shadowing() && scope.contains(&x.0) => {
                Err(ERR::ErrorVariableExists(x.0.to_string(), *pos).into())
            }
            // Let/const statement
            Stmt::Var(x, options, pos) => {
                let (var_name, expr, index) = &**x;

                let access = if options.contains(ASTFlags::CONSTANT) {
                    AccessMode::ReadOnly
                } else {
                    AccessMode::ReadWrite
                };
                let export = options.contains(ASTFlags::EXPORTED);

                // Check variable definition filter
                if let Some(ref filter) = self.def_var_filter {
                    let will_shadow = scope.contains(var_name);
                    let is_const = access == AccessMode::ReadOnly;
                    let info = VarDefInfo {
                        name: var_name,
                        is_const,
                        nesting_level: global.scope_level,
                        will_shadow,
                    };
                    let context = EvalContext::new(self, global, caches, scope, this_ptr);

                    if !filter(true, info, context)? {
                        return Err(ERR::ErrorForbiddenVariable(var_name.to_string(), *pos).into());
                    }
                }

                // Evaluate initial value
                let mut value = self
                    .eval_expr(global, caches, scope, this_ptr, expr)?
                    .flatten();

                let _alias = if !rewind_scope {
                    // Put global constants into global module
                    #[cfg(not(feature = "no_function"))]
                    #[cfg(not(feature = "no_module"))]
                    if global.scope_level == 0
                        && access == AccessMode::ReadOnly
                        && global.lib.iter().any(|m| !m.is_empty())
                    {
                        crate::func::locked_write(global.constants.get_or_insert_with(|| {
                            crate::Shared::new(
                                crate::Locked::new(std::collections::BTreeMap::new()),
                            )
                        }))
                        .insert(var_name.name.clone(), value.clone());
                    }

                    if export {
                        Some(var_name)
                    } else {
                        None
                    }
                } else if export {
                    unreachable!("exported variable not on global level");
                } else {
                    None
                };

                if let Some(index) = index {
                    value.set_access_mode(access);
                    *scope.get_mut_by_index(scope.len() - index.get()) = value;
                } else {
                    scope.push_entry(var_name.name.clone(), access, value);
                }

                #[cfg(not(feature = "no_module"))]
                if let Some(alias) = _alias {
                    scope.add_alias_by_index(scope.len() - 1, alias.name.as_str().into());
                }

                Ok(Dynamic::UNIT)
            }

            // Import statement
            #[cfg(not(feature = "no_module"))]
            Stmt::Import(x, _pos) => {
                let (expr, export) = &**x;

                // Guard against too many modules
                if global.num_modules_loaded >= self.max_modules() {
                    return Err(ERR::ErrorTooManyModules(*_pos).into());
                }

                let v = self.eval_expr(global, caches, scope, this_ptr, expr)?;
                let typ = v.type_name();
                let path = v.try_cast::<crate::ImmutableString>().ok_or_else(|| {
                    self.make_type_mismatch_err::<crate::ImmutableString>(typ, expr.position())
                })?;

                use crate::ModuleResolver;

                let path_pos = expr.start_position();

                let resolver = global.embedded_module_resolver.clone();

                let module = resolver
                    .as_ref()
                    .and_then(|r| match r.resolve_raw(self, global, &path, path_pos) {
                        Err(err) if matches!(*err, ERR::ErrorModuleNotFound(..)) => None,
                        result => Some(result),
                    })
                    .or_else(|| {
                        Some(
                            self.module_resolver
                                .resolve_raw(self, global, &path, path_pos),
                        )
                    })
                    .unwrap_or_else(|| {
                        Err(ERR::ErrorModuleNotFound(path.to_string(), path_pos).into())
                    })?;

                let (export, must_be_indexed) = if !export.is_empty() {
                    (export.name.clone(), true)
                } else {
                    (self.const_empty_string(), false)
                };

                if !must_be_indexed || module.is_indexed() {
                    global.push_import(export, module);
                } else {
                    // Index the module (making a clone copy if necessary) if it is not indexed
                    let mut m = crate::func::shared_take_or_clone(module);
                    m.build_index();
                    global.push_import(export, m);
                }

                global.num_modules_loaded += 1;

                Ok(Dynamic::UNIT)
            }

            // Export statement
            #[cfg(not(feature = "no_module"))]
            Stmt::Export(x, ..) => {
                let (Ident { name, pos, .. }, Ident { name: alias, .. }) = &**x;
                // Mark scope variables as public
                if let Some(index) = scope.search(name) {
                    let alias = if alias.is_empty() { name } else { alias }.clone();
                    scope.add_alias_by_index(index, alias.into());
                    Ok(Dynamic::UNIT)
                } else {
                    Err(ERR::ErrorVariableNotFound(name.to_string(), *pos).into())
                }
            }

            // Share statement
            #[cfg(not(feature = "no_closure"))]
            Stmt::Share(x) => {
                x.iter()
                    .try_for_each(|(name, index, pos)| {
                        if let Some(index) = index
                            .map(|n| scope.len() - n.get())
                            .or_else(|| scope.search(name))
                        {
                            let val = scope.get_mut_by_index(index);

                            if !val.is_shared() {
                                // Replace the variable with a shared value.
                                *val = std::mem::take(val).into_shared();
                            }
                            Ok(())
                        } else {
                            Err(ERR::ErrorVariableNotFound(name.to_string(), *pos).into())
                        }
                    })
                    .map(|_| Dynamic::UNIT)
            }

            _ => unreachable!("statement cannot be evaluated: {:?}", stmt),
        }
    }

    /// Evaluate a list of statements with no `this` pointer.
    /// This is commonly used to evaluate a list of statements in an [`AST`][crate::AST] or a script function body.
    #[inline]
    pub(crate) fn eval_global_statements(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        scope: &mut Scope,
        statements: &[Stmt],
    ) -> RhaiResult {
        let mut this = Dynamic::NULL;

        self.eval_stmt_block(global, caches, scope, &mut this, statements, false)
            .or_else(|err| match *err {
                ERR::Return(out, ..) => Ok(out),
                ERR::LoopBreak(..) => {
                    unreachable!("no outer loop scope to break out of")
                }
                _ => Err(err),
            })
    }
}
