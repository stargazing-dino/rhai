//! Facility to run state restoration logic at the end of scope.

use std::ops::{Deref, DerefMut};
#[cfg(feature = "no_std")]
use std::prelude::v1::*;

/// Run custom restoration logic upon the end of scope.
#[must_use]
pub struct RestoreOnDrop<'a, T, R: FnOnce(&mut T)> {
    value: &'a mut T,
    restore: Option<R>,
}

impl<'a, T, R: FnOnce(&mut T)> RestoreOnDrop<'a, T, R> {
    /// Create a new [`RestoreOnDrop`] that locks a mutable reference and runs restoration logic at
    /// the end of scope only when `need_restore` is `true`.
    ///
    /// Beware that the end of scope means the end of its lifetime, not necessarily waiting until
    /// the current block scope is exited.
    #[inline(always)]
    pub fn lock_if(need_restore: bool, value: &'a mut T, restore: R) -> Self {
        Self {
            value,
            restore: if need_restore { Some(restore) } else { None },
        }
    }

    /// Create a new [`RestoreOnDrop`] that locks a mutable reference and runs restoration logic at
    /// the end of scope.
    ///
    /// Beware that the end of scope means the end of its lifetime, not necessarily waiting until
    /// the current block scope is exited.
    #[inline(always)]
    pub fn lock(value: &'a mut T, restore: R) -> Self {
        Self {
            value,
            restore: Some(restore),
        }
    }
}

impl<'a, T, R: FnOnce(&mut T)> Drop for RestoreOnDrop<'a, T, R> {
    #[inline(always)]
    fn drop(&mut self) {
        if let Some(restore) = self.restore.take() {
            restore(self.value);
        }
    }
}

impl<'a, T, R: FnOnce(&mut T)> Deref for RestoreOnDrop<'a, T, R> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'a, T, R: FnOnce(&mut T)> DerefMut for RestoreOnDrop<'a, T, R> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}
