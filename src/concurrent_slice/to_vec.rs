use super::ConcurrentSlice;
use crate::elem::ConcurrentElement;
use alloc::vec::Vec;
use orx_fixed_vec::IntoConcurrentPinnedVec;

impl<T, P> ConcurrentSlice<'_, T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    /// Clones the values of elements of the slice into a regular vector.
    pub fn clone_to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        let iter = self.iter_cloned();
        let mut vec = Vec::with_capacity(self.len());
        for x in iter {
            vec.push(x);
        }
        vec
    }
}
