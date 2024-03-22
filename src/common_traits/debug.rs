use crate::ConcurrentVec;
use orx_fixed_vec::PinnedVec;
use std::fmt::Debug;

impl<T: Debug, P: PinnedVec<Option<T>>> Debug for ConcurrentVec<T, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe { self.correct_pinned_len() };

        f.debug_struct("ConcurrentVec")
            .field("pinned", &self.iter().collect::<Vec<_>>())
            .field("len", &self.len())
            .field("capacity", &self.capacity())
            .finish()
    }
}
