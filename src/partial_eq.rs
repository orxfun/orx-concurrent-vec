use crate::{elem::ConcurrentElement, ConcurrentVec};
use orx_pinned_vec::IntoConcurrentPinnedVec;

impl<T, P> ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
    T: PartialEq,
{
    /// Returns the index of the first element equal to the given `value`.
    /// Returns None if the value is absent.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter(['a', 'b', 'c']);
    ///
    /// assert_eq!(vec.index_of(&'c'), Some(2));
    /// assert_eq!(vec.index_of(&'d'), None);
    /// ```
    pub fn index_of(&self, value: &T) -> Option<usize> {
        self.iter().position(|e| e == value)
    }

    /// Returns whether an element equal to the given `value` exists or not.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter(['a', 'b', 'c']);
    ///
    /// assert_eq!(vec.contains(&'c'), true);
    /// assert_eq!(vec.contains(&'d'), false);
    /// ```
    pub fn contains(&self, value: &T) -> bool {
        self.index_of(value).is_some()
    }
}
