//! Data size checks during evaluation.
#![cfg(not(feature = "unchecked"))]

use super::GlobalRuntimeState;
use crate::types::dynamic::Union;
use crate::{Dynamic, Engine, Position, RhaiResult, RhaiResultOf, ERR};
#[cfg(feature = "no_std")]
use std::prelude::v1::*;

impl Engine {
    /// Recursively calculate the sizes of a value.
    ///
    /// Sizes returned are `(` [`Array`][crate::Array], [`Map`][crate::Map] and [`String`] `)`.
    ///
    /// # Panics
    ///
    /// Panics if any interior data is shared (should never happen).
    pub(crate) fn calc_data_sizes(value: &Dynamic, _top: bool) -> (usize, usize, usize) {
        match value.0 {
            #[cfg(not(feature = "no_index"))]
            Union::Array(ref arr, ..) => {
                arr.iter()
                    .fold((0, 0, 0), |(ax, mx, sx), value| match value.0 {
                        Union::Array(..) => {
                            let (a, m, s) = Self::calc_data_sizes(value, false);
                            (ax + a + 1, mx + m, sx + s)
                        }
                        Union::Blob(ref a, ..) => (ax + 1 + a.len(), mx, sx),
                        #[cfg(not(feature = "no_object"))]
                        Union::Map(..) => {
                            let (a, m, s) = Self::calc_data_sizes(value, false);
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
                            let (a, m, s) = Self::calc_data_sizes(value, false);
                            (ax + a, mx + m + 1, sx + s)
                        }
                        #[cfg(not(feature = "no_index"))]
                        Union::Blob(ref a, ..) => (ax + a.len(), mx, sx),
                        Union::Map(..) => {
                            let (a, m, s) = Self::calc_data_sizes(value, false);
                            (ax + a, mx + m + 1, sx + s)
                        }
                        Union::Str(ref s, ..) => (ax, mx + 1, sx + s.len()),
                        _ => (ax, mx + 1, sx),
                    })
            }
            Union::Str(ref s, ..) => (0, 0, s.len()),
            #[cfg(not(feature = "no_closure"))]
            Union::Shared(..) if _top => {
                Self::calc_data_sizes(&*value.read_lock::<Dynamic>().unwrap(), true)
            }
            #[cfg(not(feature = "no_closure"))]
            Union::Shared(..) => {
                unreachable!("shared values discovered within data: {}", value)
            }
            _ => (0, 0, 0),
        }
    }

    /// Raise an error if any data size exceeds limit.
    pub(crate) fn raise_err_if_over_data_size_limit(
        &self,
        (_arr, _map, s): (usize, usize, usize),
        pos: Position,
    ) -> RhaiResultOf<()> {
        if self
            .limits
            .max_string_size
            .map_or(false, |max| s > max.get())
        {
            return Err(ERR::ErrorDataTooLarge("Length of string".to_string(), pos).into());
        }

        #[cfg(not(feature = "no_index"))]
        if self
            .limits
            .max_array_size
            .map_or(false, |max| _arr > max.get())
        {
            return Err(ERR::ErrorDataTooLarge("Size of array".to_string(), pos).into());
        }

        #[cfg(not(feature = "no_object"))]
        if self
            .limits
            .max_map_size
            .map_or(false, |max| _map > max.get())
        {
            return Err(ERR::ErrorDataTooLarge("Size of object map".to_string(), pos).into());
        }

        Ok(())
    }

    /// Check whether the size of a [`Dynamic`] is within limits.
    pub(crate) fn check_data_size(&self, value: &Dynamic, pos: Position) -> RhaiResultOf<()> {
        // If no data size limits, just return
        if !self.has_data_size_limit() {
            return Ok(());
        }

        let sizes = Self::calc_data_sizes(value, true);

        self.raise_err_if_over_data_size_limit(sizes, pos)
    }

    /// Raise an error if the size of a [`Dynamic`] is out of limits (if any).
    ///
    /// Not available under `unchecked`.
    #[inline(always)]
    pub fn ensure_data_size_within_limits(&self, value: &Dynamic) -> RhaiResultOf<()> {
        self.check_data_size(value, Position::NONE)
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
        let num_operations = global.num_operations;

        if max > 0 && num_operations > max {
            return Err(ERR::ErrorTooManyOperations(pos).into());
        }

        // Report progress - only in steps
        if let Some(ref progress) = self.progress {
            if let Some(token) = progress(num_operations) {
                // Terminate script if progress returns a termination token
                return Err(ERR::ErrorTerminated(token, pos).into());
            }
        }

        Ok(())
    }

    /// Check a result to ensure that it is valid.
    #[inline]
    pub(crate) fn check_return_value(&self, result: RhaiResult, pos: Position) -> RhaiResult {
        if let Ok(ref r) = result {
            self.check_data_size(r, pos)?;
        }

        result
    }
}
