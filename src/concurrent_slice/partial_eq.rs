use super::ConcurrentSlice;
use crate::elem::ConcurrentElement;
use orx_fixed_vec::IntoConcurrentPinnedVec;

impl<'a, T, P> ConcurrentSlice<'a, T, P>
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
    /// let vec = ConcurrentVec::new();
    /// vec.extend(['a', 'b', 'c', 'd', 'e']);
    ///
    /// let slice = vec.slice(0..3);
    ///
    /// assert_eq!(slice.index_of(&'c'), Some(2));
    /// assert_eq!(slice.index_of(&'d'), None);
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
    /// let vec = ConcurrentVec::new();
    /// vec.extend(['a', 'b', 'c', 'd', 'e']);
    ///
    /// let slice = vec.slice(0..3);
    ///
    /// assert_eq!(slice.contains(&'c'), true);
    /// assert_eq!(slice.contains(&'d'), false);
    /// ```
    pub fn contains(&self, value: &T) -> bool {
        self.index_of(value).is_some()
    }
}
