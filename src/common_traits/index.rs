use crate::{elem::ConcurrentElem, ConcurrentSlice, ConcurrentVec};
use core::ops::Index;
use orx_pinned_vec::IntoConcurrentPinnedVec;

impl<P, T> Index<usize> for ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElem<T>>,
{
    type Output = ConcurrentElem<T>;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("out-of-bounds")
    }
}

impl<'a, P, T> Index<usize> for ConcurrentSlice<'a, T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElem<T>>,
{
    type Output = ConcurrentElem<T>;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("out-of-bounds")
    }
}
