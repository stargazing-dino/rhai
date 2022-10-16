mod cache;
mod chaining;
mod data_check;
mod debugger;
mod eval_context;
mod expr;
mod global_state;
mod stmt;
mod target;

pub use cache::{Caches, FnResolutionCache, FnResolutionCacheEntry};
#[cfg(any(not(feature = "no_index"), not(feature = "no_object")))]
pub use chaining::ChainType;
#[cfg(feature = "debugging")]
pub use debugger::{
    BreakPoint, CallStackFrame, Debugger, DebuggerCommand, DebuggerEvent, DebuggerStatus,
    OnDebuggerCallback, OnDebuggingInit,
};
pub use eval_context::EvalContext;
#[cfg(not(feature = "no_module"))]
#[cfg(not(feature = "no_function"))]
pub use global_state::GlobalConstants;
pub use global_state::GlobalRuntimeState;
pub use target::{calc_index, calc_offset_len, Target};

#[cfg(feature = "unchecked")]
mod unchecked {
    use crate::{eval::GlobalRuntimeState, Dynamic, Engine, Position, RhaiResult, RhaiResultOf};

    impl Engine {
        /// Check if the number of operations stay within limit.
        #[inline(always)]
        pub(crate) const fn track_operation(
            &self,
            _: &GlobalRuntimeState,
            _: Position,
        ) -> RhaiResultOf<()> {
            Ok(())
        }

        /// Check whether the size of a [`Dynamic`] is within limits.
        #[inline(always)]
        pub(crate) const fn check_data_size(&self, _: &Dynamic, _: Position) -> RhaiResultOf<()> {
            Ok(())
        }

        /// Check a result to ensure that it is valid.
        #[inline(always)]
        pub(crate) const fn check_return_value(
            &self,
            result: RhaiResult,
            _: Position,
        ) -> RhaiResult {
            result
        }
    }
}
