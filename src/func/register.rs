//! Module which defines the function registration mechanism.

#![allow(non_snake_case)]
#![allow(unused_imports)]
#![allow(unused_mut)]
#![allow(unused_variables)]

use super::call::FnCallArgs;
use super::callable_function::CallableFunction;
use super::native::{SendSync, Shared};
use crate::engine::{FN_IDX_SET, FN_SET};
use crate::types::dynamic::{DynamicWriteLock, Variant};
use crate::{reify, Dynamic, NativeCallContext, RhaiResultOf};
#[cfg(feature = "no_std")]
use std::prelude::v1::*;
use std::{
    any::{type_name, TypeId},
    mem,
};

/// These types are used to build a unique _marker_ tuple type for each combination
/// of function parameter types in order to make each trait implementation unique.
///
/// That is because stable Rust currently does not allow distinguishing implementations
/// based purely on parameter types of traits (`Fn`, `FnOnce` and `FnMut`).
///
/// # Examples
///
/// `RegisterNativeFunction<(Mut<A>, B, Ref<C>), 3, false, R, false>` = `Fn(&mut A, B, &C) -> R`
///
/// `RegisterNativeFunction<(Mut<A>, B, Ref<C>), 3, true,  R, false>` = `Fn(NativeCallContext, &mut A, B, &C) -> R`
///
/// `RegisterNativeFunction<(Mut<A>, B, Ref<C>), 3, false, R, true>`  = `Fn(&mut A, B, &C) -> Result<R, Box<EvalAltResult>>`
///
/// `RegisterNativeFunction<(Mut<A>, B, Ref<C>), 3, true,  R, true>`  = `Fn(NativeCallContext, &mut A, B, &C) -> Result<R, Box<EvalAltResult>>`
///
/// These types are not actually used anywhere.
pub struct Mut<T>(T);
//pub struct Ref<T>(T);

/// Dereference into [`DynamicWriteLock`]
#[inline(always)]
#[must_use]
pub fn by_ref<T: Variant + Clone>(data: &mut Dynamic) -> DynamicWriteLock<T> {
    // Directly cast the &mut Dynamic into DynamicWriteLock to access the underlying data.
    data.write_lock::<T>().expect("checked")
}

/// Dereference into value.
#[inline(always)]
#[must_use]
pub fn by_value<T: Variant + Clone>(data: &mut Dynamic) -> T {
    if TypeId::of::<T>() == TypeId::of::<&str>() {
        // If T is `&str`, data must be `ImmutableString`, so map directly to it
        data.flatten_in_place();
        let ref_str = data.as_str_ref().expect("&str");
        // SAFETY: We already checked that `T` is `&str`, so it is safe to cast here.
        return unsafe { mem::transmute_copy::<_, T>(&ref_str) };
    }
    if TypeId::of::<T>() == TypeId::of::<String>() {
        // If T is `String`, data must be `ImmutableString`, so map directly to it
        return reify!(mem::take(data).into_string().expect("`ImmutableString`") => T);
    }

    // We consume the argument and then replace it with () - the argument is not supposed to be used again.
    // This way, we avoid having to clone the argument again, because it is already a clone when passed here.
    mem::take(data).cast::<T>()
}

/// Trait to register custom Rust functions.
///
/// # Type Parameters
///
/// * `ARGS` - a tuple containing parameter types, with `&mut T` represented by `Mut<T>`.
/// * `NUM` - a constant generic containing the number of parameters, must be consistent with `ARGS`.
/// * `CTX` - a constant boolean generic indicating whether there is a `NativeCallContext` parameter.
/// * `RET` - return type of the function; if the function returns `Result`, it is the unwrapped inner value type.
/// * `FALL` - a constant boolean generic indicating whether the function is fallible (i.e. returns `Result<T, Box<EvalAltResult>>`).
pub trait RegisterNativeFunction<ARGS, const NUM: usize, const CTX: bool, RET, const FALL: bool> {
    /// Convert this function into a [`CallableFunction`].
    #[must_use]
    fn into_callable_function(self) -> CallableFunction;
    /// Get the type ID's of this function's parameters.
    #[must_use]
    fn param_types() -> [TypeId; NUM];
    /// Get the number of parameters for this function.
    #[must_use]
    fn num_params() -> usize;
    /// _(metadata)_ Get the type names of this function's parameters.
    /// Exported under the `metadata` feature only.
    #[cfg(feature = "metadata")]
    #[must_use]
    fn param_names() -> [&'static str; NUM];
    /// _(metadata)_ Get the type ID of this function's return value.
    /// Exported under the `metadata` feature only.
    #[cfg(feature = "metadata")]
    #[must_use]
    fn return_type() -> TypeId;
    /// _(metadata)_ Get the type name of this function's return value.
    /// Exported under the `metadata` feature only.
    #[cfg(feature = "metadata")]
    #[inline(always)]
    #[must_use]
    fn return_type_name() -> &'static str {
        type_name::<RET>()
    }
}

macro_rules! check_constant {
    ($abi:ident, $n:expr, $ctx:ident, $args:ident) => {
        #[cfg(any(not(feature = "no_object"), not(feature = "no_index")))]
        if stringify!($abi) == "Method" {
            let mut deny = false;

            #[cfg(not(feature = "no_index"))]
            if $n == 3 && !deny {
                deny = $ctx.fn_name() == FN_IDX_SET && $args[0].is_read_only();
            }
            #[cfg(not(feature = "no_object"))]
            if $n == 2 && !deny {
                deny = $ctx.fn_name().starts_with(FN_SET) && $args[0].is_read_only();
            }

            if deny {
                return Err(crate::ERR::ErrorNonPureMethodCallOnConstant(
                    $ctx.fn_name().to_string(),
                    crate::Position::NONE,
                )
                .into());
            }
        }
    };
}

macro_rules! def_register {
    () => {
        def_register!(imp Pure : 0;);
    };
    (imp $abi:ident : $n:expr ; $($par:ident => $arg:expr => $mark:ty => $param:ty => $clone:expr),*) => {
    //   ^ function ABI type
    //                ^ number of parameters
    //                            ^ function parameter generic type name (A, B, C etc.)
    //                                          ^ call argument(like A, *B, &mut C etc)
    //                                                       ^ function parameter marker type (A, Ref<B> or Mut<C>)
    //                                                                   ^ function parameter actual type (A, &B or &mut C)
    //                                                                                ^ parameter access function (by_value or by_ref)

        impl<
            FN: Fn($($param),*) -> RET + SendSync + 'static,
            $($par: Variant + Clone,)*
            RET: Variant + Clone
        > RegisterNativeFunction<($($mark,)*), $n, false, RET, false> for FN {
            #[inline(always)] fn param_types() -> [TypeId;$n] { [$(TypeId::of::<$par>()),*] }
            #[inline(always)] fn num_params() -> usize { $n }
            #[cfg(feature = "metadata")] #[inline(always)] fn param_names() -> [&'static str;$n] { [$(type_name::<$param>()),*] }
            #[cfg(feature = "metadata")] #[inline(always)] fn return_type() -> TypeId { TypeId::of::<RET>() }
            #[inline(always)] fn into_callable_function(self) -> CallableFunction {
                CallableFunction::$abi(Shared::new(move |ctx: NativeCallContext, args: &mut FnCallArgs| {
                    // The arguments are assumed to be of the correct number and types!
                    check_constant!($abi, $n, ctx, args);

                    let mut drain = args.iter_mut();
                    $(let mut $par = $clone(drain.next().unwrap()); )*

                    // Call the function with each argument value
                    let r = self($($arg),*);

                    // Map the result
                    Ok(Dynamic::from(r))
                }))
            }
        }

        impl<
            FN: for<'a> Fn(NativeCallContext<'a>, $($param),*) -> RET + SendSync + 'static,
            $($par: Variant + Clone,)*
            RET: Variant + Clone
        > RegisterNativeFunction<($($mark,)*), $n, true, RET, false> for FN {
            #[inline(always)] fn param_types() -> [TypeId;$n] { [$(TypeId::of::<$par>()),*] }
            #[inline(always)] fn num_params() -> usize { $n }
            #[cfg(feature = "metadata")] #[inline(always)] fn param_names() -> [&'static str;$n] { [$(type_name::<$param>()),*] }
            #[cfg(feature = "metadata")] #[inline(always)] fn return_type() -> TypeId { TypeId::of::<RET>() }
            #[inline(always)] fn into_callable_function(self) -> CallableFunction {
                CallableFunction::$abi(Shared::new(move |ctx: NativeCallContext, args: &mut FnCallArgs| {
                    // The arguments are assumed to be of the correct number and types!
                    check_constant!($abi, $n, ctx, args);

                    let mut drain = args.iter_mut();
                    $(let mut $par = $clone(drain.next().unwrap()); )*

                    // Call the function with each argument value
                    let r = self(ctx, $($arg),*);

                    // Map the result
                    Ok(Dynamic::from(r))
                }))
            }
        }

        impl<
            FN: Fn($($param),*) -> RhaiResultOf<RET> + SendSync + 'static,
            $($par: Variant + Clone,)*
            RET: Variant + Clone
        > RegisterNativeFunction<($($mark,)*), $n, false, RET, true> for FN {
            #[inline(always)] fn param_types() -> [TypeId;$n] { [$(TypeId::of::<$par>()),*] }
            #[inline(always)] fn num_params() -> usize { $n }
            #[cfg(feature = "metadata")] #[inline(always)] fn param_names() -> [&'static str;$n] { [$(type_name::<$param>()),*] }
            #[cfg(feature = "metadata")] #[inline(always)] fn return_type() -> TypeId { TypeId::of::<RhaiResultOf<RET>>() }
            #[cfg(feature = "metadata")] #[inline(always)] fn return_type_name() -> &'static str { type_name::<RhaiResultOf<RET>>() }
            #[inline(always)] fn into_callable_function(self) -> CallableFunction {
                CallableFunction::$abi(Shared::new(move |ctx: NativeCallContext, args: &mut FnCallArgs| {
                    // The arguments are assumed to be of the correct number and types!
                    check_constant!($abi, $n, ctx, args);

                    let mut drain = args.iter_mut();
                    $(let mut $par = $clone(drain.next().unwrap()); )*

                    // Call the function with each argument value
                    self($($arg),*).map(Dynamic::from)
                }))
            }
        }

        impl<
            FN: for<'a> Fn(NativeCallContext<'a>, $($param),*) -> RhaiResultOf<RET> + SendSync + 'static,
            $($par: Variant + Clone,)*
            RET: Variant + Clone
        > RegisterNativeFunction<($($mark,)*), $n, true, RET, true> for FN {
            #[inline(always)] fn param_types() -> [TypeId;$n] { [$(TypeId::of::<$par>()),*] }
            #[inline(always)] fn num_params() -> usize { $n }
            #[cfg(feature = "metadata")] #[inline(always)] fn param_names() -> [&'static str;$n] { [$(type_name::<$param>()),*] }
            #[cfg(feature = "metadata")] #[inline(always)] fn return_type() -> TypeId { TypeId::of::<RhaiResultOf<RET>>() }
            #[cfg(feature = "metadata")] #[inline(always)] fn return_type_name() -> &'static str { type_name::<RhaiResultOf<RET>>() }
            #[inline(always)] fn into_callable_function(self) -> CallableFunction {
                CallableFunction::$abi(Shared::new(move |ctx: NativeCallContext, args: &mut FnCallArgs| {
                    // The arguments are assumed to be of the correct number and types!
                    check_constant!($abi, $n, ctx, args);

                    let mut drain = args.iter_mut();
                    $(let mut $par = $clone(drain.next().unwrap()); )*

                    // Call the function with each argument value
                    self(ctx, $($arg),*).map(Dynamic::from)
                }))
            }
        }

        //def_register!(imp_pop $($par => $mark => $param),*);
    };
    ($p0:ident:$n0:expr $(, $p:ident: $n:expr)*) => {
        def_register!(imp Pure   : $n0 ; $p0 => $p0      => $p0      => $p0      => by_value $(, $p => $p => $p => $p => by_value)*);
        def_register!(imp Method : $n0 ; $p0 => &mut $p0 => Mut<$p0> => &mut $p0 => by_ref   $(, $p => $p => $p => $p => by_value)*);
        //                ^ CallableFunction constructor
        //                         ^ number of arguments                            ^ first parameter passed through
        //                                                                                       ^ others passed by value (by_value)

        // Currently does not support first argument which is a reference, as there will be
        // conflicting implementations since &T: Any and T: Any cannot be distinguished
        //def_register!(imp $p0 => Ref<$p0> => &$p0     => by_ref   $(, $p => $p => $p => by_value)*);

        def_register!($($p: $n),*);
    };
}

def_register!(A:20, B:19, C:18, D:17, E:16, F:15, G:14, H:13, J:12, K:11, L:10, M:9, N:8, P:7, Q:6, R:5, S:4, T:3, U:2, V:1);
