use crate::{elem::ConcurrentElement, ConcurrentSlice, ConcurrentVec};
use core::ops::Index;
use orx_pinned_vec::IntoConcurrentPinnedVec;

// ConcurrentVec

impl<P, T> Index<usize> for ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    type Output = ConcurrentElement<T>;

    /// Returns a reference to the concurrent element at the i-th position of the vec.
    ///
    /// Note that `vec[i]` is a shorthand for `vec.get(i).unwrap()`.
    ///
    /// # Panics
    ///
    /// Panics if i is out of bounds.
    fn index(&self, i: usize) -> &Self::Output {
        self.get(i).expect("out-of-bounds")
    }
}

// ConcurrentSlice

impl<P, T> Index<usize> for ConcurrentSlice<'_, T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    type Output = ConcurrentElement<T>;

    /// Returns a reference to the concurrent element at the i-th position of the slice.
    ///
    /// Note that `slice[i]` is a shorthand for `slice.get(i).unwrap()`.
    ///
    /// # Panics
    ///
    /// Panics if i is out of bounds.
    fn index(&self, i: usize) -> &Self::Output {
        self.get(i).expect("out-of-bounds")
    }
}
