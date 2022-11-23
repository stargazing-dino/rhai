//! Types to support chaining operations (i.e. indexing and dotting).
#![cfg(any(not(feature = "no_index"), not(feature = "no_object")))]

use super::{Caches, GlobalRuntimeState, Target};
use crate::ast::{ASTFlags, Expr, OpAssignment};
use crate::types::dynamic::Union;
use crate::types::RestoreOnDrop;
use crate::{Dynamic, Engine, FnArgsVec, Position, RhaiResult, RhaiResultOf, Scope, ERR};
use std::hash::Hash;
#[cfg(feature = "no_std")]
use std::prelude::v1::*;

/// Method of chaining.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ChainType {
    /// Indexing.
    #[cfg(not(feature = "no_index"))]
    Indexing,
    /// Dotting.
    #[cfg(not(feature = "no_object"))]
    Dotting,
}

impl From<&Expr> for ChainType {
    #[inline(always)]
    fn from(expr: &Expr) -> Self {
        match expr {
            #[cfg(not(feature = "no_index"))]
            Expr::Index(..) => Self::Indexing,
            #[cfg(not(feature = "no_object"))]
            Expr::Dot(..) => Self::Dotting,
            expr => unreachable!("Expr::Index or Expr::Dot expected but gets {:?}", expr),
        }
    }
}

impl Engine {
    /// Call a get indexer.
    #[inline]
    fn call_indexer_get(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        target: &mut Dynamic,
        idx: &mut Dynamic,
        pos: Position,
    ) -> RhaiResultOf<Dynamic> {
        let args = &mut [target, idx];
        let hash = global.hash_idx_get();
        let fn_name = crate::engine::FN_IDX_GET;

        let orig_level = global.level;
        global.level += 1;
        let global = &mut *RestoreOnDrop::lock(global, move |g| g.level = orig_level);

        self.exec_native_fn_call(global, caches, fn_name, None, hash, args, true, pos)
            .map(|(r, ..)| r)
    }

    /// Call a set indexer.
    #[inline]
    fn call_indexer_set(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        target: &mut Dynamic,
        idx: &mut Dynamic,
        new_val: &mut Dynamic,
        is_ref_mut: bool,
        pos: Position,
    ) -> RhaiResultOf<(Dynamic, bool)> {
        let hash = global.hash_idx_set();
        let args = &mut [target, idx, new_val];
        let fn_name = crate::engine::FN_IDX_SET;

        let orig_level = global.level;
        global.level += 1;
        let global = &mut *RestoreOnDrop::lock(global, move |g| g.level = orig_level);

        self.exec_native_fn_call(global, caches, fn_name, None, hash, args, is_ref_mut, pos)
    }

    /// Get the value at the indexed position of a base type.
    fn get_indexed_mut<'t>(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        target: &'t mut Dynamic,
        idx: &mut Dynamic,
        idx_pos: Position,
        op_pos: Position,
        _add_if_not_found: bool,
        use_indexers: bool,
    ) -> RhaiResultOf<Target<'t>> {
        self.track_operation(global, Position::NONE)?;

        match target {
            #[cfg(not(feature = "no_index"))]
            Dynamic(Union::Array(arr, ..)) => {
                // val_array[idx]
                let index = idx
                    .as_int()
                    .map_err(|typ| self.make_type_mismatch_err::<crate::INT>(typ, idx_pos))?;
                let len = arr.len();
                let arr_idx = super::calc_index(len, index, true, || {
                    ERR::ErrorArrayBounds(len, index, idx_pos).into()
                })?;

                Ok(arr.get_mut(arr_idx).map(Target::from).unwrap())
            }

            #[cfg(not(feature = "no_index"))]
            Dynamic(Union::Blob(arr, ..)) => {
                // val_blob[idx]
                let index = idx
                    .as_int()
                    .map_err(|typ| self.make_type_mismatch_err::<crate::INT>(typ, idx_pos))?;
                let len = arr.len();
                let arr_idx = super::calc_index(len, index, true, || {
                    ERR::ErrorArrayBounds(len, index, idx_pos).into()
                })?;

                let value = arr.get(arr_idx).map(|&v| (v as crate::INT).into()).unwrap();

                Ok(Target::BlobByte {
                    source: target,
                    value,
                    index: arr_idx,
                })
            }

            #[cfg(not(feature = "no_object"))]
            Dynamic(Union::Map(map, ..)) => {
                // val_map[idx]
                let index = idx.read_lock::<crate::ImmutableString>().ok_or_else(|| {
                    self.make_type_mismatch_err::<crate::ImmutableString>(idx.type_name(), idx_pos)
                })?;

                if _add_if_not_found && (map.is_empty() || !map.contains_key(index.as_str())) {
                    map.insert(index.clone().into(), Dynamic::UNIT);
                }

                map.get_mut(index.as_str()).map_or_else(
                    || {
                        if self.fail_on_invalid_map_property() {
                            Err(ERR::ErrorPropertyNotFound(index.to_string(), idx_pos).into())
                        } else {
                            Ok(Target::from(Dynamic::UNIT))
                        }
                    },
                    |value| Ok(Target::from(value)),
                )
            }

            #[cfg(not(feature = "no_index"))]
            Dynamic(Union::Int(value, ..))
                if idx.is::<crate::ExclusiveRange>() || idx.is::<crate::InclusiveRange>() =>
            {
                // val_int[range]
                let (shift, mask) = if let Some(range) = idx.read_lock::<crate::ExclusiveRange>() {
                    let start = range.start;
                    let end = range.end;

                    let start = super::calc_index(crate::INT_BITS, start, false, || {
                        ERR::ErrorBitFieldBounds(crate::INT_BITS, start, idx_pos).into()
                    })?;
                    let end = super::calc_index(crate::INT_BITS, end, false, || {
                        ERR::ErrorBitFieldBounds(crate::INT_BITS, end, idx_pos).into()
                    })?;

                    #[allow(clippy::cast_possible_truncation)]
                    if end <= start {
                        (0, 0)
                    } else if end == crate::INT_BITS && start == 0 {
                        // -1 = all bits set
                        (0, -1)
                    } else {
                        (
                            start as u8,
                            // 2^bits - 1
                            (((2 as crate::UNSIGNED_INT).pow((end - start) as u32) - 1)
                                as crate::INT)
                                << start,
                        )
                    }
                } else if let Some(range) = idx.read_lock::<crate::InclusiveRange>() {
                    let start = *range.start();
                    let end = *range.end();

                    let start = super::calc_index(crate::INT_BITS, start, false, || {
                        ERR::ErrorBitFieldBounds(crate::INT_BITS, start, idx_pos).into()
                    })?;
                    let end = super::calc_index(crate::INT_BITS, end, false, || {
                        ERR::ErrorBitFieldBounds(crate::INT_BITS, end, idx_pos).into()
                    })?;

                    #[allow(clippy::cast_possible_truncation)]
                    if end < start {
                        (0, 0)
                    } else if end == crate::INT_BITS - 1 && start == 0 {
                        // -1 = all bits set
                        (0, -1)
                    } else {
                        (
                            start as u8,
                            // 2^bits - 1
                            (((2 as crate::UNSIGNED_INT).pow((end - start + 1) as u32) - 1)
                                as crate::INT)
                                << start,
                        )
                    }
                } else {
                    unreachable!("Range or RangeInclusive expected but gets {:?}", idx);
                };

                let field_value = (*value & mask) >> shift;

                Ok(Target::BitField {
                    source: target,
                    value: field_value.into(),
                    mask,
                    shift,
                })
            }

            #[cfg(not(feature = "no_index"))]
            Dynamic(Union::Int(value, ..)) => {
                // val_int[idx]
                let index = idx
                    .as_int()
                    .map_err(|typ| self.make_type_mismatch_err::<crate::INT>(typ, idx_pos))?;

                let bit = super::calc_index(crate::INT_BITS, index, true, || {
                    ERR::ErrorBitFieldBounds(crate::INT_BITS, index, idx_pos).into()
                })?;

                let bit_value = (*value & (1 << bit)) != 0;

                Ok(Target::Bit {
                    source: target,
                    value: bit_value.into(),
                    #[allow(clippy::cast_possible_truncation)]
                    bit: bit as u8,
                })
            }

            #[cfg(not(feature = "no_index"))]
            Dynamic(Union::Str(s, ..)) => {
                // val_string[idx]
                let index = idx
                    .as_int()
                    .map_err(|typ| self.make_type_mismatch_err::<crate::INT>(typ, idx_pos))?;

                let (ch, offset) = if index >= 0 {
                    #[allow(clippy::absurd_extreme_comparisons)]
                    if index >= crate::MAX_USIZE_INT {
                        return Err(
                            ERR::ErrorStringBounds(s.chars().count(), index, idx_pos).into()
                        );
                    }

                    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                    let offset = index as usize;
                    (
                        s.chars().nth(offset).ok_or_else(|| {
                            ERR::ErrorStringBounds(s.chars().count(), index, idx_pos)
                        })?,
                        offset,
                    )
                } else {
                    let abs_index = index.unsigned_abs();

                    if abs_index > usize::MAX as u64 {
                        return Err(
                            ERR::ErrorStringBounds(s.chars().count(), index, idx_pos).into()
                        );
                    }

                    #[allow(clippy::cast_possible_truncation)]
                    let offset = abs_index as usize;
                    (
                        // Count from end if negative
                        s.chars().rev().nth(offset - 1).ok_or_else(|| {
                            ERR::ErrorStringBounds(s.chars().count(), index, idx_pos)
                        })?,
                        offset,
                    )
                };

                Ok(Target::StringChar {
                    source: target,
                    value: ch.into(),
                    index: offset,
                })
            }

            #[cfg(not(feature = "no_closure"))]
            Dynamic(Union::Shared(..)) => {
                unreachable!("`get_indexed_mut` cannot handle shared values")
            }

            _ if use_indexers => self
                .call_indexer_get(global, caches, target, idx, op_pos)
                .map(Into::into),

            _ => Err(ERR::ErrorIndexingType(
                format!(
                    "{} [{}]",
                    self.map_type_name(target.type_name()),
                    self.map_type_name(idx.type_name())
                ),
                op_pos,
            )
            .into()),
        }
    }

    /// Evaluate a dot/index chain.
    pub(crate) fn eval_dot_index_chain(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        scope: &mut Scope,
        this_ptr: &mut Dynamic,
        expr: &Expr,
        new_val: Option<(Dynamic, &OpAssignment)>,
    ) -> RhaiResult {
        let chain_type = ChainType::from(expr);

        let crate::ast::BinaryExpr { lhs, rhs } = match expr {
            #[cfg(not(feature = "no_index"))]
            Expr::Index(x, ..) => &**x,
            #[cfg(not(feature = "no_object"))]
            Expr::Dot(x, ..) => &**x,
            expr => unreachable!("Expr::Index or Expr::Dot expected but gets {:?}", expr),
        };

        let idx_values = &mut FnArgsVec::new_const();

        match rhs {
            // Short-circuit for simple property access: {expr}.prop
            #[cfg(not(feature = "no_object"))]
            Expr::Property(..) if chain_type == ChainType::Dotting => (),
            #[cfg(not(feature = "no_object"))]
            Expr::Property(..) => unreachable!("unexpected Expr::Property for indexing"),
            // Short-circuit for simple method call: {expr}.func()
            #[cfg(not(feature = "no_object"))]
            Expr::FnCall(x, ..) if chain_type == ChainType::Dotting && x.args.is_empty() => (),
            // Short-circuit for method call with all literal arguments: {expr}.func(1, 2, 3)
            #[cfg(not(feature = "no_object"))]
            Expr::FnCall(x, ..)
                if chain_type == ChainType::Dotting && x.args.iter().all(Expr::is_constant) =>
            {
                idx_values.extend(x.args.iter().map(|expr| expr.get_literal_value().unwrap()))
            }
            // Short-circuit for indexing with literal: {expr}[1]
            #[cfg(not(feature = "no_index"))]
            _ if chain_type == ChainType::Indexing && rhs.is_constant() => {
                idx_values.push(rhs.get_literal_value().unwrap())
            }
            // All other patterns - evaluate the arguments chain
            _ => {
                self.eval_dot_index_chain_arguments(
                    global, caches, scope, this_ptr, expr, rhs, idx_values,
                )?;
            }
        }

        match lhs {
            // id.??? or id[???]
            Expr::Variable(.., var_pos) => {
                #[cfg(feature = "debugging")]
                self.run_debugger(global, caches, scope, this_ptr, lhs)?;
                self.track_operation(global, *var_pos)?;

                let mut target = self.search_namespace(global, caches, scope, this_ptr, lhs)?;

                let obj_ptr = &mut target;
                let mut this = Dynamic::NULL;

                self.eval_dot_index_chain_raw(
                    global, caches, &mut this, lhs, expr, obj_ptr, rhs, idx_values, new_val,
                )
            }
            // {expr}.??? = ??? or {expr}[???] = ???
            _ if new_val.is_some() => unreachable!("cannot assign to an expression"),
            // {expr}.??? or {expr}[???]
            lhs_expr => {
                let value = self
                    .eval_expr(global, caches, scope, this_ptr, lhs_expr)?
                    .flatten();
                let obj_ptr = &mut value.into();

                self.eval_dot_index_chain_raw(
                    global, caches, this_ptr, lhs_expr, expr, obj_ptr, rhs, idx_values, new_val,
                )
            }
        }
        .map(|(v, ..)| v)
    }

    /// Evaluate a chain of indexes and store the results in a [`FnArgsVec`].
    fn eval_dot_index_chain_arguments(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        scope: &mut Scope,
        this_ptr: &mut Dynamic,
        parent: &Expr,
        expr: &Expr,
        idx_values: &mut FnArgsVec<Dynamic>,
    ) -> RhaiResultOf<()> {
        self.track_operation(global, expr.position())?;

        let chain_type = ChainType::from(parent);

        match expr {
            #[cfg(not(feature = "no_object"))]
            Expr::MethodCall(x, ..) if chain_type == ChainType::Dotting && !x.is_qualified() => {
                for arg_expr in &x.args {
                    idx_values.push(
                        self.get_arg_value(global, caches, scope, this_ptr, arg_expr)?
                            .0
                            .flatten(),
                    );
                }
            }
            #[cfg(not(feature = "no_object"))]
            Expr::MethodCall(..) if chain_type == ChainType::Dotting => {
                unreachable!("function call in dot chain should not be namespace-qualified")
            }

            #[cfg(not(feature = "no_object"))]
            Expr::Property(..) if chain_type == ChainType::Dotting => (),
            Expr::Property(..) => unreachable!("unexpected Expr::Property for indexing"),

            Expr::Index(x, ..) | Expr::Dot(x, ..)
                if !parent.options().contains(ASTFlags::BREAK) =>
            {
                let crate::ast::BinaryExpr { lhs, rhs, .. } = &**x;

                let mut _arg_values = FnArgsVec::new_const();

                // Evaluate in left-to-right order
                match lhs {
                    #[cfg(not(feature = "no_object"))]
                    Expr::Property(..) if chain_type == ChainType::Dotting => (),
                    Expr::Property(..) => unreachable!("unexpected Expr::Property for indexing"),

                    #[cfg(not(feature = "no_object"))]
                    Expr::MethodCall(x, ..)
                        if chain_type == ChainType::Dotting && !x.is_qualified() =>
                    {
                        for arg_expr in &x.args {
                            _arg_values.push(
                                self.get_arg_value(global, caches, scope, this_ptr, arg_expr)?
                                    .0
                                    .flatten(),
                            );
                        }
                    }
                    #[cfg(not(feature = "no_object"))]
                    Expr::MethodCall(..) if chain_type == ChainType::Dotting => {
                        unreachable!("function call in dot chain should not be namespace-qualified")
                    }
                    #[cfg(not(feature = "no_object"))]
                    expr if chain_type == ChainType::Dotting => {
                        unreachable!("invalid dot expression: {:?}", expr);
                    }
                    #[cfg(not(feature = "no_index"))]
                    _ if chain_type == ChainType::Indexing => {
                        _arg_values.push(
                            self.eval_expr(global, caches, scope, this_ptr, lhs)?
                                .flatten(),
                        );
                    }
                    expr => unreachable!("unknown chained expression: {:?}", expr),
                }

                // Push in reverse order
                self.eval_dot_index_chain_arguments(
                    global, caches, scope, this_ptr, expr, rhs, idx_values,
                )?;

                if !_arg_values.is_empty() {
                    idx_values.extend(_arg_values);
                }
            }

            #[cfg(not(feature = "no_object"))]
            _ if chain_type == ChainType::Dotting => {
                unreachable!("invalid dot expression: {:?}", expr);
            }
            #[cfg(not(feature = "no_index"))]
            _ if chain_type == ChainType::Indexing => idx_values.push(
                self.eval_expr(global, caches, scope, this_ptr, expr)?
                    .flatten(),
            ),
            _ => unreachable!("unknown chained expression: {:?}", expr),
        }

        Ok(())
    }

    /// Chain-evaluate a dot/index chain.
    fn eval_dot_index_chain_raw(
        &self,
        global: &mut GlobalRuntimeState,
        caches: &mut Caches,
        this_ptr: &mut Dynamic,
        root: &Expr,
        parent: &Expr,
        target: &mut Target,
        rhs: &Expr,
        idx_values: &mut FnArgsVec<Dynamic>,
        new_val: Option<(Dynamic, &OpAssignment)>,
    ) -> RhaiResultOf<(Dynamic, bool)> {
        let is_ref_mut = target.is_ref();
        let op_pos = parent.position();

        #[cfg(feature = "debugging")]
        let scope = &mut Scope::new();

        match ChainType::from(parent) {
            #[cfg(not(feature = "no_index"))]
            ChainType::Indexing => {
                // Check for existence with the null conditional operator
                if parent.options().contains(ASTFlags::NEGATED) && target.is_unit() {
                    return Ok((Dynamic::UNIT, false));
                }

                let pos = rhs.start_position();

                match rhs {
                    // xxx[idx].expr... | xxx[idx][expr]...
                    Expr::Dot(x, ..) | Expr::Index(x, ..)
                        if !parent.options().contains(ASTFlags::BREAK) =>
                    {
                        #[cfg(feature = "debugging")]
                        self.run_debugger(global, caches, scope, this_ptr, parent)?;

                        let idx_val = &mut idx_values.pop().unwrap();
                        let mut idx_val_for_setter = idx_val.clone();
                        let idx_pos = x.lhs.start_position();

                        let (try_setter, result) = {
                            let mut obj = self.get_indexed_mut(
                                global, caches, target, idx_val, idx_pos, op_pos, false, true,
                            )?;
                            let is_obj_temp_val = obj.is_temp_value();
                            let obj_ptr = &mut obj;

                            match self.eval_dot_index_chain_raw(
                                global, caches, this_ptr, root, rhs, obj_ptr, &x.rhs, idx_values,
                                new_val,
                            ) {
                                Ok((result, true)) if is_obj_temp_val => {
                                    (Some(obj.take_or_clone()), (result, true))
                                }
                                Ok(result) => (None, result),
                                Err(err) => return Err(err),
                            }
                        };

                        if let Some(mut new_val) = try_setter {
                            // Try to call index setter if value is changed
                            let idx = &mut idx_val_for_setter;
                            let new_val = &mut new_val;
                            self.call_indexer_set(
                                global, caches, target, idx, new_val, is_ref_mut, op_pos,
                            )
                            .or_else(|e| match *e {
                                ERR::ErrorIndexingType(..) => Ok((Dynamic::UNIT, false)),
                                _ => Err(e),
                            })?;
                        }

                        Ok(result)
                    }
                    // xxx[rhs] op= new_val
                    _ if new_val.is_some() => {
                        #[cfg(feature = "debugging")]
                        self.run_debugger(global, caches, scope, this_ptr, parent)?;

                        let (new_val, op_info) = new_val.expect("`Some`");
                        let idx_val = &mut idx_values.pop().unwrap();
                        let idx = &mut idx_val.clone();

                        let try_setter = match self
                            .get_indexed_mut(global, caches, target, idx, pos, op_pos, true, false)
                        {
                            // Indexed value is not a temp value - update directly
                            Ok(ref mut obj_ptr) => {
                                self.eval_op_assignment(
                                    global, caches, op_info, root, obj_ptr, new_val,
                                )?;
                                self.check_data_size(obj_ptr.as_ref(), op_info.pos)?;
                                None
                            }
                            // Indexed value cannot be referenced - use indexer
                            #[cfg(not(feature = "no_index"))]
                            Err(err) if matches!(*err, ERR::ErrorIndexingType(..)) => Some(new_val),
                            // Any other error
                            Err(err) => return Err(err),
                        };

                        if let Some(mut new_val) = try_setter {
                            // Is this an op-assignment?
                            if op_info.is_op_assignment() {
                                let idx = &mut idx_val.clone();

                                // Call the index getter to get the current value
                                if let Ok(val) =
                                    self.call_indexer_get(global, caches, target, idx, op_pos)
                                {
                                    let mut val = val.into();
                                    // Run the op-assignment
                                    self.eval_op_assignment(
                                        global, caches, op_info, root, &mut val, new_val,
                                    )?;
                                    // Replace new value
                                    new_val = val.take_or_clone();
                                    self.check_data_size(&new_val, op_info.pos)?;
                                }
                            }

                            // Try to call index setter
                            let new_val = &mut new_val;

                            self.call_indexer_set(
                                global, caches, target, idx_val, new_val, is_ref_mut, op_pos,
                            )?;
                        }

                        Ok((Dynamic::UNIT, true))
                    }
                    // xxx[rhs]
                    _ => {
                        #[cfg(feature = "debugging")]
                        self.run_debugger(global, caches, scope, this_ptr, parent)?;

                        let idx_val = &mut idx_values.pop().unwrap();

                        self.get_indexed_mut(
                            global, caches, target, idx_val, pos, op_pos, false, true,
                        )
                        .map(|v| (v.take_or_clone(), false))
                    }
                }
            }

            #[cfg(not(feature = "no_object"))]
            ChainType::Dotting => {
                // Check for existence with the Elvis operator
                if parent.options().contains(ASTFlags::NEGATED) && target.is_unit() {
                    return Ok((Dynamic::UNIT, false));
                }

                match rhs {
                    // xxx.fn_name(arg_expr_list)
                    Expr::MethodCall(x, pos) if !x.is_qualified() && new_val.is_none() => {
                        #[cfg(feature = "debugging")]
                        let reset =
                            self.run_debugger_with_reset(global, caches, scope, this_ptr, rhs)?;
                        #[cfg(feature = "debugging")]
                        let global = &mut *RestoreOnDrop::lock(global, move |g| {
                            g.debugger.reset_status(reset)
                        });

                        let crate::ast::FnCallExpr {
                            name, hashes, args, ..
                        } = &**x;

                        // Truncate the index values upon exit
                        let offset = idx_values.len() - args.len();
                        let idx_values =
                            &mut *RestoreOnDrop::lock(idx_values, move |v| v.truncate(offset));

                        let call_args = &mut idx_values[offset..];
                        let arg1_pos = args.get(0).map_or(Position::NONE, Expr::position);

                        self.make_method_call(
                            global, caches, name, *hashes, target, call_args, arg1_pos, *pos,
                        )
                    }
                    // xxx.fn_name(...) = ???
                    Expr::MethodCall(..) if new_val.is_some() => {
                        unreachable!("method call cannot be assigned to")
                    }
                    // xxx.module::fn_name(...) - syntax error
                    Expr::MethodCall(..) => {
                        unreachable!("function call in dot chain should not be namespace-qualified")
                    }
                    // {xxx:map}.id op= ???
                    Expr::Property(x, pos) if new_val.is_some() && target.is_map() => {
                        #[cfg(feature = "debugging")]
                        self.run_debugger(global, caches, scope, this_ptr, rhs)?;

                        let index = &mut x.2.clone().into();
                        let (new_val, op_info) = new_val.expect("`Some`");
                        {
                            let val_target = &mut self.get_indexed_mut(
                                global, caches, target, index, *pos, op_pos, true, false,
                            )?;
                            self.eval_op_assignment(
                                global, caches, op_info, root, val_target, new_val,
                            )?;
                        }
                        self.check_data_size(target.source(), op_info.pos)?;
                        Ok((Dynamic::UNIT, true))
                    }
                    // {xxx:map}.id
                    Expr::Property(x, pos) if target.is_map() => {
                        #[cfg(feature = "debugging")]
                        self.run_debugger(global, caches, scope, this_ptr, rhs)?;

                        let index = &mut x.2.clone().into();
                        let val = self.get_indexed_mut(
                            global, caches, target, index, *pos, op_pos, false, false,
                        )?;
                        Ok((val.take_or_clone(), false))
                    }
                    // xxx.id op= ???
                    Expr::Property(x, pos) if new_val.is_some() => {
                        #[cfg(feature = "debugging")]
                        self.run_debugger(global, caches, scope, this_ptr, rhs)?;

                        let ((getter, hash_get), (setter, hash_set), name) = &**x;
                        let (mut new_val, op_info) = new_val.expect("`Some`");

                        if op_info.is_op_assignment() {
                            let args = &mut [target.as_mut()];
                            let (mut orig_val, ..) = self
                                .exec_native_fn_call(
                                    global, caches, getter, None, *hash_get, args, is_ref_mut, *pos,
                                )
                                .or_else(|err| match *err {
                                    // Try an indexer if property does not exist
                                    ERR::ErrorDotExpr(..) => {
                                        let mut prop = name.into();
                                        self.call_indexer_get(
                                            global, caches, target, &mut prop, op_pos,
                                        )
                                        .map(|r| (r, false))
                                        .map_err(|e| {
                                            match *e {
                                                ERR::ErrorIndexingType(..) => err,
                                                _ => e,
                                            }
                                        })
                                    }
                                    _ => Err(err),
                                })?;

                            {
                                let orig_val = &mut (&mut orig_val).into();

                                self.eval_op_assignment(
                                    global, caches, op_info, root, orig_val, new_val,
                                )?;
                            }

                            new_val = orig_val;
                        }

                        let args = &mut [target.as_mut(), &mut new_val];
                        self.exec_native_fn_call(
                            global, caches, setter, None, *hash_set, args, is_ref_mut, *pos,
                        )
                        .or_else(|err| match *err {
                            // Try an indexer if property does not exist
                            ERR::ErrorDotExpr(..) => {
                                let idx = &mut name.into();
                                let new_val = &mut new_val;
                                self.call_indexer_set(
                                    global, caches, target, idx, new_val, is_ref_mut, op_pos,
                                )
                                .map_err(|e| match *e {
                                    ERR::ErrorIndexingType(..) => err,
                                    _ => e,
                                })
                            }
                            _ => Err(err),
                        })
                    }
                    // xxx.id
                    Expr::Property(x, pos) => {
                        #[cfg(feature = "debugging")]
                        self.run_debugger(global, caches, scope, this_ptr, rhs)?;

                        let ((getter, hash_get), _, name) = &**x;
                        let args = &mut [target.as_mut()];
                        self.exec_native_fn_call(
                            global, caches, getter, None, *hash_get, args, is_ref_mut, *pos,
                        )
                        .map_or_else(
                            |err| match *err {
                                // Try an indexer if property does not exist
                                ERR::ErrorDotExpr(..) => {
                                    let mut prop = name.into();
                                    self.call_indexer_get(global, caches, target, &mut prop, op_pos)
                                        .map(|r| (r, false))
                                        .map_err(|e| match *e {
                                            ERR::ErrorIndexingType(..) => err,
                                            _ => e,
                                        })
                                }
                                _ => Err(err),
                            },
                            // Assume getters are always pure
                            |(v, ..)| Ok((v, false)),
                        )
                    }
                    // {xxx:map}.sub_lhs[expr] | {xxx:map}.sub_lhs.expr
                    Expr::Index(x, ..) | Expr::Dot(x, ..) if target.is_map() => {
                        let _node = &x.lhs;

                        let val_target = &mut match x.lhs {
                            Expr::Property(ref p, pos) => {
                                #[cfg(feature = "debugging")]
                                self.run_debugger(global, caches, scope, this_ptr, _node)?;

                                let index = &mut p.2.clone().into();
                                self.get_indexed_mut(
                                    global, caches, target, index, pos, op_pos, false, true,
                                )?
                            }
                            // {xxx:map}.fn_name(arg_expr_list)[expr] | {xxx:map}.fn_name(arg_expr_list).expr
                            Expr::MethodCall(ref x, pos) if !x.is_qualified() => {
                                #[cfg(feature = "debugging")]
                                let reset = self.run_debugger_with_reset(
                                    global, caches, scope, this_ptr, _node,
                                )?;
                                #[cfg(feature = "debugging")]
                                let global = &mut *RestoreOnDrop::lock(global, move |g| {
                                    g.debugger.reset_status(reset)
                                });

                                let crate::ast::FnCallExpr {
                                    name, hashes, args, ..
                                } = &**x;

                                // Truncate the index values upon exit
                                let offset = idx_values.len() - args.len();
                                let idx_values = &mut *RestoreOnDrop::lock(idx_values, move |v| {
                                    v.truncate(offset)
                                });

                                let call_args = &mut idx_values[offset..];
                                let arg1_pos = args.get(0).map_or(Position::NONE, Expr::position);

                                self.make_method_call(
                                    global, caches, name, *hashes, target, call_args, arg1_pos, pos,
                                )?
                                .0
                                .into()
                            }
                            // {xxx:map}.module::fn_name(...) - syntax error
                            Expr::MethodCall(..) => unreachable!(
                                "function call in dot chain should not be namespace-qualified"
                            ),
                            // Others - syntax error
                            ref expr => unreachable!("invalid dot expression: {:?}", expr),
                        };

                        self.eval_dot_index_chain_raw(
                            global, caches, this_ptr, root, rhs, val_target, &x.rhs, idx_values,
                            new_val,
                        )
                    }
                    // xxx.sub_lhs[expr] | xxx.sub_lhs.expr
                    Expr::Index(x, ..) | Expr::Dot(x, ..) => {
                        let _node = &x.lhs;

                        match x.lhs {
                            // xxx.prop[expr] | xxx.prop.expr
                            Expr::Property(ref p, pos) => {
                                #[cfg(feature = "debugging")]
                                self.run_debugger(global, caches, scope, this_ptr, _node)?;

                                let ((getter, hash_get), (setter, hash_set), name) = &**p;
                                let mut arg_values = [target.as_mut(), &mut Dynamic::UNIT.clone()];
                                let args = &mut arg_values[..1];

                                // Assume getters are always pure
                                let (mut val, ..) = self
                                    .exec_native_fn_call(
                                        global, caches, getter, None, *hash_get, args, is_ref_mut,
                                        pos,
                                    )
                                    .or_else(|err| match *err {
                                        // Try an indexer if property does not exist
                                        ERR::ErrorDotExpr(..) => {
                                            let mut prop = name.into();
                                            self.call_indexer_get(
                                                global, caches, target, &mut prop, op_pos,
                                            )
                                            .map(|r| (r, false))
                                            .map_err(
                                                |e| match *e {
                                                    ERR::ErrorIndexingType(..) => err,
                                                    _ => e,
                                                },
                                            )
                                        }
                                        _ => Err(err),
                                    })?;

                                let val = &mut (&mut val).into();

                                let (result, may_be_changed) = self.eval_dot_index_chain_raw(
                                    global, caches, this_ptr, root, rhs, val, &x.rhs, idx_values,
                                    new_val,
                                )?;

                                // Feed the value back via a setter just in case it has been updated
                                if may_be_changed {
                                    // Re-use args because the first &mut parameter will not be consumed
                                    let mut arg_values = [target.as_mut(), val.as_mut()];
                                    let args = &mut arg_values;
                                    self.exec_native_fn_call(
                                        global, caches, setter, None, *hash_set, args, is_ref_mut,
                                        pos,
                                    )
                                    .or_else(
                                        |err| match *err {
                                            // Try an indexer if property does not exist
                                            ERR::ErrorDotExpr(..) => {
                                                let idx = &mut name.into();
                                                let new_val = val;
                                                self.call_indexer_set(
                                                    global, caches, target, idx, new_val,
                                                    is_ref_mut, op_pos,
                                                )
                                                .or_else(|e| match *e {
                                                    // If there is no setter, no need to feed it
                                                    // back because the property is read-only
                                                    ERR::ErrorIndexingType(..) => {
                                                        Ok((Dynamic::UNIT, false))
                                                    }
                                                    _ => Err(e),
                                                })
                                            }
                                            _ => Err(err),
                                        },
                                    )?;
                                }

                                Ok((result, may_be_changed))
                            }
                            // xxx.fn_name(arg_expr_list)[expr] | xxx.fn_name(arg_expr_list).expr
                            Expr::MethodCall(ref f, pos) if !f.is_qualified() => {
                                let val = {
                                    #[cfg(feature = "debugging")]
                                    let reset = self.run_debugger_with_reset(
                                        global, caches, scope, this_ptr, _node,
                                    )?;
                                    #[cfg(feature = "debugging")]
                                    let global = &mut *RestoreOnDrop::lock(global, move |g| {
                                        g.debugger.reset_status(reset)
                                    });

                                    let crate::ast::FnCallExpr {
                                        name, hashes, args, ..
                                    } = &**f;

                                    // Truncate the index values upon exit
                                    let offset = idx_values.len() - args.len();
                                    let idx_values =
                                        &mut *RestoreOnDrop::lock(idx_values, move |v| {
                                            v.truncate(offset)
                                        });

                                    let call_args = &mut idx_values[offset..];
                                    let pos1 = args.get(0).map_or(Position::NONE, Expr::position);

                                    self.make_method_call(
                                        global, caches, name, *hashes, target, call_args, pos1, pos,
                                    )?
                                    .0
                                };

                                let val = &mut val.into();

                                self.eval_dot_index_chain_raw(
                                    global, caches, this_ptr, root, rhs, val, &x.rhs, idx_values,
                                    new_val,
                                )
                            }
                            // xxx.module::fn_name(...) - syntax error
                            Expr::MethodCall(..) => unreachable!(
                                "function call in dot chain should not be namespace-qualified"
                            ),
                            // Others - syntax error
                            ref expr => unreachable!("invalid dot expression: {:?}", expr),
                        }
                    }
                    // Syntax error
                    expr => unreachable!("invalid chaining expression: {:?}", expr),
                }
            }
        }
    }
}
