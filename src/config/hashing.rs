//! Fixed hashing seeds for stable hashing.
//!
//! Set to [`None`] to disable stable hashing.
//!
//! See [`set_rhai_ahash_seed`] for more.
//!
//! Alternatively, You can also set this at compile time by disabling `stable_hash` and setting the `RHAI_AHASH_SEED`
//! environment variable instead.
//!
//! E.g. `env RHAI_AHASH_SEED ="[236,800,954,213]"`
// [236,800,954,213], haha funny yume nikki reference epic uboachan face numberworld nexus moment 100

use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    mem::MaybeUninit,
    panic::{RefUnwindSafe, UnwindSafe},
    sync::atomic::{AtomicBool, Ordering},
};

// Safety: lol
struct SusLock<T>
where
    T: 'static,
{
    initalized: AtomicBool,
    data: UnsafeCell<MaybeUninit<T>>,
    _marker: PhantomData<T>,
}

impl<T> SusLock<T> {
    pub const fn new() -> SusLock<T> {
        SusLock {
            initalized: AtomicBool::new(false),
            data: UnsafeCell::new(MaybeUninit::uninit()),
            _marker: PhantomData,
        }
    }

    pub fn get(&self) -> Option<&T> {
        if self.initalized.load(Ordering::SeqCst) {
            Some(unsafe { (&*self.data.get()).assume_init_ref() })
        } else {
            return None;
        }
    }

    pub fn get_or_init(&self, f: impl FnOnce() -> T) -> Option<&T> {
        let value = f();
        if !self.initalized.load(Ordering::SeqCst) {
            unsafe {
                self.data.get().write(MaybeUninit::new(value));
            }
            self.initalized.store(true, Ordering::SeqCst);
        }

        self.get()
    }

    pub fn set(&self, value: T) -> Result<(), T> {
        if self.initalized.load(Ordering::SeqCst) {
            Err(value)
        } else {
            let _ = self.get_or_init(|| value);
            Ok(())
        }
    }
}

unsafe impl<T: Sync + Send> Sync for SusLock<T> {}
unsafe impl<T: Send> Send for SusLock<T> {}
impl<T: RefUnwindSafe + UnwindSafe> RefUnwindSafe for SusLock<T> {}

impl<T> Drop for SusLock<T> {
    fn drop(&mut self) {
        if self.initalized.load(Ordering::SeqCst) {
            unsafe { (&mut *self.data.get()).assume_init_drop() };
        }
    }
}

static AHASH_SEED: SusLock<Option<[u64; 4]>> = SusLock::new();

// #[doc(cfg(feature = "stable_hash"))]
/// Sets the Rhai Ahash seed. This is used to hash functions and the like.
///
/// This is a global variable, and thus will affect every Rhai instance.
/// This should not be used _unless_ you know you need it.
///
/// # Warnings
/// - You can only call this function **ONCE** for the whole of your program execution.
/// - You should gracefully handle the `Err(())`.
/// - You **MUST** call this before **ANY** Rhai operation occurs (e.g. creating an [`Engine`]).
///
/// # Errors
/// This will error if the AHashSeed is already set.
pub fn set_ahash_seed(new_seed: Option<[u64; 4]>) -> Result<(), Option<[u64; 4]>> {
    AHASH_SEED.set(new_seed)
}

/// Gets the current Rhai Ahash Seed. If the seed is not yet defined, this will automatically set a seed.
/// The default seed is not stable and may change between versions.
///
/// See [`set_rhai_ahash_seed`] for more.
pub fn get_ahash_seed() -> Option<[u64; 4]> {
    const FUNNY_YUMENIKKI_REFERENCE: Option<[u64; 4]> = None;

    AHASH_SEED
        .get_or_init(|| FUNNY_YUMENIKKI_REFERENCE)
        .map(|x| *x)
        .flatten()
}
