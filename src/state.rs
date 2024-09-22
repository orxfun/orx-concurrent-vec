use crate::elem::ConcurrentElement;
use core::{
    cmp::Ordering,
    sync::atomic::{self, AtomicUsize},
};
use orx_concurrent_option::ConcurrentOption;
use orx_pinned_concurrent_col::{ConcurrentState, PinnedConcurrentCol, WritePermit};
use orx_pinned_vec::{ConcurrentPinnedVec, PinnedVec};

#[derive(Debug)]
pub struct ConcurrentVecState {
    pub(super) len_reserved: AtomicUsize,
    pub(super) len_written: AtomicUsize,
}

impl<T> ConcurrentState<ConcurrentElement<T>> for ConcurrentVecState {
    fn fill_memory_with(&self) -> Option<fn() -> ConcurrentElement<T>> {
        Some(|| ConcurrentElement(ConcurrentOption::none()))
    }

    fn new_for_pinned_vec<P: PinnedVec<ConcurrentElement<T>>>(pinned_vec: &P) -> Self {
        Self {
            len_reserved: pinned_vec.len().into(),
            len_written: pinned_vec.len().into(),
        }
    }

    fn new_for_con_pinned_vec<P: ConcurrentPinnedVec<ConcurrentElement<T>>>(
        _: &P,
        len: usize,
    ) -> Self {
        Self {
            len_reserved: len.into(),
            len_written: len.into(),
        }
    }

    fn write_permit<P>(
        &self,
        col: &PinnedConcurrentCol<ConcurrentElement<T>, P, Self>,
        idx: usize,
    ) -> WritePermit
    where
        P: ConcurrentPinnedVec<ConcurrentElement<T>>,
    {
        let capacity = col.capacity();

        match idx.cmp(&capacity) {
            Ordering::Less => WritePermit::JustWrite,
            Ordering::Equal => WritePermit::GrowThenWrite,
            Ordering::Greater => WritePermit::Spin,
        }
    }

    fn write_permit_n_items<P>(
        &self,
        col: &PinnedConcurrentCol<ConcurrentElement<T>, P, Self>,
        begin_idx: usize,
        num_items: usize,
    ) -> WritePermit
    where
        P: ConcurrentPinnedVec<ConcurrentElement<T>>,
    {
        let capacity = col.capacity();
        let last_idx = begin_idx + num_items - 1;

        match (begin_idx.cmp(&capacity), last_idx.cmp(&capacity)) {
            (_, core::cmp::Ordering::Less) => WritePermit::JustWrite,
            (core::cmp::Ordering::Greater, _) => WritePermit::Spin,
            _ => WritePermit::GrowThenWrite,
        }
    }

    fn release_growth_handle(&self) {}

    #[inline(always)]
    fn update_after_write(&self, _: usize, _: usize) {}

    fn try_get_no_gap_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl ConcurrentVecState {
    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        self.len_reserved.load(atomic::Ordering::Relaxed)
    }
}
