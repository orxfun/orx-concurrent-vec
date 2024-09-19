use super::ConcurrentSlice;
use crate::elem::ConcurrentElem;
use orx_fixed_vec::IntoConcurrentPinnedVec;

impl<'a, T, P> ConcurrentSlice<'a, T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElem<T>>,
{
    /// Swaps two elements in the slice.
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
    /// let vec = ConcurrentVec::new();
    /// vec.extend([0, 1, 2, 3, 4, 5]);
    ///
    /// let slice = vec.slice(1..5);
    ///
    /// let swapped = slice.swap(0, 2);
    /// assert_eq!(swapped, true);
    /// assert_eq!(&slice, &[3, 2, 1, 4]);
    ///
    /// let swapped = slice.swap(0, 4); // out-of-bounds
    /// assert_eq!(swapped, false);
    /// assert_eq!(&slice, &[3, 2, 1, 4]);
    ///
    /// assert_eq!(&vec, &[0, 3, 2, 1, 4, 5]);
    /// ```
    pub fn swap(&self, i: usize, j: usize) -> bool {
        let idx_i = self.idx(i);
        let idx_j = self.idx(j);

        match (idx_i, idx_j) {
            (Some(i), Some(j)) => self.vec.swap(i, j),
            _ => false,
        }
    }

    /// Fills all positions of the slice with the given `value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter([0, 1, 2, 3]);
    ///
    /// vec.slice(2..).fill(42);
    /// assert_eq!(&vec, &[0, 1, 42, 42]);
    /// ```
    pub fn fill(&self, value: T)
    where
        T: Clone,
    {
        for elem in self.iter() {
            elem.set(value.clone());
        }
    }

    /// Fills all positions of the slice with the the values
    /// created by successively calling `value(i)` for each position.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    /// vec.extend([0, 1, 2, 3, 4, 5, 6]);
    ///
    /// let slice = vec.slice(0..=3);
    ///
    /// let mut current = 0;
    /// slice.fill_with(|i| {
    ///     current += i as i32;
    ///     current
    /// });
    /// assert_eq!(&slice, &[0, 1, 3, 6]);
    /// assert_eq!(&vec, &[0, 1, 3, 6, 4, 5, 6]);
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
