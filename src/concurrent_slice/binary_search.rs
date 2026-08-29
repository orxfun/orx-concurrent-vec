use super::ConcurrentSlice;
use crate::elem::ConcurrentElement;
use core::cmp::Ordering;
use orx_pinned_vec::IntoConcurrentPinnedVec;

impl<'a, T, P> ConcurrentSlice<'a, T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    /// Binary searches this slice for a given element using a comparator function.
    ///
    /// This method assumes that the slice is sorted in ascending order according to the
    /// comparator function. If the slice is not sorted, the result is unspecified.
    ///
    /// Note: This method snapshots the slice length at the start of the search to handle
    /// concurrent growth safely. If the vector grows during the search, the search will only
    /// consider elements that existed at the start.
    ///
    /// If the value is found, returns `Ok(index)` where `index` is the position of the element
    /// in the slice. If the value is not found, returns `Err(insertion_point)` where
    /// `insertion_point` is the index where the element should be inserted to maintain order.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    /// use std::cmp::Ordering;
    ///
    /// let vec = ConcurrentVec::from_iter([1, 3, 5, 7, 9]);
    /// let slice = vec.as_slice();
    ///
    /// // Found: element exists at index 2
    /// assert_eq!(slice.binary_search_by(|x| x.map(|val| {
    ///     if *val == 5 { Ordering::Equal } else if *val < 5 { Ordering::Less } else { Ordering::Greater }
    /// })), Ok(2));
    ///
    /// // Not found: element should be inserted at index 3
    /// assert_eq!(slice.binary_search_by(|x| x.map(|val| {
    ///     if *val == 6 { Ordering::Equal } else if *val < 6 { Ordering::Less } else { Ordering::Greater }
    /// })), Err(3));
    /// ```
    pub fn binary_search_by<F>(&self, mut f: F) -> Result<usize, usize>
    where
        F: FnMut(&ConcurrentElement<T>) -> Ordering,
    {
        // Snapshot the length at the start to handle concurrent growth
        let len = self.len;
        let mut left = 0;
        let mut right = len;

        while left < right {
            let mid = left + (right - left) / 2;

            // SAFETY: can unwrap because mid is guaranteed to be < len (which is self.len)
            #[allow(clippy::missing_panics_doc)]
            let elem = unsafe { self.vec.core.get(self.a + mid) }.expect("mid index out of bounds");
            let cmp = f(elem);

            match cmp {
                Ordering::Less => left = mid + 1,
                Ordering::Greater => right = mid,
                Ordering::Equal => return Ok(mid),
            }
        }

        Err(left)
    }
}

// Helper implementations that delegate to binary_search_by
impl<'a, T, P> ConcurrentSlice<'a, T, P>
where
    T: Ord,
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    /// Binary searches this slice for a given element.
    ///
    /// This method assumes that the slice is sorted in ascending order.
    /// If the slice is not sorted, the result is unspecified.
    ///
    /// If the value is found, returns `Ok(index)` where `index` is the position of the element
    /// in the slice. If the value is not found, returns `Err(insertion_point)` where
    /// `insertion_point` is the index where the element should be inserted to maintain order.
    ///
    /// This is a convenience method that delegates to `binary_search_by`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter([1, 3, 5, 7, 9]);
    /// let slice = vec.as_slice();
    ///
    /// // Found: element exists at index 2
    /// assert_eq!(slice.binary_search(&5), Ok(2));
    ///
    /// // Not found: element should be inserted at index 3
    /// assert_eq!(slice.binary_search(&6), Err(3));
    /// ```
    pub fn binary_search(&self, x: &T) -> Result<usize, usize> {
        self.binary_search_by(|elem| elem.map(|val| val.cmp(x)))
    }
}

impl<'a, T, P> ConcurrentSlice<'a, T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    /// Binary searches this slice for a given key using a key extraction function.
    ///
    /// This method assumes that the slice is sorted by the extracted keys in ascending order.
    /// If the slice is not sorted, the result is unspecified.
    ///
    /// If a value with the matching key is found, returns `Ok(index)` where `index` is the
    /// position of the element in the slice. If no matching value is found, returns `Err(insertion_point)`
    /// where `insertion_point` is the index where an element with the matching key should be
    /// inserted to maintain order.
    ///
    /// This is a convenience method that delegates to `binary_search_by`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// #[derive(Clone)]
    /// struct Pair {
    ///     key: i32,
    ///     value: &'static str,
    /// }
    ///
    /// let vec = ConcurrentVec::from_iter(vec![
    ///     Pair { key: 1, value: "a" },
    ///     Pair { key: 3, value: "b" },
    ///     Pair { key: 5, value: "c" },
    /// ]);
    /// let slice = vec.as_slice();
    ///
    /// // Found: element with key 3 exists at index 1
    /// assert_eq!(slice.binary_search_by_key(&3, |elem| {
    ///     elem.map(|pair| pair.key)
    /// }), Ok(1));
    ///
    /// // Not found: element with key 4 should be inserted at index 2
    /// assert_eq!(slice.binary_search_by_key(&4, |elem| {
    ///     elem.map(|pair| pair.key)
    /// }), Err(2));
    /// ```
    pub fn binary_search_by_key<K, F>(&self, b: &K, mut f: F) -> Result<usize, usize>
    where
        K: Ord,
        F: FnMut(&ConcurrentElement<T>) -> K,
    {
        self.binary_search_by(|elem| f(elem).cmp(b))
    }
}
