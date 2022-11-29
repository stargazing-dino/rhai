//! Data size checks during evaluation.
#![cfg(not(feature = "unchecked"))]

use super::GlobalRuntimeState;
use crate::types::dynamic::Union;
use crate::{Dynamic, Engine, Position, RhaiResultOf, ERR};
use std::borrow::Borrow;
#[cfg(feature = "no_std")]
use std::prelude::v1::*;

impl Dynamic {
    /// Recursively calculate the sizes of a value.
    ///
    /// Sizes returned are `(` [`Array`][crate::Array], [`Map`][crate::Map] and [`String`] `)`.
    ///
    /// # Panics
    ///
    /// Panics if any interior data is shared (should never happen).
    pub(crate) fn calc_data_sizes(&self, _top: bool) -> (usize, usize, usize) {
        match self.0 {
            #[cfg(not(feature = "no_index"))]
            Union::Array(ref arr, ..) => {
                arr.iter()
                    .fold((0, 0, 0), |(ax, mx, sx), value| match value.0 {
                        Union::Array(..) => {
                            let (a, m, s) = value.calc_data_sizes(false);
                            (ax + a + 1, mx + m, sx + s)
                        }
                        Union::Blob(ref a, ..) => (ax + 1 + a.len(), mx, sx),
                        #[cfg(not(feature = "no_object"))]
                        Union::Map(..) => {
                            let (a, m, s) = value.calc_data_sizes(false);
                            (ax + a + 1, mx + m, sx + s)
                        }
                        Union::Str(ref s, ..) => (ax + 1, mx, sx + s.len()),
                        _ => (ax + 1, mx, sx),
                    })
            }
            #[cfg(not(feature = "no_index"))]
            Union::Blob(ref blob, ..) => (blob.len(), 0, 0),
            #[cfg(not(feature = "no_object"))]
            Union::Map(ref map, ..) => {
                map.values()
                    .fold((0, 0, 0), |(ax, mx, sx), value| match value.0 {
                        #[cfg(not(feature = "no_index"))]
                        Union::Array(..) => {
                            let (a, m, s) = value.calc_data_sizes(false);
                            (ax + a, mx + m + 1, sx + s)
                        }
                        #[cfg(not(feature = "no_index"))]
                        Union::Blob(ref a, ..) => (ax + a.len(), mx, sx),
                        Union::Map(..) => {
                            let (a, m, s) = value.calc_data_sizes(false);
                            (ax + a, mx + m + 1, sx + s)
                        }
                        Union::Str(ref s, ..) => (ax, mx + 1, sx + s.len()),
                        _ => (ax, mx + 1, sx),
                    })
            }
            Union::Str(ref s, ..) => (0, 0, s.len()),
            #[cfg(not(feature = "no_closure"))]
            Union::Shared(..) if _top => self.read_lock::<Self>().unwrap().calc_data_sizes(true),
            #[cfg(not(feature = "no_closure"))]
            Union::Shared(..) => {
                unreachable!("shared values discovered within data: {}", self)
            }
            _ => (0, 0, 0),
        }
    }
}

impl Engine {
    /// Raise an error if any data size exceeds limit.
    ///
    /// [`Position`] in [`EvalAltResult`][crate::EvalAltResult] is always [`NONE`][Position::NONE]
    /// and should be set afterwards.
    pub(crate) fn raise_err_if_over_data_size_limit(
        &self,
        (_arr, _map, s): (usize, usize, usize),
    ) -> RhaiResultOf<()> {
        if self
            .limits
            .max_string_len
            .map_or(false, |max| s > max.get())
        {
            return Err(
                ERR::ErrorDataTooLarge("Length of string".to_string(), Position::NONE).into(),
            );
        }

        #[cfg(not(feature = "no_index"))]
        if self
            .limits
            .max_array_size
            .map_or(false, |max| _arr > max.get())
        {
            return Err(
                ERR::ErrorDataTooLarge("Size of array/BLOB".to_string(), Position::NONE).into(),
            );
        }

        #[cfg(not(feature = "no_object"))]
        if self
            .limits
            .max_map_size
            .map_or(false, |max| _map > max.get())
        {
            return Err(
                ERR::ErrorDataTooLarge("Size of object map".to_string(), Position::NONE).into(),
            );
        }

        Ok(())
    }

    /// Check whether the size of a [`Dynamic`] is within limits.
    #[inline]
    pub(crate) fn check_data_size<T: Borrow<Dynamic>>(
        &self,
        value: T,
        pos: Position,
    ) -> RhaiResultOf<T> {
        // If no data size limits, just return
        if !self.has_data_size_limit() {
            return Ok(value);
        }

        let sizes = value.borrow().calc_data_sizes(true);

        self.raise_err_if_over_data_size_limit(sizes)
            .map(|_| value)
            .map_err(|err| err.fill_position(pos))
    }

    /// Raise an error if the size of a [`Dynamic`] is out of limits (if any).
    ///
    /// Not available under `unchecked`.
    #[inline(always)]
    pub fn ensure_data_size_within_limits(&self, value: &Dynamic) -> RhaiResultOf<()> {
        self.check_data_size(value, Position::NONE).map(|_| ())
    }

    /// Check if the number of operations stay within limit.
    pub(crate) fn track_operation(
        &self,
        global: &mut GlobalRuntimeState,
        pos: Position,
    ) -> RhaiResultOf<()> {
        global.num_operations += 1;

        // Guard against too many operations
        let max = self.max_operations();

        if max > 0 && global.num_operations > max {
            return Err(ERR::ErrorTooManyOperations(pos).into());
        }

        // Report progress
        self.progress
            .as_ref()
            .and_then(|p| p(global.num_operations))
            .map_or(Ok(()), |token| Err(ERR::ErrorTerminated(token, pos).into()))
    }
}
