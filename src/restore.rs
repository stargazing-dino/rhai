//! Facility to run state restoration logic at the end of scope.

use std::ops::{Deref, DerefMut};
#[cfg(feature = "no_std")]
use std::prelude::v1::*;

/// Automatically restore state at the end of the scope.
macro_rules! auto_restore {
    (let $temp:ident = $var:ident . $prop:ident; $code:stmt) => {
        auto_restore!(let $temp = $var.$prop; $code => move |v| v.$prop = $temp);
    };
    (let $temp:ident = $var:ident . $prop:ident; $code:stmt => $restore:expr) => {
        let $temp = $var.$prop;
        $code
        auto_restore!($var => $restore);
    };
    ($var:ident => $restore:ident; let $temp:ident = $save:expr;) => {
        auto_restore!($var => $restore; let $temp = $save; {});
    };
    ($var:ident if $guard:expr => $restore:ident; let $temp:ident = $save:expr;) => {
        auto_restore!($var if $guard => $restore; let $temp = $save; {});
    };
    ($var:ident => $restore:ident; let $temp:ident = $save:expr; $code:stmt) => {
        let $temp = $save;
        $code
        auto_restore!($var => move |v| { v.$restore($temp); });
    };
    ($var:ident if $guard:expr => $restore:ident; let $temp:ident = $save:expr; $code:stmt) => {
        let $temp = $save;
        $code
        auto_restore!($var if $guard => move |v| { v.$restore($temp); });
    };
    ($var:ident => $restore:expr) => {
        auto_restore!($var = $var => $restore);
    };
    ($var:ident = $value:expr => $restore:expr) => {
        let $var = &mut *crate::RestoreOnDrop::lock($value, $restore);
    };
    ($var:ident if Some($guard:ident) => $restore:expr) => {
        auto_restore!($var = ($var) if Some($guard) => $restore);
    };
    ($var:ident = ( $value:expr ) if Some($guard:ident) => $restore:expr) => {
        let mut __rx__;
        let $var = if let Some($guard) = $guard {
            __rx__ = crate::RestoreOnDrop::lock($value, $restore);
            &mut *__rx__
        } else {
            &mut *$value
        };
    };
    ($var:ident if $guard:expr => $restore:expr) => {
        auto_restore!($var = ($var) if $guard => $restore);
    };
    ($var:ident = ( $value:expr ) if $guard:expr => $restore:expr) => {
        let mut __rx__;
        let $var = if $guard {
            __rx__ = crate::RestoreOnDrop::lock($value, $restore);
            &mut *__rx__
        } else {
            &mut *$value
        };
    };
}

/// Run custom restoration logic upon the end of scope.
#[must_use]
pub struct RestoreOnDrop<'a, T: ?Sized, R: FnOnce(&mut T)> {
    value: &'a mut T,
    restore: Option<R>,
}

impl<'a, T: ?Sized, R: FnOnce(&mut T)> RestoreOnDrop<'a, T, R> {
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

impl<'a, T: ?Sized, R: FnOnce(&mut T)> Drop for RestoreOnDrop<'a, T, R> {
    #[inline(always)]
    fn drop(&mut self) {
        self.restore.take().unwrap()(self.value);
    }
}

impl<'a, T: ?Sized, R: FnOnce(&mut T)> Deref for RestoreOnDrop<'a, T, R> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'a, T: ?Sized, R: FnOnce(&mut T)> DerefMut for RestoreOnDrop<'a, T, R> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}
