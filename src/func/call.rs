//! Implement function-calling mechanism for [`Engine`].

use super::{get_builtin_binary_op_fn, get_builtin_op_assignment_fn, CallableFunction};
use crate::api::default_limits::MAX_DYNAMIC_PARAMETERS;
use crate::ast::{Expr, FnCallExpr, FnCallHashes};
use crate::engine::{
    KEYWORD_DEBUG, KEYWORD_EVAL, KEYWORD_FN_PTR, KEYWORD_FN_PTR_CALL, KEYWORD_FN_PTR_CURRY,
    KEYWORD_IS_DEF_VAR, KEYWORD_PRINT, KEYWORD_TYPE_OF,
};
use crate::eval::{Caches, FnResolutionCacheEntry, GlobalRuntimeState};
use crate::tokenizer::{is_valid_function_name, Token};
use crate::types::RestoreOnDrop;
use crate::{
    calc_fn_hash, calc_fn_hash_full, Dynamic, Engine, FnArgsVec, FnPtr, ImmutableString,
    OptimizationLevel, Position, RhaiError, RhaiResult, RhaiResultOf, Scope, SharedModule, ERR,
};
#[cfg(feature = "no_std")]
use hashbrown::hash_map::Entry;
#[cfg(not(feature = "no_std"))]
use std::collections::hash_map::Entry;
#[cfg(feature = "no_std")]
use std::prelude::v1::*;
use std::{
    any::{type_name, TypeId},
    convert::TryFrom,
    mem,
};

/// Arguments to a function call, which is a list of [`&mut Dynamic`][Dynamic].
pub type FnCallArgs<'a> = [&'a mut Dynamic];

/// A type that temporarily stores a mutable reference to a `Dynamic`,
/// replacing it with a cloned copy.
#[derive(Debug)]
struct ArgBackup<'a> {
    orig_mut: Option<&'a mut Dynamic>,
    value_copy: Dynamic,
}

impl<'a> ArgBackup<'a> {
    /// Create a new `ArgBackup`.
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            orig_mut: None,
            value_copy: Dynamic::UNIT,
        }
    }
    /// This function replaces the first argument of a method call with a clone copy.
    /// This is to prevent a pure function unintentionally consuming the first argument.
    ///
    /// `restore_first_arg` must be called before the end of the scope to prevent the shorter
    /// lifetime from leaking.
    ///
    /// # Safety
    ///
    /// This method blindly casts a reference to another lifetime, which saves allocation and
    /// string cloning.
    ///
    /// As long as `restore_first_arg` is called before the end of the scope, the shorter lifetime
    /// will not leak.
    ///
    /// # Panics
    ///
    /// Panics when `args` is empty.
    #[inline(always)]
    pub fn change_first_arg_to_copy(&mut self, args: &mut FnCallArgs<'a>) {
        // Clone the original value.
        self.value_copy = args[0].clone();

        // Replace the first reference with a reference to the clone, force-casting the lifetime.
        // Must remember to restore it later with `restore_first_arg`.
        //
        // SAFETY:
        //
        // Blindly casting a reference to another lifetime saves allocation and string cloning,
        // but must be used with the utmost care.
        //
        // We can do this here because, before the end of this scope, we'd restore the original
        // reference via `restore_first_arg`. Therefore this shorter lifetime does not leak.
        self.orig_mut = Some(mem::replace(&mut args[0], unsafe {
            mem::transmute(&mut self.value_copy)
        }));
    }
    /// This function restores the first argument that was replaced by `change_first_arg_to_copy`.
    ///
    /// # Safety
    ///
    /// If `change_first_arg_to_copy` has been called, this function **MUST** be called _BEFORE_
    /// exiting the current scope.  Otherwise it is undefined behavior as the shorter lifetime will leak.
    #[inline(always)]
    pub fn restore_first_arg(&mut self, args: &mut FnCallArgs<'a>) {
        if let Some(p) = self.orig_mut.take() {
            args[0] = p;
        } else {
            unreachable!("`Some`");
        }
    }
}

impl Drop for ArgBackup<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        // Panic if the shorter lifetime leaks.
        assert!(
            self.orig_mut.is_none(),
            "ArgBackup::restore_first_arg has not been called prior to existing this scope"
        );
    }
}

// Ensure no data races in function call arguments.
#[cfg(not(feature = "no_closure"))]
#[inline]
pub fn ensure_no_data_race(fn_name: &str, args: &FnCallArgs, is_ref_mut: bool) -> RhaiResultOf<()> {
    if let Some((n, ..)) = args
        .iter()
        .enumerate()
        .skip(if is_ref_mut { 1 } else { 0 })
        .find(|(.., a)| a.is_locked())
    {
        return Err(ERR::ErrorDataRace(
            format!("argument #{} of function '{fn_name}'", n + 1),
            Position::NONE,
        )
        .into());
    }

    Ok(())
}

/// Is a function name an anonymous function?
#[cfg(not(feature = "no_function"))]
#[inline]
#[must_use]
pub fn is_anonymous_fn(name: &str) -> bool {
    name.starts_with(crate::engine::FN_ANONYMOUS)
}

impl Engine {
    /// Generate the signature for a function call.
    #[inline]
    #[must_use]
    fn gen_fn_call_signature(&self, fn_name: &str, args: &[&mut Dynamic]) -> String {
        format!(
            "{fn_name} ({})",
            args.iter()
                .map(|a| if a.is::<ImmutableString>() {
                    "&str | ImmutableString | String"
                } else {
                    self.map_type_name(a.type_name())
                })
                .collect::<FnArgsVec<_>>()
                .join(", ")
        )
    }

    /// Resolve a normal (non-qualified) function call.
    ///
    /// Search order:
    /// 1) AST - script functions in the AST
    /// 2) Global namespace - functions registered via `Engine::register_XXX`
    /// 3) Global registered modules - packages
    /// 4) Imported modules - functions marked with global namespace
    /// 5) Static registered modules
    #[must_use]
    fn resolve_fn<'s>(
        &self,
        _global: &GlobalRuntimeState,
        caches: &'s mut Caches,
        local_entry: &'s mut Option<FnResolutionCacheEntry>,
        lib: &[SharedModule],
        op_token: Option<&Token>,
        hash_base: u64,
        args: Option<&mut FnCallArgs>,
        allow_dynamic: bool,
    ) -> Option<&'s FnResolutionCacheEntry> {
        if hash_base == 0 {
            return None;
        }

        let mut hash = args.as_ref().map_or(hash_base, |args| {
            calc_fn_hash_full(hash_base, args.iter().map(|a| a.type_id()))
        });

        let cache = caches.fn_resolution_cache_mut();

        match cache.map.entry(hash) {
            Entry::Occupied(entry) => entry.into_mut().as_ref(),
            Entry::Vacant(entry) => {
                let num_args = args.as_ref().map_or(0, |a| a.len());
                let mut max_bitmask = 0; // One above maximum bitmask based on number of parameters.
                                         // Set later when a specific matching function is not found.
                let mut bitmask = 1usize; // Bitmask of which parameter to replace with `Dynamic`

                loop {
                    let func = lib
                        .iter()
                        .chain(self.global_modules.iter())
                        .find_map(|m| m.get_fn(hash).map(|f| (f, m.id_raw())));

                    #[cfg(not(feature = "no_module"))]
                    let func = if args.is_none() {
                        // Scripted functions are not exposed globally
                        func
                    } else {
                        func.or_else(|| _global.get_qualified_fn(hash)).or_else(|| {
                            self.global_sub_modules
                                .values()
                                .find_map(|m| m.get_qualified_fn(hash).map(|f| (f, m.id_raw())))
                        })
                    };

                    if let Some((f, s)) = func {
                        // Specific version found
                        let new_entry = Some(FnResolutionCacheEntry {
                            func: f.clone(),
                            source: s.cloned(),
                        });
                        return if cache.filter.is_absent_and_set(hash) {
                            // Do not cache "one-hit wonders"
                            *local_entry = new_entry;
                            local_entry.as_ref()
                        } else {
                            // Cache entry
                            entry.insert(new_entry).as_ref()
                        };
                    }

                    // Check `Dynamic` parameters for functions with parameters
                    if allow_dynamic && max_bitmask == 0 && num_args > 0 {
                        let is_dynamic = lib.iter().any(|m| m.may_contain_dynamic_fn(hash_base))
                            || self
                                .global_modules
                                .iter()
                                .any(|m| m.may_contain_dynamic_fn(hash_base));

                        #[cfg(not(feature = "no_module"))]
                        let is_dynamic = is_dynamic
                            || _global.may_contain_dynamic_fn(hash_base)
                            || self
                                .global_sub_modules
                                .values()
                                .any(|m| m.may_contain_dynamic_fn(hash_base));

                        // Set maximum bitmask when there are dynamic versions of the function
                        if is_dynamic {
                            max_bitmask = 1usize << usize::min(num_args, MAX_DYNAMIC_PARAMETERS);
                        }
                    }

                    // Stop when all permutations are exhausted
                    if bitmask >= max_bitmask {
                        if num_args != 2 {
                            return None;
                        }

                        // Try to find a built-in version
                        let builtin =
                            args.and_then(|args| match op_token {
                                Some(token) if token.is_op_assignment() => {
                                    let (first_arg, rest_args) = args.split_first().unwrap();

                                    get_builtin_op_assignment_fn(token, *first_arg, rest_args[0])
                                        .map(|f| FnResolutionCacheEntry {
                                            func: CallableFunction::from_fn_builtin(f),
                                            source: None,
                                        })
                                }
                                Some(token) => get_builtin_binary_op_fn(token, args[0], args[1])
                                    .map(|f| FnResolutionCacheEntry {
                                        func: CallableFunction::from_fn_builtin(f),
                                        source: None,
                                    }),

                                None => None,
                            });

                        return if cache.filter.is_absent_and_set(hash) {
                            // Do not cache "one-hit wonders"
                            *local_entry = builtin;
                            local_entry.as_ref()
                        } else {
                            // Cache entry
                            entry.insert(builtin).as_ref()
                        };
                    }

                    // Try all permutations with `Dynamic` wildcards
                    hash = calc_fn_hash_full(
                        hash_base,
                        args.as_ref()
                            .expect("no permutations")
                            .iter()
                            .enumerate()
                            .map(|(i, a)| {
                                let mask = 1usize << (num_args - i - 1);
                                if bitmask & mask == 0 {
                                    a.type_id()
                                } else {
                                    // Replace with `Dynamic`
                                    TypeId::of::<Dynamic>()
                                }
                            }),
                    );

                    bitmask += 1;
                }
            }
        }
    }

    /// # Main Entry-Point
    ///
    /// Call a native Rust function registered with the [`Engine`].
    ///
    /// # WARNING
    ///
    /// Function call arguments be _consumed_ when the function requires them to be passed by value.
    /// All function arguments not in the first position are always passed by value and thus consumed.
    ///
    /// **DO NOT** reuse the argument values unless for the first `&mut` argument -
    /// all others are silently replaced by `()`!
    pub(crate) fn exec_native_fn_call(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        lib: &[SharedModule],
        name: &str,
        op_token: Option<&Token>,
        hash: u64,
        mut args: &mut FnCallArgs,
        is_ref_mut: bool,
        pos: Position,
    ) -> RhaiResultOf<(Dynamic, bool)> {
        self.track_operation(global, pos)?;

        // Check if function access already in the cache
        let local_entry = &mut None;

        let func = self.resolve_fn(
            global,
            caches,
            local_entry,
            lib,
            op_token,
            hash,
            Some(args),
            true,
        );

        if func.is_some() {
            let is_method = func.map_or(false, |f| f.func.is_method());

            // Push a new call stack frame
            #[cfg(feature = "debugging")]
            let orig_call_stack_len = global.debugger.call_stack().len();

            let FnResolutionCacheEntry { func, source } = func.unwrap();
            assert!(func.is_native());

            let backup = &mut ArgBackup::new();

            // Calling pure function but the first argument is a reference?
            let swap = is_ref_mut && func.is_pure() && !args.is_empty();

            if swap {
                // Clone the first argument
                backup.change_first_arg_to_copy(args);
            }

            let args =
                &mut *RestoreOnDrop::lock_if(swap, &mut args, move |a| backup.restore_first_arg(a));

            #[cfg(feature = "debugging")]
            if self.debugger.is_some() {
                global.debugger.push_call_stack_frame(
                    self.get_interned_string(name),
                    args.iter().map(|v| (*v).clone()).collect(),
                    source.clone().or_else(|| global.source.clone()),
                    pos,
                );
            }

            // Run external function
            let src = source.as_ref().map(|s| s.as_str());
            let context = (self, name, src, &*global, lib, pos).into();

            let mut _result = if func.is_plugin_fn() {
                let f = func.get_plugin_fn().unwrap();
                if !f.is_pure() && !args.is_empty() && args[0].is_read_only() {
                    Err(ERR::ErrorNonPureMethodCallOnConstant(name.to_string(), pos).into())
                } else {
                    f.call(context, args)
                }
            } else {
                func.get_native_fn().unwrap()(context, args)
            };

            #[cfg(feature = "debugging")]
            {
                let trigger = match global.debugger.status {
                    crate::eval::DebuggerStatus::FunctionExit(n) => n >= global.level,
                    crate::eval::DebuggerStatus::Next(.., true) => true,
                    _ => false,
                };
                if trigger {
                    let scope = &mut &mut Scope::new();
                    let mut this = Dynamic::NULL;
                    let node = crate::ast::Stmt::Noop(pos);
                    let node = (&node).into();
                    let event = match _result {
                        Ok(ref r) => crate::eval::DebuggerEvent::FunctionExitWithValue(r),
                        Err(ref err) => crate::eval::DebuggerEvent::FunctionExitWithError(err),
                    };

                    if let Err(err) =
                        self.run_debugger_raw(global, caches, lib, scope, &mut this, node, event)
                    {
                        _result = Err(err);
                    }
                }

                // Pop the call stack
                global.debugger.rewind_call_stack(orig_call_stack_len);
            }

            // Check the return value (including data sizes)
            let result = self.check_return_value(_result, pos)?;

            // Check the data size of any `&mut` object, which may be changed.
            #[cfg(not(feature = "unchecked"))]
            if is_ref_mut && !args.is_empty() {
                self.check_data_size(args[0], pos)?;
            }

            // See if the function match print/debug (which requires special processing)
            return Ok(match name {
                KEYWORD_PRINT => {
                    let text = result.into_immutable_string().map_err(|typ| {
                        let t = self.map_type_name(type_name::<ImmutableString>()).into();
                        ERR::ErrorMismatchOutputType(t, typ.into(), pos)
                    })?;
                    ((*self.print)(&text).into(), false)
                }
                KEYWORD_DEBUG => {
                    let text = result.into_immutable_string().map_err(|typ| {
                        let t = self.map_type_name(type_name::<ImmutableString>()).into();
                        ERR::ErrorMismatchOutputType(t, typ.into(), pos)
                    })?;
                    ((*self.debug)(&text, global.source(), pos).into(), false)
                }
                _ => (result, is_method),
            });
        }

        // Error handling

        match name {
            // index getter function not found?
            #[cfg(any(not(feature = "no_index"), not(feature = "no_object")))]
            crate::engine::FN_IDX_GET => {
                assert!(args.len() == 2);

                let t0 = self.map_type_name(args[0].type_name());
                let t1 = self.map_type_name(args[1].type_name());

                Err(ERR::ErrorIndexingType(format!("{t0} [{t1}]"), pos).into())
            }

            // index setter function not found?
            #[cfg(any(not(feature = "no_index"), not(feature = "no_object")))]
            crate::engine::FN_IDX_SET => {
                assert!(args.len() == 3);

                let t0 = self.map_type_name(args[0].type_name());
                let t1 = self.map_type_name(args[1].type_name());
                let t2 = self.map_type_name(args[2].type_name());

                Err(ERR::ErrorIndexingType(format!("{t0} [{t1}] = {t2}"), pos).into())
            }

            // Getter function not found?
            #[cfg(not(feature = "no_object"))]
            _ if name.starts_with(crate::engine::FN_GET) => {
                assert!(args.len() == 1);

                let prop = &name[crate::engine::FN_GET.len()..];
                let t0 = self.map_type_name(args[0].type_name());

                Err(ERR::ErrorDotExpr(
                    format!(
                        "Unknown property '{prop}' - a getter is not registered for type '{t0}'"
                    ),
                    pos,
                )
                .into())
            }

            // Setter function not found?
            #[cfg(not(feature = "no_object"))]
            _ if name.starts_with(crate::engine::FN_SET) => {
                assert!(args.len() == 2);

                let prop = &name[crate::engine::FN_SET.len()..];
                let t0 = self.map_type_name(args[0].type_name());
                let t1 = self.map_type_name(args[1].type_name());

                Err(ERR::ErrorDotExpr(
                    format!(
                        "No writable property '{prop}' - a setter is not registered for type '{t0}' to handle '{t1}'"
                    ),
                    pos,
                )
                .into())
            }

            // Raise error
            _ => {
                Err(ERR::ErrorFunctionNotFound(self.gen_fn_call_signature(name, args), pos).into())
            }
        }
    }

    /// # Main Entry-Point
    ///
    /// Perform an actual function call, native Rust or scripted, taking care of special functions.
    ///
    /// # WARNING
    ///
    /// Function call arguments may be _consumed_ when the function requires them to be passed by
    /// value. All function arguments not in the first position are always passed by value and thus consumed.
    ///
    /// **DO NOT** reuse the argument values unless for the first `&mut` argument -
    /// all others are silently replaced by `()`!
    pub(crate) fn exec_fn_call(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        lib: &[SharedModule],
        _scope: Option<&mut Scope>,
        fn_name: &str,
        op_token: Option<&Token>,
        hashes: FnCallHashes,
        mut args: &mut FnCallArgs,
        is_ref_mut: bool,
        _is_method_call: bool,
        pos: Position,
    ) -> RhaiResultOf<(Dynamic, bool)> {
        fn no_method_err(name: &str, pos: Position) -> RhaiResultOf<(Dynamic, bool)> {
            Err(ERR::ErrorRuntime(
                format!("'{name}' should not be called this way. Try {name}(...);").into(),
                pos,
            )
            .into())
        }

        // Check for data race.
        #[cfg(not(feature = "no_closure"))]
        ensure_no_data_race(fn_name, args, is_ref_mut)?;

        global.level += 1;
        let global = &mut *RestoreOnDrop::lock(global, move |g| g.level -= 1);

        // These may be redirected from method style calls.
        if hashes.is_native_only() {
            match fn_name {
                // Handle type_of()
                KEYWORD_TYPE_OF if args.len() == 1 => {
                    let typ = self.map_type_name(args[0].type_name()).to_string().into();
                    return Ok((typ, false));
                }

                // Handle is_def_fn()
                #[cfg(not(feature = "no_function"))]
                crate::engine::KEYWORD_IS_DEF_FN
                    if args.len() == 2 && args[0].is::<FnPtr>() && args[1].is::<crate::INT>() =>
                {
                    let fn_name = args[0].read_lock::<ImmutableString>().expect("`FnPtr`");
                    let num_params = args[1].as_int().expect("`INT`");

                    return Ok((
                        if num_params < 0 || num_params > crate::MAX_USIZE_INT {
                            false
                        } else {
                            let hash_script =
                                calc_fn_hash(None, fn_name.as_str(), num_params as usize);
                            self.has_script_fn(global, caches, lib, hash_script)
                        }
                        .into(),
                        false,
                    ));
                }

                // Handle is_shared()
                #[cfg(not(feature = "no_closure"))]
                crate::engine::KEYWORD_IS_SHARED if args.len() == 1 => {
                    return no_method_err(fn_name, pos)
                }

                KEYWORD_FN_PTR | KEYWORD_EVAL | KEYWORD_IS_DEF_VAR if args.len() == 1 => {
                    return no_method_err(fn_name, pos)
                }

                KEYWORD_FN_PTR_CALL | KEYWORD_FN_PTR_CURRY if !args.is_empty() => {
                    return no_method_err(fn_name, pos)
                }

                _ => (),
            }
        }

        #[cfg(not(feature = "no_function"))]
        if !hashes.is_native_only() {
            // Script-defined function call?
            let hash = hashes.script();
            let local_entry = &mut None;

            if let Some(FnResolutionCacheEntry { func, ref source }) = self
                .resolve_fn(global, caches, local_entry, lib, None, hash, None, false)
                .cloned()
            {
                // Script function call
                assert!(func.is_script());

                let func = func.get_script_fn_def().expect("script-defined function");

                if func.body.is_empty() {
                    return Ok((Dynamic::UNIT, false));
                }

                let mut empty_scope;
                let scope = match _scope {
                    Some(scope) => scope,
                    None => {
                        empty_scope = Scope::new();
                        &mut empty_scope
                    }
                };

                let orig_source = mem::replace(&mut global.source, source.clone());
                let global = &mut *RestoreOnDrop::lock(global, move |g| g.source = orig_source);

                return if _is_method_call {
                    // Method call of script function - map first argument to `this`
                    let (first_arg, rest_args) = args.split_first_mut().unwrap();

                    self.call_script_fn(
                        global, caches, lib, scope, first_arg, func, rest_args, true, pos,
                    )
                } else {
                    // Normal call of script function
                    let backup = &mut ArgBackup::new();

                    // The first argument is a reference?
                    let swap = is_ref_mut && !args.is_empty();

                    if swap {
                        backup.change_first_arg_to_copy(args);
                    }

                    let args = &mut *RestoreOnDrop::lock_if(swap, &mut args, move |a| {
                        backup.restore_first_arg(a)
                    });

                    let mut this = Dynamic::NULL;

                    self.call_script_fn(
                        global, caches, lib, scope, &mut this, func, args, true, pos,
                    )
                }
                .map(|r| (r, false));
            }
        }

        // Native function call
        let hash = hashes.native();

        self.exec_native_fn_call(
            global, caches, lib, fn_name, op_token, hash, args, is_ref_mut, pos,
        )
    }

    /// Evaluate an argument.
    #[inline]
    pub(crate) fn get_arg_value(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        lib: &[SharedModule],
        scope: &mut Scope,
        this_ptr: &mut Dynamic,
        arg_expr: &Expr,
    ) -> RhaiResultOf<(Dynamic, Position)> {
        // Literal values
        if let Some(value) = arg_expr.get_literal_value() {
            self.track_operation(global, arg_expr.start_position())?;

            #[cfg(feature = "debugging")]
            self.run_debugger(global, caches, lib, scope, this_ptr, arg_expr)?;

            return Ok((value, arg_expr.start_position()));
        }

        // Do not match function exit for arguments
        #[cfg(feature = "debugging")]
        let reset = global.debugger.clear_status_if(|status| {
            matches!(status, crate::eval::DebuggerStatus::FunctionExit(..))
        });
        #[cfg(feature = "debugging")]
        let global = &mut *RestoreOnDrop::lock(global, move |g| g.debugger.reset_status(reset));

        self.eval_expr(global, caches, lib, scope, this_ptr, arg_expr)
            .map(|r| (r, arg_expr.start_position()))
    }

    /// Call a dot method.
    #[cfg(not(feature = "no_object"))]
    pub(crate) fn make_method_call(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        lib: &[SharedModule],
        fn_name: &str,
        mut hash: FnCallHashes,
        target: &mut crate::eval::Target,
        mut call_args: &mut [Dynamic],
        first_arg_pos: Position,
        fn_call_pos: Position,
    ) -> RhaiResultOf<(Dynamic, bool)> {
        let is_ref_mut = target.is_ref();

        let (result, updated) = match fn_name {
            KEYWORD_FN_PTR_CALL if target.is::<FnPtr>() => {
                // FnPtr call
                let fn_ptr = target.read_lock::<FnPtr>().expect("`FnPtr`");

                #[cfg(not(feature = "no_function"))]
                let is_anon = fn_ptr.is_anonymous();
                #[cfg(feature = "no_function")]
                let is_anon = false;

                // Redirect function name
                let fn_name = fn_ptr.fn_name();
                // Recalculate hashes
                let args_len = call_args.len() + fn_ptr.curry().len();
                let new_hash = if !is_anon && !is_valid_function_name(fn_name) {
                    FnCallHashes::from_native(calc_fn_hash(None, fn_name, args_len))
                } else {
                    calc_fn_hash(None, fn_name, args_len).into()
                };
                // Arguments are passed as-is, adding the curried arguments
                let mut curry = FnArgsVec::with_capacity(fn_ptr.curry().len());
                curry.extend(fn_ptr.curry().iter().cloned());
                let mut args = FnArgsVec::with_capacity(curry.len() + call_args.len());
                args.extend(curry.iter_mut());
                args.extend(call_args.iter_mut());

                // Map it to name(args) in function-call style
                self.exec_fn_call(
                    global,
                    caches,
                    lib,
                    None,
                    fn_name,
                    None,
                    new_hash,
                    &mut args,
                    false,
                    false,
                    fn_call_pos,
                )
            }
            KEYWORD_FN_PTR_CALL => {
                if call_args.is_empty() {
                    let typ = self.map_type_name(target.type_name());
                    return Err(self.make_type_mismatch_err::<FnPtr>(typ, fn_call_pos));
                } else if !call_args[0].is::<FnPtr>() {
                    let typ = self.map_type_name(call_args[0].type_name());
                    return Err(self.make_type_mismatch_err::<FnPtr>(typ, first_arg_pos));
                }

                // FnPtr call on object
                let fn_ptr = mem::take(&mut call_args[0]).cast::<FnPtr>();

                #[cfg(not(feature = "no_function"))]
                let is_anon = fn_ptr.is_anonymous();
                #[cfg(feature = "no_function")]
                let is_anon = false;

                call_args = &mut call_args[1..];

                // Redirect function name
                let (fn_name, fn_curry) = fn_ptr.take_data();
                // Recalculate hash
                let args_len = call_args.len() + fn_curry.len();
                let new_hash = if !is_anon && !is_valid_function_name(&fn_name) {
                    FnCallHashes::from_native(calc_fn_hash(None, &fn_name, args_len + 1))
                } else {
                    FnCallHashes::from_all(
                        #[cfg(not(feature = "no_function"))]
                        calc_fn_hash(None, &fn_name, args_len),
                        calc_fn_hash(None, &fn_name, args_len + 1),
                    )
                };
                // Replace the first argument with the object pointer, adding the curried arguments
                let mut curry = FnArgsVec::with_capacity(fn_curry.len());
                curry.extend(fn_curry.into_iter());
                let mut args = FnArgsVec::with_capacity(curry.len() + call_args.len() + 1);
                args.push(target.as_mut());
                args.extend(curry.iter_mut());
                args.extend(call_args.iter_mut());

                // Map it to name(args) in function-call style
                self.exec_fn_call(
                    global,
                    caches,
                    lib,
                    None,
                    &fn_name,
                    None,
                    new_hash,
                    &mut args,
                    is_ref_mut,
                    true,
                    fn_call_pos,
                )
            }
            KEYWORD_FN_PTR_CURRY => {
                if !target.is::<FnPtr>() {
                    let typ = self.map_type_name(target.type_name());
                    return Err(self.make_type_mismatch_err::<FnPtr>(typ, fn_call_pos));
                }

                let fn_ptr = target.read_lock::<FnPtr>().expect("`FnPtr`");

                // Curry call
                Ok((
                    if call_args.is_empty() {
                        fn_ptr.clone()
                    } else {
                        FnPtr::new_unchecked(
                            fn_ptr.fn_name_raw().clone(),
                            fn_ptr
                                .curry()
                                .iter()
                                .cloned()
                                .chain(call_args.iter_mut().map(mem::take))
                                .collect(),
                        )
                    }
                    .into(),
                    false,
                ))
            }

            // Handle is_shared()
            #[cfg(not(feature = "no_closure"))]
            crate::engine::KEYWORD_IS_SHARED if call_args.is_empty() => {
                return Ok((target.is_shared().into(), false));
            }

            _ => {
                let mut fn_name = fn_name;
                let _redirected;
                let mut _arg_values: FnArgsVec<_>;
                let mut call_args = call_args;

                // Check if it is a map method call in OOP style
                #[cfg(not(feature = "no_object"))]
                if let Some(map) = target.read_lock::<crate::Map>() {
                    if let Some(val) = map.get(fn_name) {
                        if let Some(fn_ptr) = val.read_lock::<FnPtr>() {
                            #[cfg(not(feature = "no_function"))]
                            let is_anon = fn_ptr.is_anonymous();
                            #[cfg(feature = "no_function")]
                            let is_anon = false;

                            // Remap the function name
                            _redirected = fn_ptr.fn_name_raw().clone();
                            fn_name = &_redirected;
                            // Add curried arguments
                            if fn_ptr.is_curried() {
                                _arg_values = fn_ptr
                                    .curry()
                                    .iter()
                                    .cloned()
                                    .chain(call_args.iter_mut().map(mem::take))
                                    .collect();
                                call_args = &mut _arg_values;
                            }
                            // Recalculate the hash based on the new function name and new arguments
                            hash = if !is_anon && !is_valid_function_name(&fn_name) {
                                FnCallHashes::from_native(calc_fn_hash(
                                    None,
                                    fn_name,
                                    call_args.len() + 1,
                                ))
                            } else {
                                FnCallHashes::from_all(
                                    #[cfg(not(feature = "no_function"))]
                                    calc_fn_hash(None, fn_name, call_args.len()),
                                    calc_fn_hash(None, fn_name, call_args.len() + 1),
                                )
                            };
                        }
                    }
                };

                // Attached object pointer in front of the arguments
                let mut args = FnArgsVec::with_capacity(call_args.len() + 1);
                args.push(target.as_mut());
                args.extend(call_args.iter_mut());

                self.exec_fn_call(
                    global,
                    caches,
                    lib,
                    None,
                    fn_name,
                    None,
                    hash,
                    &mut args,
                    is_ref_mut,
                    true,
                    fn_call_pos,
                )
            }
        }?;

        // Propagate the changed value back to the source if necessary
        if updated {
            target.propagate_changed_value(fn_call_pos)?;
        }

        Ok((result, updated))
    }

    /// Call a function in normal function-call style.
    pub(crate) fn make_function_call(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        lib: &[SharedModule],
        scope: &mut Scope,
        this_ptr: &mut Dynamic,
        fn_name: &str,
        op_token: Option<&Token>,
        first_arg: Option<&Expr>,
        args_expr: &[Expr],
        hashes: FnCallHashes,
        capture_scope: bool,
        pos: Position,
    ) -> RhaiResult {
        let mut first_arg = first_arg;
        let mut a_expr = args_expr;
        let mut total_args = if first_arg.is_some() { 1 } else { 0 } + a_expr.len();
        let mut curry = FnArgsVec::new_const();
        let mut name = fn_name;
        let mut hashes = hashes;
        let redirected; // Handle call() - Redirect function call

        match name {
            _ if op_token.is_some() => (),

            // Handle call()
            KEYWORD_FN_PTR_CALL if total_args >= 1 => {
                let arg = first_arg.unwrap();
                let (arg_value, arg_pos) =
                    self.get_arg_value(global, caches, lib, scope, this_ptr, arg)?;

                if !arg_value.is::<FnPtr>() {
                    let typ = self.map_type_name(arg_value.type_name());
                    return Err(self.make_type_mismatch_err::<FnPtr>(typ, arg_pos));
                }

                let fn_ptr = arg_value.cast::<FnPtr>();

                #[cfg(not(feature = "no_function"))]
                let is_anon = fn_ptr.is_anonymous();
                #[cfg(feature = "no_function")]
                let is_anon = false;

                let (fn_name, fn_curry) = fn_ptr.take_data();
                curry.extend(fn_curry.into_iter());

                // Redirect function name
                redirected = fn_name;
                name = &redirected;

                // Shift the arguments
                first_arg = a_expr.get(0);
                if !a_expr.is_empty() {
                    a_expr = &a_expr[1..];
                }
                total_args -= 1;

                // Recalculate hash
                let args_len = total_args + curry.len();

                hashes = if !is_anon && !is_valid_function_name(name) {
                    FnCallHashes::from_native(calc_fn_hash(None, name, args_len))
                } else {
                    calc_fn_hash(None, name, args_len).into()
                };
            }
            // Handle Fn()
            KEYWORD_FN_PTR if total_args == 1 => {
                let arg = first_arg.unwrap();
                let (arg_value, arg_pos) =
                    self.get_arg_value(global, caches, lib, scope, this_ptr, arg)?;

                // Fn - only in function call style
                return arg_value
                    .into_immutable_string()
                    .map_err(|typ| self.make_type_mismatch_err::<ImmutableString>(typ, arg_pos))
                    .and_then(FnPtr::try_from)
                    .map(Into::into)
                    .map_err(|err| err.fill_position(arg_pos));
            }

            // Handle curry()
            KEYWORD_FN_PTR_CURRY if total_args > 1 => {
                let first = first_arg.unwrap();
                let (arg_value, arg_pos) =
                    self.get_arg_value(global, caches, lib, scope, this_ptr, first)?;

                if !arg_value.is::<FnPtr>() {
                    let typ = self.map_type_name(arg_value.type_name());
                    return Err(self.make_type_mismatch_err::<FnPtr>(typ, arg_pos));
                }

                let (name, fn_curry) = arg_value.cast::<FnPtr>().take_data();

                // Append the new curried arguments to the existing list.
                let fn_curry = a_expr.iter().try_fold(fn_curry, |mut curried, expr| {
                    let (value, ..) =
                        self.get_arg_value(global, caches, lib, scope, this_ptr, expr)?;
                    curried.push(value);
                    Ok::<_, RhaiError>(curried)
                })?;

                return Ok(FnPtr::new_unchecked(name, fn_curry).into());
            }

            // Handle is_shared()
            #[cfg(not(feature = "no_closure"))]
            crate::engine::KEYWORD_IS_SHARED if total_args == 1 => {
                let arg = first_arg.unwrap();
                let (arg_value, ..) =
                    self.get_arg_value(global, caches, lib, scope, this_ptr, arg)?;
                return Ok(arg_value.is_shared().into());
            }

            // Handle is_def_fn()
            #[cfg(not(feature = "no_function"))]
            crate::engine::KEYWORD_IS_DEF_FN if total_args == 2 => {
                let first = first_arg.unwrap();
                let (arg_value, arg_pos) =
                    self.get_arg_value(global, caches, lib, scope, this_ptr, first)?;

                let fn_name = arg_value
                    .into_immutable_string()
                    .map_err(|typ| self.make_type_mismatch_err::<ImmutableString>(typ, arg_pos))?;

                let (arg_value, arg_pos) =
                    self.get_arg_value(global, caches, lib, scope, this_ptr, &a_expr[0])?;

                let num_params = arg_value
                    .as_int()
                    .map_err(|typ| self.make_type_mismatch_err::<crate::INT>(typ, arg_pos))?;

                return Ok(if num_params < 0 || num_params > crate::MAX_USIZE_INT {
                    false
                } else {
                    let hash_script = calc_fn_hash(None, &fn_name, num_params as usize);
                    self.has_script_fn(global, caches, lib, hash_script)
                }
                .into());
            }

            // Handle is_def_var()
            KEYWORD_IS_DEF_VAR if total_args == 1 => {
                let arg = first_arg.unwrap();
                let (arg_value, arg_pos) =
                    self.get_arg_value(global, caches, lib, scope, this_ptr, arg)?;
                let var_name = arg_value
                    .into_immutable_string()
                    .map_err(|typ| self.make_type_mismatch_err::<ImmutableString>(typ, arg_pos))?;
                return Ok(scope.contains(&var_name).into());
            }

            // Handle eval()
            KEYWORD_EVAL if total_args == 1 => {
                // eval - only in function call style
                let orig_scope_len = scope.len();
                #[cfg(not(feature = "no_module"))]
                let orig_imports_len = global.num_imports();
                let arg = first_arg.unwrap();
                let (arg_value, pos) =
                    self.get_arg_value(global, caches, lib, scope, this_ptr, arg)?;
                let s = &arg_value
                    .into_immutable_string()
                    .map_err(|typ| self.make_type_mismatch_err::<ImmutableString>(typ, pos))?;

                global.level += 1;
                let global = &mut *RestoreOnDrop::lock(global, move |g| g.level -= 1);

                let result = self.eval_script_expr_in_place(global, caches, lib, scope, s, pos);

                // IMPORTANT! If the eval defines new variables in the current scope,
                //            all variable offsets from this point on will be mis-aligned.
                //            The same is true for imports.
                let scope_changed = scope.len() != orig_scope_len;
                #[cfg(not(feature = "no_module"))]
                let scope_changed = scope_changed || global.num_imports() != orig_imports_len;

                if scope_changed {
                    global.always_search_scope = true;
                }

                return result.map_err(|err| {
                    ERR::ErrorInFunctionCall(
                        KEYWORD_EVAL.to_string(),
                        global.source().unwrap_or("").to_string(),
                        err,
                        pos,
                    )
                    .into()
                });
            }

            _ => (),
        }

        // Normal function call - except for Fn, curry, call and eval (handled above)
        let mut arg_values = FnArgsVec::with_capacity(total_args);
        let mut args = FnArgsVec::with_capacity(total_args + curry.len());
        let mut is_ref_mut = false;

        // Capture parent scope?
        //
        // If so, do it separately because we cannot convert the first argument (if it is a simple
        // variable access) to &mut because `scope` is needed.
        if capture_scope && !scope.is_empty() {
            first_arg
                .iter()
                .copied()
                .chain(a_expr.iter())
                .try_for_each(|expr| {
                    self.get_arg_value(global, caches, lib, scope, this_ptr, expr)
                        .map(|(value, ..)| arg_values.push(value.flatten()))
                })?;
            args.extend(curry.iter_mut());
            args.extend(arg_values.iter_mut());

            // Use parent scope
            let scope = Some(scope);

            return self
                .exec_fn_call(
                    global, caches, lib, scope, name, op_token, hashes, &mut args, is_ref_mut,
                    false, pos,
                )
                .map(|(v, ..)| v);
        }

        // Call with blank scope
        if total_args == 0 && curry.is_empty() {
            // No arguments
        } else {
            // If the first argument is a variable, and there is no curried arguments,
            // convert to method-call style in order to leverage potential &mut first argument and
            // avoid cloning the value
            if curry.is_empty() && first_arg.map_or(false, |expr| expr.is_variable_access(false)) {
                let first_expr = first_arg.unwrap();

                #[cfg(feature = "debugging")]
                self.run_debugger(global, caches, lib, scope, this_ptr, first_expr)?;

                // func(x, ...) -> x.func(...)
                a_expr.iter().try_for_each(|expr| {
                    self.get_arg_value(global, caches, lib, scope, this_ptr, expr)
                        .map(|(value, ..)| arg_values.push(value.flatten()))
                })?;

                let (mut target, _pos) =
                    self.search_namespace(global, caches, lib, scope, this_ptr, first_expr)?;

                if target.is_read_only() {
                    target = target.into_owned();
                }

                self.track_operation(global, _pos)?;

                #[cfg(not(feature = "no_closure"))]
                let target_is_shared = target.is_shared();
                #[cfg(feature = "no_closure")]
                let target_is_shared = false;

                if target_is_shared || target.is_temp_value() {
                    arg_values.insert(0, target.take_or_clone().flatten());
                } else {
                    // Turn it into a method call only if the object is not shared and not a simple value
                    is_ref_mut = true;
                    let obj_ref = target.take_ref().expect("ref");
                    args.push(obj_ref);
                }
            } else {
                // func(..., ...)
                first_arg
                    .into_iter()
                    .chain(a_expr.iter())
                    .try_for_each(|expr| {
                        self.get_arg_value(global, caches, lib, scope, this_ptr, expr)
                            .map(|(value, ..)| arg_values.push(value.flatten()))
                    })?;
                args.extend(curry.iter_mut());
            }

            args.extend(arg_values.iter_mut());
        }

        self.exec_fn_call(
            global, caches, lib, None, name, op_token, hashes, &mut args, is_ref_mut, false, pos,
        )
        .map(|(v, ..)| v)
    }

    /// Call a namespace-qualified function in normal function-call style.
    #[cfg(not(feature = "no_module"))]
    pub(crate) fn make_qualified_function_call(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        lib: &[SharedModule],
        scope: &mut Scope,
        this_ptr: &mut Dynamic,
        namespace: &crate::ast::Namespace,
        fn_name: &str,
        args_expr: &[Expr],
        hash: u64,
        pos: Position,
    ) -> RhaiResult {
        let mut arg_values = FnArgsVec::with_capacity(args_expr.len());
        let mut args = FnArgsVec::with_capacity(args_expr.len());
        let mut first_arg_value = None;

        if args_expr.is_empty() {
            // No arguments
        } else {
            // See if the first argument is a variable (not namespace-qualified).
            // If so, convert to method-call style in order to leverage potential &mut first argument
            // and avoid cloning the value
            if !args_expr.is_empty() && args_expr[0].is_variable_access(true) {
                #[cfg(feature = "debugging")]
                self.run_debugger(global, caches, lib, scope, this_ptr, &args_expr[0])?;

                // func(x, ...) -> x.func(...)
                arg_values.push(Dynamic::UNIT);

                args_expr.iter().skip(1).try_for_each(|expr| {
                    self.get_arg_value(global, caches, lib, scope, this_ptr, expr)
                        .map(|(value, ..)| arg_values.push(value.flatten()))
                })?;

                // Get target reference to first argument
                let first_arg = &args_expr[0];
                let (target, _pos) =
                    self.search_scope_only(global, caches, lib, scope, this_ptr, first_arg)?;

                self.track_operation(global, _pos)?;

                #[cfg(not(feature = "no_closure"))]
                let target_is_shared = target.is_shared();
                #[cfg(feature = "no_closure")]
                let target_is_shared = false;

                if target_is_shared || target.is_temp_value() {
                    arg_values[0] = target.take_or_clone().flatten();
                    args.extend(arg_values.iter_mut());
                } else {
                    // Turn it into a method call only if the object is not shared and not a simple value
                    let (first, rest) = arg_values.split_first_mut().unwrap();
                    first_arg_value = Some(first);
                    let obj_ref = target.take_ref().expect("ref");
                    args.push(obj_ref);
                    args.extend(rest.iter_mut());
                }
            } else {
                // func(..., ...) or func(mod::x, ...)
                args_expr.iter().try_for_each(|expr| {
                    self.get_arg_value(global, caches, lib, scope, this_ptr, expr)
                        .map(|(value, ..)| arg_values.push(value.flatten()))
                })?;
                args.extend(arg_values.iter_mut());
            }
        }

        // Search for the root namespace
        let module = self
            .search_imports(global, namespace)
            .ok_or_else(|| ERR::ErrorModuleNotFound(namespace.to_string(), namespace.position()))?;

        // First search script-defined functions in namespace (can override built-in)
        let mut func = match module.get_qualified_fn(hash) {
            // Then search native Rust functions
            None => {
                self.track_operation(global, pos)?;
                let hash_qualified_fn = calc_fn_hash_full(hash, args.iter().map(|a| a.type_id()));
                module.get_qualified_fn(hash_qualified_fn)
            }
            r => r,
        };

        // Check for `Dynamic` parameters.
        //
        // Note - This is done during every function call mismatch without cache,
        //        so hopefully the number of arguments should not be too many
        //        (expected because closures cannot be qualified).
        if func.is_none() && !args.is_empty() {
            let num_args = args.len();
            let max_bitmask = 1usize << usize::min(num_args, MAX_DYNAMIC_PARAMETERS);
            let mut bitmask = 1usize; // Bitmask of which parameter to replace with `Dynamic`

            // Try all permutations with `Dynamic` wildcards
            while bitmask < max_bitmask {
                let hash_qualified_fn = calc_fn_hash_full(
                    hash,
                    args.iter().enumerate().map(|(i, a)| {
                        let mask = 1usize << (num_args - i - 1);
                        if bitmask & mask == 0 {
                            a.type_id()
                        } else {
                            // Replace with `Dynamic`
                            TypeId::of::<Dynamic>()
                        }
                    }),
                );

                self.track_operation(global, pos)?;

                if let Some(f) = module.get_qualified_fn(hash_qualified_fn) {
                    func = Some(f);
                    break;
                }

                bitmask += 1;
            }
        }

        // Clone first argument if the function is not a method after-all
        if !func.map_or(true, CallableFunction::is_method) {
            if let Some(first) = first_arg_value {
                *first = args[0].clone();
                args[0] = first;
            }
        }

        global.level += 1;
        let global = &mut *RestoreOnDrop::lock(global, move |g| g.level -= 1);

        match func {
            #[cfg(not(feature = "no_function"))]
            Some(f) if f.is_script() => {
                let fn_def = f.get_script_fn_def().expect("script-defined function");
                let new_scope = &mut Scope::new();
                let mut this = Dynamic::NULL;

                let orig_source = mem::replace(&mut global.source, module.id_raw().cloned());
                let global = &mut *RestoreOnDrop::lock(global, move |g| g.source = orig_source);

                self.call_script_fn(
                    global, caches, lib, new_scope, &mut this, fn_def, &mut args, true, pos,
                )
            }

            Some(f) if f.is_plugin_fn() => {
                let context = (self, fn_name, module.id(), &*global, lib, pos).into();
                let f = f.get_plugin_fn().expect("plugin function");
                let result = if !f.is_pure() && !args.is_empty() && args[0].is_read_only() {
                    Err(ERR::ErrorNonPureMethodCallOnConstant(fn_name.to_string(), pos).into())
                } else {
                    f.call(context, &mut args)
                };
                self.check_return_value(result, pos)
            }

            Some(f) if f.is_native() => {
                let func = f.get_native_fn().expect("native function");
                let context = (self, fn_name, module.id(), &*global, lib, pos).into();
                let result = func(context, &mut args);
                self.check_return_value(result, pos)
            }

            Some(f) => unreachable!("unknown function type: {:?}", f),

            None => {
                let sig = if namespace.is_empty() {
                    self.gen_fn_call_signature(fn_name, &args)
                } else {
                    format!(
                        "{namespace}{}{}",
                        crate::tokenizer::Token::DoubleColon.literal_syntax(),
                        self.gen_fn_call_signature(fn_name, &args)
                    )
                };

                Err(ERR::ErrorFunctionNotFound(sig, pos).into())
            }
        }
    }

    /// Evaluate a text script in place - used primarily for 'eval'.
    pub(crate) fn eval_script_expr_in_place(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        lib: &[SharedModule],
        scope: &mut Scope,
        script: &str,
        _pos: Position,
    ) -> RhaiResult {
        self.track_operation(global, _pos)?;

        let script = script.trim();

        if script.is_empty() {
            return Ok(Dynamic::UNIT);
        }

        // Compile the script text
        // No optimizations because we only run it once
        let ast = self.compile_with_scope_and_optimization_level(
            &Scope::new(),
            &[script],
            #[cfg(not(feature = "no_optimize"))]
            OptimizationLevel::None,
            #[cfg(feature = "no_optimize")]
            OptimizationLevel::default(),
        )?;

        // If new functions are defined within the eval string, it is an error
        #[cfg(not(feature = "no_function"))]
        if !ast.shared_lib().is_empty() {
            return Err(crate::PERR::WrongFnDefinition.into());
        }

        let statements = ast.statements();
        if statements.is_empty() {
            return Ok(Dynamic::UNIT);
        }

        // Evaluate the AST
        self.eval_global_statements(global, caches, lib, scope, statements)
    }

    /// Evaluate a function call expression.
    pub(crate) fn eval_fn_call_expr(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        lib: &[SharedModule],
        scope: &mut Scope,
        this_ptr: &mut Dynamic,
        expr: &FnCallExpr,
        pos: Position,
    ) -> RhaiResult {
        let FnCallExpr {
            #[cfg(not(feature = "no_module"))]
            namespace,
            name,
            hashes,
            args,
            op_token,
            capture_parent_scope: capture,
            ..
        } = expr;

        let op_token = op_token.as_ref();

        // Short-circuit native binary operator call if under Fast Operators mode
        if op_token.is_some() && self.fast_operators() && args.len() == 2 {
            let mut lhs = self
                .get_arg_value(global, caches, lib, scope, this_ptr, &args[0])?
                .0
                .flatten();

            let mut rhs = self
                .get_arg_value(global, caches, lib, scope, this_ptr, &args[1])?
                .0
                .flatten();

            let operands = &mut [&mut lhs, &mut rhs];

            if let Some(func) =
                get_builtin_binary_op_fn(op_token.as_ref().unwrap(), operands[0], operands[1])
            {
                // Built-in found
                global.level += 1;
                let global = &*RestoreOnDrop::lock(global, move |g| g.level -= 1);

                let context = (self, name.as_str(), None, global, lib, pos).into();
                return func(context, operands);
            }

            return self
                .exec_fn_call(
                    global, caches, lib, None, name, op_token, *hashes, operands, false, false, pos,
                )
                .map(|(v, ..)| v);
        }

        #[cfg(not(feature = "no_module"))]
        if !namespace.is_empty() {
            // Qualified function call
            let hash = hashes.native();

            return self.make_qualified_function_call(
                global, caches, lib, scope, this_ptr, namespace, name, args, hash, pos,
            );
        }

        // Normal function call
        let (first_arg, args) = args.split_first().map_or_else(
            || (None, args.as_ref()),
            |(first, rest)| (Some(first), rest),
        );

        self.make_function_call(
            global, caches, lib, scope, this_ptr, name, op_token, first_arg, args, *hashes,
            *capture, pos,
        )
    }
}
