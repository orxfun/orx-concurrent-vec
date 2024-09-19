use crate::{elem::ConcurrentElem, ConcurrentVec};
use orx_concurrent_option::SOME;
use orx_pinned_vec::IntoConcurrentPinnedVec;

impl<T, P> ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElem<T>>,
{
    /// Swaps two elements in the vector.
    ///
    /// Returns:
    /// * true of both `i` and `j` are in bounds and values are swapped,
    /// * false if at least one of the indices is out of bounds.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter([0, 1, 2, 3]);
    ///
    /// let swapped = vec.swap(0, 2);
    /// assert_eq!(swapped, true);
    /// assert_eq!(&vec, &[2, 1, 0, 3]);
    ///
    /// let swapped = vec.swap(0, 4);
    /// assert_eq!(swapped, false);
    /// assert_eq!(&vec, &[2, 1, 0, 3]);
    /// ```
    pub fn swap(&self, i: usize, j: usize) -> bool {
        let h_i = unsafe { self.mut_handle(i, SOME, SOME) };
        let h_j = unsafe { self.mut_handle(j, SOME, SOME) };

        match (h_i, h_j) {
            (Some(h_i), Some(h_j)) => {
                let mut_i = unsafe { h_i.get_mut() };
                let mut_j = unsafe { h_j.get_mut() };
                core::mem::swap(mut_i, mut_j);
                true
            }
            _ => false,
        }
    }

    /// Fills all positions of the vec with the given `value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter([0, 1, 2, 3]);
    ///
    /// vec.fill(42);
    /// assert_eq!(&vec, &[42, 42, 42, 42]);
    /// ```
    pub fn fill(&self, value: T)
    where
        T: Clone,
    {
        for elem in self.iter() {
            elem.set(value.clone());
        }
    }

    /// Fills all positions of the vec with the the values
    /// created by successively calling `value(i)` for each position.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter([0, 1, 2, 3]);
    ///
    /// let mut current = 0;
    /// vec.fill_with(|i| {
    ///     current += i as i32;
    ///     current
    /// });
    /// assert_eq!(&vec, &[0, 1, 3, 6]);
    /// ```
    pub fn fill_with<F>(&self, mut value: F)
    where
        F: FnMut(usize) -> T,
    {
        for (i, elem) in self.iter().enumerate() {
            elem.set(value(i));
        }
    }
}
