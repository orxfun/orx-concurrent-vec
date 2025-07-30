use crate::{ConcurrentSlice, ConcurrentVec, elem::ConcurrentElement};
use orx_pinned_vec::IntoConcurrentPinnedVec;

impl<T, P> ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    /// Divides one slice into two at an index:
    /// * the first will contain elements in positions `[0, mid)`,
    /// * the second will contain elements in positions `[mid, self.len())`.
    ///
    /// # Panics
    ///
    /// Panics if `mid > self.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter(0..8);
    ///
    /// let (a, b) = vec.split_at(3);
    /// assert_eq!(a, [0, 1, 2]);
    /// assert_eq!(b, [3, 4, 5, 6, 7]);
    /// ```
    pub fn split_at(&self, mid: usize) -> (ConcurrentSlice<T, P>, ConcurrentSlice<T, P>) {
        assert!(mid <= self.len());
        (self.slice(0..mid), self.slice(mid..))
    }

    /// Returns the first and all the rest of the elements of the slice, or None if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter(0..4);
    ///
    /// let (a, b) = vec.split_first().unwrap();
    /// assert_eq!(a, &0);
    /// assert_eq!(b, [1, 2, 3]);
    ///
    /// // empty
    /// let slice = vec.slice(0..0);
    /// assert!(slice.split_first().is_none());
    ///
    /// // single element
    /// let slice = vec.slice(2..3);
    /// let (a, b) = slice.split_first().unwrap();
    /// assert_eq!(a, &2);
    /// assert_eq!(b, []);
    /// ```
    pub fn split_first(&self) -> Option<(&ConcurrentElement<T>, ConcurrentSlice<T, P>)> {
        match self.get(0) {
            Some(a) => {
                let b = self.slice(1..self.len());
                Some((a, b))
            }
            None => None,
        }
    }

    /// Returns the last and all the rest of the elements of the slice, or None if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter(0..4);
    ///
    /// let (a, b) = vec.split_last().unwrap();
    /// assert_eq!(a, &3);
    /// assert_eq!(b, [0, 1, 2]);
    ///
    /// // empty
    /// let slice = vec.slice(0..0);
    /// assert!(slice.split_last().is_none());
    ///
    /// // single element
    /// let slice = vec.slice(2..3);
    /// let (a, b) = slice.split_last().unwrap();
    /// assert_eq!(a, &2);
    /// assert_eq!(b, []);
    /// ```
    pub fn split_last(&self) -> Option<(&ConcurrentElement<T>, ConcurrentSlice<T, P>)> {
        let len = self.len();
        match len {
            0 => None,
            _ => {
                let a = self.slice(0..(len - 1));
                let b = &self[len - 1];
                Some((b, a))
            }
        }
    }

    /// Returns an iterator over `chunk_size` elements of the slice at a time, starting at the beginning of the slice.
    ///
    /// The chunks are slices and do not overlap.
    /// If chunk_size does not divide the length of the slice, then the last chunk will not have length chunk_size.
    ///
    /// # Panics
    ///
    /// Panics if chunk_size is 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let vec: ConcurrentVec<_> = ['l', 'o', 'r', 'e', 'm'].into_iter().collect();
    ///
    /// let mut iter = vec.chunks(2);
    /// assert_eq!(iter.next().unwrap(), ['l', 'o']);
    /// assert_eq!(iter.next().unwrap(), ['r', 'e']);
    /// assert_eq!(iter.next().unwrap(), ['m']);
    /// assert!(iter.next().is_none());
    /// ```
    pub fn chunks(
        &self,
        chunk_size: usize,
    ) -> impl ExactSizeIterator<Item = ConcurrentSlice<T, P>> {
        assert!(chunk_size > 0);

        let len = self.len();
        let mut num_slices = len / chunk_size;
        let remainder = len - num_slices * chunk_size;
        if remainder > 0 {
            num_slices += 1;
        }

        (0..num_slices).map(move |i| {
            let a = i * chunk_size;
            let b = a + match (i == num_slices - 1, remainder > 0) {
                (true, true) => remainder,
                _ => chunk_size,
            };
            self.slice(a..b)
        })
    }
}
