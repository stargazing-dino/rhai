//! Fixed hashing seeds for stable hashing.
//!
//! Set to [`None`] to disable stable hashing.
//!
//! See [`set_rhai_ahash_seed`].
//!
//! # Example
//!
//! ```rust
//! // Set the hashing seed to [1, 2, 3, 4]
//! rhai::config::hashing::set_ahash_seed(Some([1, 2, 3, 4])).unwrap();
//! ```
//! Alternatively, set this at compile time via the `RHAI_AHASH_SEED` environment variable.
//!
//! # Example
//!
//! ```sh
//! env RHAI_AHASH_SEED ="[236,800,954,213]"
//! ```
// [236,800,954,213], haha funny yume nikki reference epic uboachan face numberworld nexus moment 100

use crate::config::hashing_env;
use core::panic::{RefUnwindSafe, UnwindSafe};
#[cfg(feature = "no_std")]
use std::prelude::v1::*;
use std::{
    cell::UnsafeCell,
    marker::PhantomData,
    mem,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

// omg its hokma from record team here to record our locks
// what does this do?
// so what this does is keep track of a global address in memory that acts as a global lock
// i stole this from crossbeam so read their docs for more
#[must_use]
struct HokmaLock {
    lock: AtomicUsize,
}

impl HokmaLock {
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            lock: AtomicUsize::new(0),
        }
    }

    pub fn write(&'static self) -> WhenTheHokmaSuppression {
        loop {
            let previous = self.lock.swap(1, Ordering::SeqCst);

            if previous != 1 {
                return WhenTheHokmaSuppression {
                    hokma: self,
                    state: previous,
                };
            }
        }
    }
}

struct WhenTheHokmaSuppression {
    hokma: &'static HokmaLock,
    state: usize,
}

impl WhenTheHokmaSuppression {
    #[inline]
    pub fn the_price_of_silence(self) {
        self.hokma.lock.store(self.state, Ordering::SeqCst);
        mem::forget(self)
    }
}

impl Drop for WhenTheHokmaSuppression {
    #[inline]
    fn drop(&mut self) {
        self.hokma
            .lock
            .store(self.state.wrapping_add(2), Ordering::SeqCst)
    }
}

#[inline(always)]
#[must_use]
fn hokmalock(address: usize) -> &'static HokmaLock {
    const LEN: usize = 787;
    const LCK: HokmaLock = HokmaLock::new();
    static RECORDS: [HokmaLock; LEN] = [LCK; LEN];

    &RECORDS[address % LEN]
}

// Safety: lol, there is a reason its called "SusLock<T>"
#[must_use]
struct SusLock<T>
where
    T: 'static + Copy,
{
    initialized: AtomicBool,
    data: UnsafeCell<MaybeUninit<T>>,
    _marker: PhantomData<T>,
}

impl<T> SusLock<T>
where
    T: 'static + Copy,
{
    #[inline]
    pub const fn new() -> SusLock<T> {
        SusLock {
            initialized: AtomicBool::new(false),
            data: UnsafeCell::new(MaybeUninit::uninit()),
            _marker: PhantomData,
        }
    }

    #[must_use]
    pub fn get(&self) -> Option<T> {
        if self.initialized.load(Ordering::SeqCst) {
            let hokma = hokmalock(unsafe { mem::transmute(self.data.get()) });
            // we forgo the optimistic read, because we don't really care
            let guard = hokma.write();
            let val = {
                let cast: *const T = self.data.get().cast();
                unsafe { cast.read() }
            };
            guard.the_price_of_silence();
            Some(val)
        } else {
            return None;
        }
    }

    #[must_use]
    pub fn get_or_init(&self, f: impl FnOnce() -> T) -> Option<T> {
        let value = f();
        if !self.initialized.load(Ordering::SeqCst) {
            self.initialized.store(true, Ordering::SeqCst);
            let hokma = hokmalock(unsafe { mem::transmute(self.data.get()) });
            hokma.write();
            unsafe {
                self.data.get().write(MaybeUninit::new(value));
            }
        }

        self.get()
    }

    pub fn set(&self, value: T) -> Result<(), T> {
        if self.initialized.load(Ordering::SeqCst) {
            Err(value)
        } else {
            let _ = self.get_or_init(|| value);
            Ok(())
        }
    }
}

unsafe impl<T: Sync + Send> Sync for SusLock<T> where T: 'static + Copy {}
unsafe impl<T: Send> Send for SusLock<T> where T: 'static + Copy {}
impl<T: RefUnwindSafe + UnwindSafe> RefUnwindSafe for SusLock<T> where T: 'static + Copy {}

impl<T> Drop for SusLock<T>
where
    T: 'static + Copy,
{
    #[inline]
    fn drop(&mut self) {
        if self.initialized.load(Ordering::SeqCst) {
            unsafe { (&mut *self.data.get()).assume_init_drop() };
        }
    }
}

static AHASH_SEED: SusLock<Option<[u64; 4]>> = SusLock::new();

/// Set the hashing seed. This is used to hash functions etc.
///
/// This is a static global value and affects every Rhai instance.
/// This should not be used _unless_ you know you need it.
///
/// # Warning
///
/// * You can only call this function **ONCE** for the entire duration of program execution.
/// * You **MUST** call this before performing **ANY** Rhai operation (e.g. creating an [`Engine`]).
///
/// # Error
///
/// Returns an error containing the existing hashing seed if already set.
///
/// # Example
///
/// ```rust
/// # use rhai::Engine;
/// // Set the hashing seed to [1, 2, 3, 4]
/// rhai::config::hashing::set_ahash_seed(Some([1, 2, 3, 4])).unwrap();
///
/// // Use Rhai AFTER setting the hashing seed
/// let engine = Engine::new();
/// ```
#[inline(always)]
pub fn set_ahash_seed(new_seed: Option<[u64; 4]>) -> Result<(), Option<[u64; 4]>> {
    AHASH_SEED.set(new_seed)
}

/// Get the current hashing Seed.
///
/// If the seed is not yet defined, the `RHAI_AHASH_SEED` environment variable (if any) is used.
///
/// Otherwise, the hashing seed is randomized to protect against DOS attacks.
///
/// See [`set_rhai_ahash_seed`] for more.
#[inline]
#[must_use]
pub fn get_ahash_seed() -> Option<[u64; 4]> {
    AHASH_SEED.get_or_init(|| hashing_env::AHASH_SEED).flatten()
}
