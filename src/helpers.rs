use crate::{ConcurrentVec, elem::ConcurrentElement};
use core::sync::atomic::AtomicUsize;
use orx_concurrent_option::{MutHandle, StateU8};
use orx_pinned_vec::IntoConcurrentPinnedVec;
use orx_split_vec::{Doubling, SplitVec};

pub(crate) type DefaultPinVec<T> = SplitVec<ConcurrentElement<T>, Doubling>;

impl<T, P> ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    #[inline(always)]
    pub(crate) fn len_reserved(&self) -> &AtomicUsize {
        &self.core.state().len_reserved
    }

    #[inline(always)]
    pub(crate) fn len_written(&self) -> &AtomicUsize {
        &self.core.state().len_written
    }

    #[inline(always)]
    pub(crate) unsafe fn mut_handle(
        &self,
        i: usize,
        initial_state: StateU8,
        success_state: StateU8,
    ) -> Option<MutHandle<T>> {
        match i < self.len() {
            true => unsafe {
                self.core
                    .get(i)
                    .and_then(|e| e.0.mut_handle(initial_state, success_state))
            },
            false => None,
        }
    }
}
