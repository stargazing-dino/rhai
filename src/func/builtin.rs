//! Built-in implementations for common operators.

use super::call::FnCallArgs;
use super::native::FnBuiltin;
use crate::tokenizer::{Token, Token::*};
use crate::{
    Dynamic, ExclusiveRange, ImmutableString, InclusiveRange, NativeCallContext, RhaiResult, INT,
};
use std::any::TypeId;
#[cfg(feature = "no_std")]
use std::prelude::v1::*;

#[cfg(not(feature = "no_float"))]
use crate::FLOAT;

#[cfg(not(feature = "no_float"))]
#[cfg(feature = "no_std")]
use num_traits::Float;

#[cfg(feature = "decimal")]
use rust_decimal::Decimal;

/// The message: data type was checked
const BUILTIN: &str = "data type was checked";

/// Is the type a numeric type?
#[inline]
#[must_use]
fn is_numeric(type_id: TypeId) -> bool {
    if type_id == TypeId::of::<INT>() {
        return true;
    }

    #[cfg(not(feature = "only_i64"))]
    #[cfg(not(feature = "only_i32"))]
    if type_id == TypeId::of::<u8>()
        || type_id == TypeId::of::<u16>()
        || type_id == TypeId::of::<u32>()
        || type_id == TypeId::of::<u64>()
        || type_id == TypeId::of::<i8>()
        || type_id == TypeId::of::<i16>()
        || type_id == TypeId::of::<i32>()
        || type_id == TypeId::of::<i64>()
    {
        return true;
    }

    #[cfg(not(feature = "only_i64"))]
    #[cfg(not(feature = "only_i32"))]
    #[cfg(not(target_family = "wasm"))]
    if type_id == TypeId::of::<u128>() || type_id == TypeId::of::<i128>() {
        return true;
    }

    #[cfg(not(feature = "no_float"))]
    if type_id == TypeId::of::<f32>() || type_id == TypeId::of::<f64>() {
        return true;
    }

    #[cfg(feature = "decimal")]
    if type_id == TypeId::of::<rust_decimal::Decimal>() {
        return true;
    }

    false
}

/// A function that returns `true`.
#[inline(always)]
#[must_use]
fn const_true_fn(_: NativeCallContext, _: &mut [&mut Dynamic]) -> RhaiResult {
    Ok(Dynamic::TRUE)
}
/// A function that returns `false`.
#[inline(always)]
#[must_use]
fn const_false_fn(_: NativeCallContext, _: &mut [&mut Dynamic]) -> RhaiResult {
    Ok(Dynamic::FALSE)
}

/// Build in common binary operator implementations to avoid the cost of calling a registered function.
///
/// The return function will be registered as a _method_, so the first parameter cannot be consumed.
#[must_use]
pub fn get_builtin_binary_op_fn(op: &Token, x: &Dynamic, y: &Dynamic) -> Option<FnBuiltin> {
    let type1 = x.type_id();
    let type2 = y.type_id();

    macro_rules! impl_op {
        ($xx:ident $op:tt $yy:ident) => { |_, args| {
            let x = &*args[0].read_lock::<$xx>().expect(BUILTIN);
            let y = &*args[1].read_lock::<$yy>().expect(BUILTIN);
            Ok((x $op y).into())
        } };
        ($xx:ident . $func:ident ( $yy:ty )) => { |_, args| {
            let x = &*args[0].read_lock::<$xx>().expect(BUILTIN);
            let y = &*args[1].read_lock::<$yy>().expect(BUILTIN);
            Ok(x.$func(y).into())
        } };
        ($xx:ident . $func:ident ( $yy:ident . $yyy:ident () )) => { |_, args| {
            let x = &*args[0].read_lock::<$xx>().expect(BUILTIN);
            let y = &*args[1].read_lock::<$yy>().expect(BUILTIN);
            Ok(x.$func(y.$yyy()).into())
        } };
        ($func:ident ( $op:tt )) => { |_, args| {
            let (x, y) = $func(args);
            Ok((x $op y).into())
        } };
        ($base:ty => $xx:ident $op:tt $yy:ident) => { |_, args| {
            let x = args[0].$xx().expect(BUILTIN) as $base;
            let y = args[1].$yy().expect(BUILTIN) as $base;
            Ok((x $op y).into())
        } };
        ($base:ty => $xx:ident . $func:ident ( $yy:ident as $yyy:ty)) => { |_, args| {
            let x = args[0].$xx().expect(BUILTIN) as $base;
            let y = args[1].$yy().expect(BUILTIN) as $base;
            Ok(x.$func(y as $yyy).into())
        } };
        ($base:ty => $func:ident ( $xx:ident, $yy:ident )) => { |_, args| {
            let x = args[0].$xx().expect(BUILTIN) as $base;
            let y = args[1].$yy().expect(BUILTIN) as $base;
            $func(x, y).map(Into::into)
        } };
        (from $base:ty => $xx:ident $op:tt $yy:ident) => { |_, args| {
            let x = <$base>::from(args[0].$xx().expect(BUILTIN));
            let y = <$base>::from(args[1].$yy().expect(BUILTIN));
            Ok((x $op y).into())
        } };
        (from $base:ty => $xx:ident . $func:ident ( $yy:ident )) => { |_, args| {
            let x = <$base>::from(args[0].$xx().expect(BUILTIN));
            let y = <$base>::from(args[1].$yy().expect(BUILTIN));
            Ok(x.$func(y).into())
        } };
        (from $base:ty => $func:ident ( $xx:ident, $yy:ident )) => { |_, args| {
            let x = <$base>::from(args[0].$xx().expect(BUILTIN));
            let y = <$base>::from(args[1].$yy().expect(BUILTIN));
            $func(x, y).map(Into::into)
        } };
    }

    // Check for common patterns
    if type1 == type2 {
        if type1 == TypeId::of::<INT>() {
            #[cfg(not(feature = "unchecked"))]
            use crate::packages::arithmetic::arith_basic::INT::functions::*;

            #[cfg(not(feature = "unchecked"))]
            match op {
                Plus => return Some(impl_op!(INT => add(as_int, as_int))),
                Minus => return Some(impl_op!(INT => subtract(as_int, as_int))),
                Multiply => return Some(impl_op!(INT => multiply(as_int, as_int))),
                Divide => return Some(impl_op!(INT => divide(as_int, as_int))),
                Modulo => return Some(impl_op!(INT => modulo(as_int, as_int))),
                PowerOf => return Some(impl_op!(INT => power(as_int, as_int))),
                RightShift => return Some(impl_op!(INT => shift_right(as_int, as_int))),
                LeftShift => return Some(impl_op!(INT => shift_left(as_int, as_int))),
                _ => (),
            }

            #[cfg(feature = "unchecked")]
            match op {
                Plus => return Some(impl_op!(INT => as_int + as_int)),
                Minus => return Some(impl_op!(INT => as_int - as_int)),
                Multiply => return Some(impl_op!(INT => as_int * as_int)),
                Divide => return Some(impl_op!(INT => as_int / as_int)),
                Modulo => return Some(impl_op!(INT => as_int % as_int)),
                PowerOf => return Some(impl_op!(INT => as_int.pow(as_int as u32))),
                RightShift => return Some(impl_op!(INT => as_int >> as_int)),
                LeftShift => return Some(impl_op!(INT => as_int << as_int)),
                _ => (),
            }

            return match op {
                EqualsTo => Some(impl_op!(INT => as_int == as_int)),
                NotEqualsTo => Some(impl_op!(INT => as_int != as_int)),
                GreaterThan => Some(impl_op!(INT => as_int > as_int)),
                GreaterThanEqualsTo => Some(impl_op!(INT => as_int >= as_int)),
                LessThan => Some(impl_op!(INT => as_int < as_int)),
                LessThanEqualsTo => Some(impl_op!(INT => as_int <= as_int)),
                Ampersand => Some(impl_op!(INT => as_int & as_int)),
                Pipe => Some(impl_op!(INT => as_int | as_int)),
                XOr => Some(impl_op!(INT => as_int ^ as_int)),
                ExclusiveRange => Some(|_, args| {
                    let x = args[0].as_int().expect(BUILTIN);
                    let y = args[1].as_int().expect(BUILTIN);
                    Ok((x..y).into())
                }),
                InclusiveRange => Some(|_, args| {
                    let x = args[0].as_int().expect(BUILTIN);
                    let y = args[1].as_int().expect(BUILTIN);
                    Ok((x..=y).into())
                }),
                _ => None,
            };
        }

        if type1 == TypeId::of::<bool>() {
            return match op {
                EqualsTo => Some(impl_op!(bool => as_bool == as_bool)),
                NotEqualsTo => Some(impl_op!(bool => as_bool != as_bool)),
                GreaterThan => Some(impl_op!(bool => as_bool > as_bool)),
                GreaterThanEqualsTo => Some(impl_op!(bool => as_bool >= as_bool)),
                LessThan => Some(impl_op!(bool => as_bool < as_bool)),
                LessThanEqualsTo => Some(impl_op!(bool => as_bool <= as_bool)),
                Ampersand => Some(impl_op!(bool => as_bool & as_bool)),
                Pipe => Some(impl_op!(bool => as_bool | as_bool)),
                XOr => Some(impl_op!(bool => as_bool ^ as_bool)),
                _ => None,
            };
        }

        if type1 == TypeId::of::<ImmutableString>() {
            return match op {
                Plus => Some(|_ctx, args| {
                    let s1 = &*args[0].read_lock::<ImmutableString>().expect(BUILTIN);
                    let s2 = &*args[1].read_lock::<ImmutableString>().expect(BUILTIN);

                    #[cfg(not(feature = "unchecked"))]
                    if !s1.is_empty() && !s2.is_empty() {
                        let total_len = s1.len() + s2.len();
                        _ctx.engine()
                            .raise_err_if_over_data_size_limit((0, 0, total_len))?;
                    }

                    Ok((s1 + s2).into())
                }),
                Minus => Some(impl_op!(ImmutableString - ImmutableString)),
                EqualsTo => Some(impl_op!(ImmutableString == ImmutableString)),
                NotEqualsTo => Some(impl_op!(ImmutableString != ImmutableString)),
                GreaterThan => Some(impl_op!(ImmutableString > ImmutableString)),
                GreaterThanEqualsTo => Some(impl_op!(ImmutableString >= ImmutableString)),
                LessThan => Some(impl_op!(ImmutableString < ImmutableString)),
                LessThanEqualsTo => Some(impl_op!(ImmutableString <= ImmutableString)),
                _ => None,
            };
        }

        if type1 == TypeId::of::<char>() {
            return match op {
                Plus => Some(|_ctx, args| {
                    let x = args[0].as_char().expect(BUILTIN);
                    let y = args[1].as_char().expect(BUILTIN);

                    let result = format!("{x}{y}");

                    #[cfg(not(feature = "unchecked"))]
                    _ctx.engine()
                        .raise_err_if_over_data_size_limit((0, 0, result.len()))?;

                    Ok(result.into())
                }),
                EqualsTo => Some(impl_op!(char => as_char == as_char)),
                NotEqualsTo => Some(impl_op!(char => as_char != as_char)),
                GreaterThan => Some(impl_op!(char => as_char > as_char)),
                GreaterThanEqualsTo => Some(impl_op!(char => as_char >= as_char)),
                LessThan => Some(impl_op!(char => as_char < as_char)),
                LessThanEqualsTo => Some(impl_op!(char => as_char <= as_char)),
                _ => None,
            };
        }

        #[cfg(not(feature = "no_index"))]
        if type1 == TypeId::of::<crate::Blob>() {
            use crate::Blob;

            return match op {
                Plus => Some(|_ctx, args| {
                    let blob1 = &*args[0].read_lock::<Blob>().expect(BUILTIN);
                    let blob2 = &*args[1].read_lock::<Blob>().expect(BUILTIN);

                    Ok(Dynamic::from_blob(if blob2.is_empty() {
                        blob1.clone()
                    } else if blob1.is_empty() {
                        blob2.clone()
                    } else {
                        #[cfg(not(feature = "unchecked"))]
                        _ctx.engine().raise_err_if_over_data_size_limit((
                            blob1.len() + blob2.len(),
                            0,
                            0,
                        ))?;

                        let mut blob = blob1.clone();
                        blob.extend(blob2);
                        blob
                    }))
                }),
                EqualsTo => Some(impl_op!(Blob == Blob)),
                NotEqualsTo => Some(impl_op!(Blob != Blob)),
                _ => None,
            };
        }

        if type1 == TypeId::of::<()>() {
            return match op {
                EqualsTo => Some(const_true_fn),
                NotEqualsTo | GreaterThan | GreaterThanEqualsTo | LessThan | LessThanEqualsTo => {
                    Some(const_false_fn)
                }
                _ => None,
            };
        }
    }

    #[cfg(not(feature = "no_float"))]
    macro_rules! impl_float {
        ($x:ty, $xx:ident, $y:ty, $yy:ident) => {
            if (type1, type2) == (TypeId::of::<$x>(), TypeId::of::<$y>()) {
                return match op {
                    Plus                => Some(impl_op!(FLOAT => $xx + $yy)),
                    Minus               => Some(impl_op!(FLOAT => $xx - $yy)),
                    Multiply            => Some(impl_op!(FLOAT => $xx * $yy)),
                    Divide              => Some(impl_op!(FLOAT => $xx / $yy)),
                    Modulo              => Some(impl_op!(FLOAT => $xx % $yy)),
                    PowerOf             => Some(impl_op!(FLOAT => $xx.powf($yy as FLOAT))),
                    EqualsTo            => Some(impl_op!(FLOAT => $xx == $yy)),
                    NotEqualsTo         => Some(impl_op!(FLOAT => $xx != $yy)),
                    GreaterThan         => Some(impl_op!(FLOAT => $xx > $yy)),
                    GreaterThanEqualsTo => Some(impl_op!(FLOAT => $xx >= $yy)),
                    LessThan            => Some(impl_op!(FLOAT => $xx < $yy)),
                    LessThanEqualsTo    => Some(impl_op!(FLOAT => $xx <= $yy)),
                    _                   => None,
                };
            }
        };
    }

    #[cfg(not(feature = "no_float"))]
    {
        impl_float!(FLOAT, as_float, FLOAT, as_float);
        impl_float!(FLOAT, as_float, INT, as_int);
        impl_float!(INT, as_int, FLOAT, as_float);
    }

    #[cfg(feature = "decimal")]
    macro_rules! impl_decimal {
        ($x:ty, $xx:ident, $y:ty, $yy:ident) => {
            if (type1, type2) == (TypeId::of::<$x>(), TypeId::of::<$y>()) {
                #[cfg(not(feature = "unchecked"))]
                use crate::packages::arithmetic::decimal_functions::builtin::*;

                #[cfg(not(feature = "unchecked"))]
                match op {
                    Plus     => return Some(impl_op!(from Decimal => add($xx, $yy))),
                    Minus    => return Some(impl_op!(from Decimal => subtract($xx, $yy))),
                    Multiply => return Some(impl_op!(from Decimal => multiply($xx, $yy))),
                    Divide   => return Some(impl_op!(from Decimal => divide($xx, $yy))),
                    Modulo   => return Some(impl_op!(from Decimal => modulo($xx, $yy))),
                    PowerOf  => return Some(impl_op!(from Decimal => power($xx, $yy))),
                    _        => ()
                }

                #[cfg(feature = "unchecked")]
                use rust_decimal::MathematicalOps;

                #[cfg(feature = "unchecked")]
                match op {
                    Plus     => return Some(impl_op!(from Decimal => $xx + $yy)),
                    Minus    => return Some(impl_op!(from Decimal => $xx - $yy)),
                    Multiply => return Some(impl_op!(from Decimal => $xx * $yy)),
                    Divide   => return Some(impl_op!(from Decimal => $xx / $yy)),
                    Modulo   => return Some(impl_op!(from Decimal => $xx % $yy)),
                    PowerOf  => return Some(impl_op!(from Decimal => $xx.powd($yy))),
                    _        => ()
                }

                return match op {
                    EqualsTo            => Some(impl_op!(from Decimal => $xx == $yy)),
                    NotEqualsTo         => Some(impl_op!(from Decimal => $xx != $yy)),
                    GreaterThan         => Some(impl_op!(from Decimal => $xx > $yy)),
                    GreaterThanEqualsTo => Some(impl_op!(from Decimal => $xx >= $yy)),
                    LessThan            => Some(impl_op!(from Decimal => $xx < $yy)),
                    LessThanEqualsTo    => Some(impl_op!(from Decimal => $xx <= $yy)),
                    _                   => None
                };
            }
        };
    }

    #[cfg(feature = "decimal")]
    {
        impl_decimal!(Decimal, as_decimal, Decimal, as_decimal);
        impl_decimal!(Decimal, as_decimal, INT, as_int);
        impl_decimal!(INT, as_int, Decimal, as_decimal);
    }

    // char op string
    if (type1, type2) == (TypeId::of::<char>(), TypeId::of::<ImmutableString>()) {
        fn get_s1s2(args: &FnCallArgs) -> ([char; 2], [char; 2]) {
            let x = args[0].as_char().expect(BUILTIN);
            let y = &*args[1].read_lock::<ImmutableString>().expect(BUILTIN);
            let s1 = [x, '\0'];
            let mut y = y.chars();
            let s2 = [y.next().unwrap_or('\0'), y.next().unwrap_or('\0')];
            (s1, s2)
        }

        return match op {
            Plus => Some(|_ctx, args| {
                let x = args[0].as_char().expect(BUILTIN);
                let y = &*args[1].read_lock::<ImmutableString>().expect(BUILTIN);
                let result = format!("{x}{y}");

                #[cfg(not(feature = "unchecked"))]
                _ctx.engine()
                    .raise_err_if_over_data_size_limit((0, 0, result.len()))?;

                Ok(result.into())
            }),
            EqualsTo => Some(impl_op!(get_s1s2(==))),
            NotEqualsTo => Some(impl_op!(get_s1s2(!=))),
            GreaterThan => Some(impl_op!(get_s1s2(>))),
            GreaterThanEqualsTo => Some(impl_op!(get_s1s2(>=))),
            LessThan => Some(impl_op!(get_s1s2(<))),
            LessThanEqualsTo => Some(impl_op!(get_s1s2(<=))),
            _ => None,
        };
    }
    // string op char
    if (type1, type2) == (TypeId::of::<ImmutableString>(), TypeId::of::<char>()) {
        fn get_s1s2(args: &FnCallArgs) -> ([char; 2], [char; 2]) {
            let x = &*args[0].read_lock::<ImmutableString>().expect(BUILTIN);
            let y = args[1].as_char().expect(BUILTIN);
            let mut x = x.chars();
            let s1 = [x.next().unwrap_or('\0'), x.next().unwrap_or('\0')];
            let s2 = [y, '\0'];
            (s1, s2)
        }

        return match op {
            Plus => Some(|_ctx, args| {
                let x = &*args[0].read_lock::<ImmutableString>().expect(BUILTIN);
                let y = args[1].as_char().expect(BUILTIN);
                let result = x + y;

                #[cfg(not(feature = "unchecked"))]
                _ctx.engine()
                    .raise_err_if_over_data_size_limit((0, 0, result.len()))?;

                Ok(result.into())
            }),
            Minus => Some(|_, args| {
                let x = &*args[0].read_lock::<ImmutableString>().expect(BUILTIN);
                let y = args[1].as_char().expect(BUILTIN);
                Ok((x - y).into())
            }),
            EqualsTo => Some(impl_op!(get_s1s2(==))),
            NotEqualsTo => Some(impl_op!(get_s1s2(!=))),
            GreaterThan => Some(impl_op!(get_s1s2(>))),
            GreaterThanEqualsTo => Some(impl_op!(get_s1s2(>=))),
            LessThan => Some(impl_op!(get_s1s2(<))),
            LessThanEqualsTo => Some(impl_op!(get_s1s2(<=))),
            _ => None,
        };
    }
    // () op string
    if (type1, type2) == (TypeId::of::<()>(), TypeId::of::<ImmutableString>()) {
        return match op {
            Plus => Some(|_, args| Ok(args[1].clone())),
            EqualsTo | GreaterThan | GreaterThanEqualsTo | LessThan | LessThanEqualsTo => {
                Some(const_false_fn)
            }
            NotEqualsTo => Some(const_true_fn),
            _ => None,
        };
    }
    // string op ()
    if (type1, type2) == (TypeId::of::<ImmutableString>(), TypeId::of::<()>()) {
        return match op {
            Plus => Some(|_, args| Ok(args[0].clone())),
            EqualsTo | GreaterThan | GreaterThanEqualsTo | LessThan | LessThanEqualsTo => {
                Some(const_false_fn)
            }
            NotEqualsTo => Some(const_true_fn),
            _ => None,
        };
    }

    // blob
    #[cfg(not(feature = "no_index"))]
    if type1 == TypeId::of::<crate::Blob>() {
        use crate::Blob;

        if type2 == TypeId::of::<char>() {
            return match op {
                Plus => Some(|_ctx, args| {
                    let mut blob = args[0].read_lock::<Blob>().expect(BUILTIN).clone();
                    let mut buf = [0_u8; 4];
                    let x = args[1].as_char().expect(BUILTIN).encode_utf8(&mut buf);

                    #[cfg(not(feature = "unchecked"))]
                    _ctx.engine().raise_err_if_over_data_size_limit((
                        blob.len() + x.len(),
                        0,
                        0,
                    ))?;

                    blob.extend(x.as_bytes());
                    Ok(Dynamic::from_blob(blob))
                }),
                _ => None,
            };
        }
    }

    // Non-compatible ranges
    if (type1, type2)
        == (
            TypeId::of::<ExclusiveRange>(),
            TypeId::of::<InclusiveRange>(),
        )
        || (type1, type2)
            == (
                TypeId::of::<InclusiveRange>(),
                TypeId::of::<ExclusiveRange>(),
            )
    {
        return match op {
            NotEqualsTo => Some(const_true_fn),
            Equals => Some(const_false_fn),
            _ => None,
        };
    }

    // Handle ranges here because ranges are implemented as custom type
    if type1 == TypeId::of::<ExclusiveRange>() {
        if type1 == type2 {
            return match op {
                EqualsTo => Some(impl_op!(ExclusiveRange == ExclusiveRange)),
                NotEqualsTo => Some(impl_op!(ExclusiveRange != ExclusiveRange)),
                _ => None,
            };
        }
    }

    if type1 == TypeId::of::<InclusiveRange>() {
        if type1 == type2 {
            return match op {
                EqualsTo => Some(impl_op!(InclusiveRange == InclusiveRange)),
                NotEqualsTo => Some(impl_op!(InclusiveRange != InclusiveRange)),
                _ => None,
            };
        }
    }

    // One of the operands is a custom type, so it is never built-in
    if x.is_variant() || y.is_variant() {
        return if is_numeric(type1) && is_numeric(type2) {
            // Disallow comparisons between different numeric types
            None
        } else if type1 != type2 {
            // If the types are not the same, default to not compare
            match op {
                NotEqualsTo => Some(const_true_fn),
                EqualsTo | GreaterThan | GreaterThanEqualsTo | LessThan | LessThanEqualsTo => {
                    Some(const_false_fn)
                }
                _ => None,
            }
        } else {
            // Disallow comparisons between the same type
            None
        };
    }

    // Default comparison operators for different types
    if type2 != type1 {
        return match op {
            NotEqualsTo => Some(const_true_fn),
            EqualsTo | GreaterThan | GreaterThanEqualsTo | LessThan | LessThanEqualsTo => {
                Some(const_false_fn)
            }
            _ => None,
        };
    }

    // Beyond here, type1 == type2
    None
}

/// Build in common operator assignment implementations to avoid the cost of calling a registered function.
///
/// The return function is registered as a _method_, so the first parameter cannot be consumed.
#[must_use]
pub fn get_builtin_op_assignment_fn(op: &Token, x: &Dynamic, y: &Dynamic) -> Option<FnBuiltin> {
    let type1 = x.type_id();
    let type2 = y.type_id();

    macro_rules! impl_op {
        ($x:ty = x $op:tt $yy:ident) => { |_, args| {
            let x = args[0].$yy().expect(BUILTIN);
            let y = args[1].$yy().expect(BUILTIN) as $x;
            Ok((*args[0].write_lock::<$x>().expect(BUILTIN) = x $op y).into())
        } };
        ($x:ident $op:tt $yy:ident) => { |_, args| {
            let y = args[1].$yy().expect(BUILTIN) as $x;
            Ok((*args[0].write_lock::<$x>().expect(BUILTIN) $op y).into())
        } };
        ($x:ident $op:tt $yy:ident as $yyy:ty) => { |_, args| {
            let y = args[1].$yy().expect(BUILTIN) as $yyy;
            Ok((*args[0].write_lock::<$x>().expect(BUILTIN) $op y).into())
        } };
        ($x:ty => $xx:ident . $func:ident ( $yy:ident as $yyy:ty )) => { |_, args| {
            let x = args[0].$xx().expect(BUILTIN);
            let y = args[1].$yy().expect(BUILTIN) as $x;
            Ok((*args[0].write_lock::<$x>().expect(BUILTIN) = x.$func(y as $yyy)).into())
        } };
        ($x:ty => $func:ident ( $xx:ident, $yy:ident )) => { |_, args| {
            let x = args[0].$xx().expect(BUILTIN);
            let y = args[1].$yy().expect(BUILTIN) as $x;
            Ok((*args[0].write_lock().expect(BUILTIN) = $func(x, y)?).into())
        } };
        (from $x:ident $op:tt $yy:ident) => { |_, args| {
            let y = <$x>::from(args[1].$yy().expect(BUILTIN));
            Ok((*args[0].write_lock::<$x>().expect(BUILTIN) $op y).into())
        } };
        (from $x:ty => $xx:ident . $func:ident ( $yy:ident )) => { |_, args| {
            let x = args[0].$xx().expect(BUILTIN);
            let y = <$x>::from(args[1].$yy().expect(BUILTIN));
            Ok((*args[0].write_lock::<$x>().expect(BUILTIN) = x.$func(y)).into())
        } };
        (from $x:ty => $func:ident ( $xx:ident, $yy:ident )) => { |_, args| {
            let x = args[0].$xx().expect(BUILTIN);
            let y = <$x>::from(args[1].$yy().expect(BUILTIN));
            Ok((*args[0].write_lock().expect(BUILTIN) = $func(x, y)?).into())
        } };
    }

    // Check for common patterns
    if type1 == type2 {
        if type1 == TypeId::of::<INT>() {
            #[cfg(not(feature = "unchecked"))]
            use crate::packages::arithmetic::arith_basic::INT::functions::*;

            #[cfg(not(feature = "unchecked"))]
            match op {
                PlusAssign => return Some(impl_op!(INT => add(as_int, as_int))),
                MinusAssign => return Some(impl_op!(INT => subtract(as_int, as_int))),
                MultiplyAssign => return Some(impl_op!(INT => multiply(as_int, as_int))),
                DivideAssign => return Some(impl_op!(INT => divide(as_int, as_int))),
                ModuloAssign => return Some(impl_op!(INT => modulo(as_int, as_int))),
                PowerOfAssign => return Some(impl_op!(INT => power(as_int, as_int))),
                RightShiftAssign => return Some(impl_op!(INT => shift_right(as_int, as_int))),
                LeftShiftAssign => return Some(impl_op!(INT => shift_left(as_int, as_int))),
                _ => (),
            }

            #[cfg(feature = "unchecked")]
            match op {
                PlusAssign => return Some(impl_op!(INT += as_int)),
                MinusAssign => return Some(impl_op!(INT -= as_int)),
                MultiplyAssign => return Some(impl_op!(INT *= as_int)),
                DivideAssign => return Some(impl_op!(INT /= as_int)),
                ModuloAssign => return Some(impl_op!(INT %= as_int)),
                PowerOfAssign => return Some(impl_op!(INT => as_int.pow(as_int as u32))),
                RightShiftAssign => return Some(impl_op!(INT >>= as_int)),
                LeftShiftAssign => return Some(impl_op!(INT <<= as_int)),
                _ => (),
            }

            return match op {
                AndAssign => Some(impl_op!(INT &= as_int)),
                OrAssign => Some(impl_op!(INT |= as_int)),
                XOrAssign => Some(impl_op!(INT ^= as_int)),
                _ => None,
            };
        }

        if type1 == TypeId::of::<bool>() {
            return match op {
                AndAssign => Some(impl_op!(bool = x && as_bool)),
                OrAssign => Some(impl_op!(bool = x || as_bool)),
                _ => None,
            };
        }

        if type1 == TypeId::of::<char>() {
            return match op {
                PlusAssign => Some(|_, args| {
                    let y = args[1].as_char().expect(BUILTIN);
                    let x = &mut *args[0].write_lock::<Dynamic>().expect(BUILTIN);
                    Ok((*x = format!("{x}{y}").into()).into())
                }),
                _ => None,
            };
        }

        if type1 == TypeId::of::<ImmutableString>() {
            return match op {
                PlusAssign => Some(|_ctx, args| {
                    let (first, second) = args.split_first_mut().expect(BUILTIN);
                    let x = &mut *first.write_lock::<ImmutableString>().expect(BUILTIN);
                    let y = std::mem::take(second[0]).cast::<ImmutableString>();

                    #[cfg(not(feature = "unchecked"))]
                    if !x.is_empty() && !y.is_empty() {
                        let total_len = x.len() + y.len();
                        _ctx.engine()
                            .raise_err_if_over_data_size_limit((0, 0, total_len))?;
                    }

                    Ok((*x += y).into())
                }),
                MinusAssign => Some(|_, args| {
                    let (first, second) = args.split_first_mut().expect(BUILTIN);
                    let x = &mut *first.write_lock::<ImmutableString>().expect(BUILTIN);
                    let y = std::mem::take(second[0]).cast::<ImmutableString>();
                    Ok((*x -= y).into())
                }),
                _ => None,
            };
        }

        #[cfg(not(feature = "no_index"))]
        if type1 == TypeId::of::<crate::Array>() {
            use crate::packages::array_basic::array_functions::*;
            use crate::Array;

            return match op {
                PlusAssign => Some(|_ctx, args| {
                    let x = std::mem::take(args[1]).cast::<Array>();

                    if x.is_empty() {
                        return Ok(Dynamic::UNIT);
                    }

                    let _array_is_empty = args[0].read_lock::<Array>().expect(BUILTIN).is_empty();

                    #[cfg(not(feature = "unchecked"))]
                    if !_array_is_empty {
                        _ctx.engine().check_data_size(
                            &*args[0].read_lock().expect(BUILTIN),
                            crate::Position::NONE,
                        )?;
                    }

                    let array = &mut *args[0].write_lock::<Array>().expect(BUILTIN);

                    Ok(append(array, x).into())
                }),
                _ => None,
            };
        }

        #[cfg(not(feature = "no_index"))]
        if type1 == TypeId::of::<crate::Blob>() {
            use crate::packages::blob_basic::blob_functions::*;
            use crate::Blob;

            return match op {
                PlusAssign => Some(|_ctx, args| {
                    let blob2 = std::mem::take(args[1]).cast::<Blob>();
                    let blob1 = &mut *args[0].write_lock::<Blob>().expect(BUILTIN);

                    #[cfg(not(feature = "unchecked"))]
                    _ctx.engine().raise_err_if_over_data_size_limit((
                        blob1.len() + blob2.len(),
                        0,
                        0,
                    ))?;

                    Ok(append(blob1, blob2).into())
                }),
                _ => None,
            };
        }
    }

    #[cfg(not(feature = "no_float"))]
    macro_rules! impl_float {
        ($x:ident, $xx:ident, $y:ty, $yy:ident) => {
            if (type1, type2) == (TypeId::of::<$x>(), TypeId::of::<$y>()) {
                return match op {
                    PlusAssign      => Some(impl_op!($x += $yy)),
                    MinusAssign     => Some(impl_op!($x -= $yy)),
                    MultiplyAssign  => Some(impl_op!($x *= $yy)),
                    DivideAssign    => Some(impl_op!($x /= $yy)),
                    ModuloAssign    => Some(impl_op!($x %= $yy)),
                    PowerOfAssign   => Some(impl_op!($x => $xx.powf($yy as $x))),
                    _               => None,
                };
            }
        }
    }

    #[cfg(not(feature = "no_float"))]
    {
        impl_float!(FLOAT, as_float, FLOAT, as_float);
        impl_float!(FLOAT, as_float, INT, as_int);
    }

    #[cfg(feature = "decimal")]
    macro_rules! impl_decimal {
        ($x:ident, $xx:ident, $y:ty, $yy:ident) => {
            if (type1, type2) == (TypeId::of::<$x>(), TypeId::of::<$y>()) {
                #[cfg(not(feature = "unchecked"))]
                use crate::packages::arithmetic::decimal_functions::builtin::*;

                #[cfg(not(feature = "unchecked"))]
                return match op {
                    PlusAssign      => Some(impl_op!(from $x => add($xx, $yy))),
                    MinusAssign     => Some(impl_op!(from $x => subtract($xx, $yy))),
                    MultiplyAssign  => Some(impl_op!(from $x => multiply($xx, $yy))),
                    DivideAssign    => Some(impl_op!(from $x => divide($xx, $yy))),
                    ModuloAssign    => Some(impl_op!(from $x => modulo($xx, $yy))),
                    PowerOfAssign   => Some(impl_op!(from $x => power($xx, $yy))),
                    _               => None,
                };

                #[cfg(feature = "unchecked")]
                use rust_decimal::MathematicalOps;

                #[cfg(feature = "unchecked")]
                return match op {
                    PlusAssign      => Some(impl_op!(from $x += $yy)),
                    MinusAssign     => Some(impl_op!(from $x -= $yy)),
                    MultiplyAssign  => Some(impl_op!(from $x *= $yy)),
                    DivideAssign    => Some(impl_op!(from $x /= $yy)),
                    ModuloAssign    => Some(impl_op!(from $x %= $yy)),
                    PowerOfAssign   => Some(impl_op!(from $x => $xx.powd($yy))),
                    _               => None,
                };
            }
        };
    }

    #[cfg(feature = "decimal")]
    {
        impl_decimal!(Decimal, as_decimal, Decimal, as_decimal);
        impl_decimal!(Decimal, as_decimal, INT, as_int);
    }

    // string op= char
    if (type1, type2) == (TypeId::of::<ImmutableString>(), TypeId::of::<char>()) {
        return match op {
            PlusAssign => Some(|_ctx, args| {
                let mut buf = [0_u8; 4];
                let ch = &*args[1].as_char().expect(BUILTIN).encode_utf8(&mut buf);
                let mut x = args[0].write_lock::<ImmutableString>().expect(BUILTIN);

                #[cfg(not(feature = "unchecked"))]
                _ctx.engine()
                    .raise_err_if_over_data_size_limit((0, 0, x.len() + ch.len()))?;

                Ok((*x += ch).into())
            }),
            MinusAssign => Some(impl_op!(ImmutableString -= as_char as char)),
            _ => None,
        };
    }
    // char op= string
    if (type1, type2) == (TypeId::of::<char>(), TypeId::of::<ImmutableString>()) {
        return match op {
            PlusAssign => Some(|_ctx, args| {
                let ch = {
                    let s = &*args[1].read_lock::<ImmutableString>().expect(BUILTIN);

                    if s.is_empty() {
                        return Ok(Dynamic::UNIT);
                    }

                    let mut ch = args[0].as_char().expect(BUILTIN).to_string();

                    #[cfg(not(feature = "unchecked"))]
                    _ctx.engine()
                        .raise_err_if_over_data_size_limit((0, 0, ch.len() + s.len()))?;

                    ch.push_str(s);
                    ch
                };

                *args[0].write_lock::<Dynamic>().expect(BUILTIN) = ch.into();

                Ok(Dynamic::UNIT)
            }),
            _ => None,
        };
    }

    // array op= any
    #[cfg(not(feature = "no_index"))]
    if type1 == TypeId::of::<crate::Array>() {
        use crate::packages::array_basic::array_functions::*;
        use crate::Array;

        return match op {
            PlusAssign => Some(|_ctx, args| {
                {
                    let x = std::mem::take(args[1]);
                    let array = &mut *args[0].write_lock::<Array>().expect(BUILTIN);
                    push(array, x);
                }

                #[cfg(not(feature = "unchecked"))]
                _ctx.engine().check_data_size(
                    &*args[0].read_lock().expect(BUILTIN),
                    crate::Position::NONE,
                )?;

                Ok(Dynamic::UNIT)
            }),
            _ => None,
        };
    }

    #[cfg(not(feature = "no_index"))]
    {
        use crate::Blob;

        // blob op= int
        if (type1, type2) == (TypeId::of::<Blob>(), TypeId::of::<INT>()) {
            use crate::packages::blob_basic::blob_functions::*;

            return match op {
                PlusAssign => Some(|_ctx, args| {
                    let x = args[1].as_int().expect(BUILTIN);
                    let blob = &mut *args[0].write_lock::<Blob>().expect(BUILTIN);

                    #[cfg(not(feature = "unchecked"))]
                    _ctx.engine()
                        .raise_err_if_over_data_size_limit((blob.len() + 1, 0, 0))?;

                    Ok(push(blob, x).into())
                }),
                _ => None,
            };
        }

        // blob op= char
        if (type1, type2) == (TypeId::of::<Blob>(), TypeId::of::<char>()) {
            use crate::packages::blob_basic::blob_functions::*;

            return match op {
                PlusAssign => Some(|_ctx, args| {
                    let x = args[1].as_char().expect(BUILTIN);
                    let blob = &mut *args[0].write_lock::<Blob>().expect(BUILTIN);

                    #[cfg(not(feature = "unchecked"))]
                    _ctx.engine()
                        .raise_err_if_over_data_size_limit((blob.len() + 1, 0, 0))?;

                    Ok(append_char(blob, x).into())
                }),
                _ => None,
            };
        }

        // blob op= string
        if (type1, type2) == (TypeId::of::<Blob>(), TypeId::of::<ImmutableString>()) {
            use crate::packages::blob_basic::blob_functions::*;

            return match op {
                PlusAssign => Some(|_ctx, args| {
                    let s = std::mem::take(args[1]).cast::<ImmutableString>();

                    if s.is_empty() {
                        return Ok(Dynamic::UNIT);
                    }

                    let blob = &mut *args[0].write_lock::<Blob>().expect(BUILTIN);

                    #[cfg(not(feature = "unchecked"))]
                    _ctx.engine().raise_err_if_over_data_size_limit((
                        blob.len() + s.len(),
                        0,
                        0,
                    ))?;

                    Ok(append_str(blob, &s).into())
                }),
                _ => None,
            };
        }
    }

    None
}
