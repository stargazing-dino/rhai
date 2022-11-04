//! Fixed hashing seeds for stable hashing.
//!
//! Set to [`None`] to disable stable hashing.
//!
//! See [`set_rhai_ahash_seed`] for more.
//!
//! Alternatively, You can also set this at compile time by setting the `RHAI_AHASH_SEED`
//! environment variable instead.
//!
//! E.g. `env RHAI_AHASH_SEED ="[236,800,954,213]"`
// [236,800,954,213], haha funny yume nikki reference epic uboachan face numberworld nexus moment 100

use crate::config::hashing_env;
use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    mem::MaybeUninit,
    panic::{RefUnwindSafe, UnwindSafe},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

// omg its hokma from record team here to record our locks
// what does this do?
// so what this does is keep track of a global address in memory that acts as a global lock
// i stole this from crossbeam so read their docs for more
struct HokmaLock {
    lock: AtomicUsize,
}

impl HokmaLock {
    pub const fn new() -> Self {
        Self {
            lock: AtomicUsize::new(0),
        }
    }

    pub fn write(&'static self) -> WhenTheHokmaSupression {
        loop {
            let previous = self.lock.swap(1, Ordering::SeqCst);

            if previous != 1 {
                return WhenTheHokmaSupression {
                    hokma: self,
                    state: previous,
                };
            }
        }
    }
}

struct WhenTheHokmaSupression {
    hokma: &'static HokmaLock,

    state: usize,
}

impl WhenTheHokmaSupression {
    pub fn the_price_of_silence(self) {
        self.hokma.lock.store(self.state, Ordering::SeqCst);

        core::mem::forget(self)
    }
}

impl Drop for WhenTheHokmaSupression {
    fn drop(&mut self) {
        self.hokma
            .lock
            .store(self.state.wrapping_add(2), Ordering::SeqCst)
    }
}

fn hokmalock(address: usize) -> &'static HokmaLock {
    const LEN: usize = 787;
    const LCK: HokmaLock = HokmaLock::new();
    static RECORDS: [HokmaLock; LEN] = [LCK; LEN];

    &RECORDS[address % LEN]
}

// Safety: lol, there is a reason its called "SusLock<T>"
struct SusLock<T>
where
    T: 'static + Copy,
{
    initalized: AtomicBool,
    data: UnsafeCell<MaybeUninit<T>>,
    _marker: PhantomData<T>,
}

impl<T> SusLock<T>
where
    T: 'static + Copy,
{
    pub const fn new() -> SusLock<T> {
        SusLock {
            initalized: AtomicBool::new(false),
            data: UnsafeCell::new(MaybeUninit::uninit()),
            _marker: PhantomData,
        }
    }

    pub fn get(&self) -> Option<T> {
        if self.initalized.load(Ordering::SeqCst) {
            let hokma = hokmalock(unsafe { core::mem::transmute(self.data.get()) });
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

    pub fn get_or_init(&self, f: impl FnOnce() -> T) -> Option<T> {
        let value = f();
        if !self.initalized.load(Ordering::SeqCst) {
            self.initalized.store(true, Ordering::SeqCst);
            let hokma = hokmalock(unsafe { core::mem::transmute(self.data.get()) });
            hokma.write();
            unsafe {
                self.data.get().write(MaybeUninit::new(value));
            }
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

unsafe impl<T: Sync + Send> Sync for SusLock<T> where T: 'static + Copy {}
unsafe impl<T: Send> Send for SusLock<T> where T: 'static + Copy {}
impl<T: RefUnwindSafe + UnwindSafe> RefUnwindSafe for SusLock<T> where T: 'static + Copy {}

impl<T> Drop for SusLock<T>
where
    T: 'static + Copy,
{
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
    AHASH_SEED.get_or_init(|| hashing_env::AHASH_SEED).flatten()
}
