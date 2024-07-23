use orx_pinned_concurrent_col::{ConcurrentState, PinnedConcurrentCol, WritePermit};
use orx_pinned_vec::ConcurrentPinnedVec;
use std::{
    cmp::Ordering,
    sync::atomic::{self, AtomicUsize},
};

#[derive(Debug)]
pub struct ConcurrentVecState {
    len: AtomicUsize,
    max_written_len: AtomicUsize,
}

impl ConcurrentState for ConcurrentVecState {
    #[inline(always)]
    fn zero_memory(&self) -> bool {
        false
    }

    fn new_for_pinned_vec<T, P: orx_fixed_vec::prelude::PinnedVec<T>>(pinned_vec: &P) -> Self {
        Self {
            len: pinned_vec.len().into(),
            max_written_len: pinned_vec.len().into(),
        }
    }

    fn new_for_con_pinned_vec<T, P: ConcurrentPinnedVec<T>>(_: &P, len: usize) -> Self {
        Self {
            len: len.into(),
            max_written_len: len.into(),
        }
    }

    fn write_permit<T, P, S>(&self, col: &PinnedConcurrentCol<T, P, S>, idx: usize) -> WritePermit
    where
        P: ConcurrentPinnedVec<T>,
        S: ConcurrentState,
    {
        let capacity = col.capacity();

        match idx.cmp(&capacity) {
            Ordering::Less => WritePermit::JustWrite,
            Ordering::Equal => WritePermit::GrowThenWrite,
            Ordering::Greater => WritePermit::Spin,
        }
    }

    fn write_permit_n_items<T, P, S>(
        &self,
        col: &PinnedConcurrentCol<T, P, S>,
        begin_idx: usize,
        num_items: usize,
    ) -> WritePermit
    where
        P: ConcurrentPinnedVec<T>,
        S: ConcurrentState,
    {
        let capacity = col.capacity();
        let last_idx = begin_idx + num_items - 1;

        match (begin_idx.cmp(&capacity), last_idx.cmp(&capacity)) {
            (_, std::cmp::Ordering::Less) => WritePermit::JustWrite,
            (std::cmp::Ordering::Greater, _) => WritePermit::Spin,
            _ => WritePermit::GrowThenWrite,
        }
    }

    #[inline(always)]
    fn release_growth_handle(&self) {}

    #[inline(always)]
    fn update_after_write(&self, _: usize, end_idx: usize) {
        self.max_written_len
            .store(end_idx, atomic::Ordering::SeqCst);
    }

    fn try_get_no_gap_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl ConcurrentVecState {
    #[inline(always)]
    pub(crate) fn fetch_increment_len(&self, increment_by: usize) -> usize {
        self.len.fetch_add(increment_by, atomic::Ordering::SeqCst)
    }

    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        self.len.load(atomic::Ordering::SeqCst)
    }

    #[inline(always)]
    pub(crate) fn max_written_len(&self) -> usize {
        self.max_written_len.load(atomic::Ordering::SeqCst)
    }
}
