use crate::{ConcurrentVec, elem::ConcurrentElement};
use alloc::vec::Vec;
use orx_pinned_vec::IntoConcurrentPinnedVec;

impl<T, P> ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    /// Transforms the concurrent vec into a regular vector.
    pub fn to_vec(self) -> Vec<T> {
        let pinned = self.into_inner();
        let mut vec = Vec::with_capacity(pinned.len());
        for x in pinned {
            vec.push(x.0.expect("unwrapped none"));
        }
        vec
    }

    /// Without consuming the concurrent vector,
    /// clones the values of its elements into a regular vector.
    pub fn clone_to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.as_slice().clone_to_vec()
    }
}
