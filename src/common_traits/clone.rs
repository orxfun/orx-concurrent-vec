use crate::ConcurrentVec;

impl<T> Clone for ConcurrentVec<T>
where
    T: Clone,
{
    /// A thread-safe method to clone the concurrent vec.
    ///
    /// # Example
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let vec: ConcurrentVec<_> = (0..4).into_iter().collect();
    /// let clone = vec.clone();
    ///
    /// assert_eq!(&clone, &[0, 1, 2, 3]);
    /// ```
    fn clone(&self) -> Self {
        self.iter_cloned().collect()
    }
}
