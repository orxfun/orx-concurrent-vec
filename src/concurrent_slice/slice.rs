use crate::{elem::ConcurrentElem, helpers::DefaultPinVec, ConcurrentVec};
use core::ops::{Bound, RangeBounds};
use orx_fixed_vec::IntoConcurrentPinnedVec;

/// A slice of a [`ConcurrentVec`].
///
/// It can be created from a ConcurrentVec by [`ConcurrentVec::slice`]
/// or from another slice by [`ConcurrentSlice::slice`].
///
/// [`ConcurrentVec::slice`]: crate::ConcurrentVec::slice
/// [`ConcurrentSlice::slice`]: crate::ConcurrentSlice::slice
#[derive(Clone, Copy)]
pub struct ConcurrentSlice<'a, T, P = DefaultPinVec<T>>
where
    P: IntoConcurrentPinnedVec<ConcurrentElem<T>>,
{
    pub(super) vec: &'a ConcurrentVec<T, P>,
    pub(super) a: usize,
    pub(super) len: usize,
}

impl<'a, T, P> ConcurrentSlice<'a, T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElem<T>>,
{
    pub(crate) fn new(vec: &'a ConcurrentVec<T, P>, a: usize, len: usize) -> Self {
        Self { vec, a, len }
    }

    #[inline(always)]
    pub(super) fn idx(&self, i: usize) -> Option<usize> {
        match i < self.len {
            true => Some(self.a + i),
            false => None,
        }
    }

    // api

    /// Returns the length of the slice.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter([0, 1, 2, 3, 4]);
    ///
    /// assert_eq!(vec.slice(0..3).len(), 3);
    /// assert_eq!(vec.slice(1..=2).len(), 2);
    /// assert_eq!(vec.slice(5..).len(), 0);
    /// ```
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the slice is empty or not.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter([0, 1, 2, 3, 4]);
    ///
    /// assert_eq!(vec.slice(0..3).is_empty(), false);
    /// assert_eq!(vec.slice(1..=2).is_empty(), false);
    /// assert_eq!(vec.slice(5..).is_empty(), true);
    /// ```
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Creates and returns a slice of a `ConcurrentVec` or another `ConcurrentSlice`.
    ///
    /// Concurrent counterpart of a slice for a standard vec or an array.
    ///
    /// A `ConcurrentSlice` provides a focused / restricted view on a slice of the vector.
    /// It provides all methods of the concurrent vector except for the ones which
    /// grow the size of the vector.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter([0, 1, 2, 3, 4]);
    ///
    /// let slice = vec.slice(1..);
    /// assert_eq!(&slice, &[1, 2, 3, 4]);
    ///
    /// let slice = vec.slice(1..4);
    /// assert_eq!(&slice, &[1, 2, 3]);
    ///
    /// let slice = vec.slice(..3);
    /// assert_eq!(&slice, &[0, 1, 2]);
    ///
    /// let slice = vec.slice(3..10);
    /// assert_eq!(&slice, &[3, 4]);
    ///
    /// let slice = vec.slice(7..9);
    /// assert_eq!(&slice, &[]);
    ///
    /// // slices can also be sliced
    ///
    /// let slice = vec.slice(1..=4);
    /// assert_eq!(&slice, &[1, 2, 3, 4]);
    ///
    /// let sub_slice = slice.slice(1..3);
    /// assert_eq!(&sub_slice, &[2, 3]);
    /// ```
    pub fn slice<R: RangeBounds<usize>>(&self, range: R) -> ConcurrentSlice<T, P> {
        let (a, len) = begin_and_len(range, self.len());
        ConcurrentSlice::new(&self.vec, self.a + a, len)
    }

    /// Returns the element at the `i`-th position;
    /// returns None if the index is out of bounds.
    ///
    /// The safe api of the `ConcurrentVec` never gives out `&T` or `&mut T` references.
    /// Instead, returns a [`ConcurrentElem`] which provides thread safe concurrent read and write
    /// methods on the element.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    /// vec.extend([0, 1, 2, 3, 4, 5, 6]);
    ///
    /// let slice = vec.slice(1..5);
    /// assert_eq!(&slice, &[1, 2, 3, 4]);
    ///
    /// assert!(slice.get(4).is_none());
    ///
    /// let cloned = slice.get(2).map(|elem| elem.clone());
    /// assert_eq!(cloned, Some(3));
    ///
    /// let double = slice.get(2).map(|elem| elem.map(|x| x * 2));
    /// assert_eq!(double, Some(6));
    ///
    /// let elem = slice.get(2).unwrap();
    /// assert_eq!(elem, &3);
    ///
    /// elem.set(42);
    /// assert_eq!(elem, &42);
    ///
    /// elem.update(|x| *x = *x / 2);
    /// assert_eq!(elem, &21);
    ///
    /// let old = elem.replace(7);
    /// assert_eq!(old, 21);
    /// assert_eq!(elem, &7);
    ///
    /// assert_eq!(&slice, &[1, 2, 7, 4]);
    /// assert_eq!(&vec, &[0, 1, 2, 7, 4, 5, 6]);
    /// ```
    #[inline(always)]
    pub fn get(&self, i: usize) -> Option<&ConcurrentElem<T>> {
        self.idx(i).and_then(|i| self.vec.get(i))
    }

    /// Returns the cloned value of element at the `i`-th position;
    /// returns None if the index is out of bounds.
    ///
    /// Note that `slice.get_cloned(i)` is short-hand for `slice.get(i).map(|elem| elem.clone())`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    /// vec.extend([10, 0, 1, 2, 3, 14, 15]);
    ///
    /// let slice = vec.slice(1..5);
    ///
    /// assert_eq!(slice.get_cloned(2), Some(2));
    /// assert_eq!(slice.get_cloned(4), None);
    /// ```
    #[inline(always)]
    pub fn get_cloned(&self, i: usize) -> Option<T>
    where
        T: Clone,
    {
        self.idx(i).and_then(|i| self.vec.get_cloned(i))
    }

    /// Returns an iterator to the elements of the slice.
    ///
    /// The safe api of the `ConcurrentSlice` never gives out `&T` or `&mut T` references.
    /// Instead, the iterator yields [`ConcurrentElem`] which provides thread safe concurrent read and write
    /// methods on the element.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    /// vec.extend([10, 0, 1, 2, 3, 14, 15]);
    ///
    /// let slice = vec.slice(1..5);
    ///
    /// // read - map
    ///
    /// let doubles: Vec<_> = slice.iter().map(|elem| elem.map(|x| x * 2)).collect();
    /// assert_eq!(doubles, [0, 2, 4, 6]);
    ///
    /// // read - reduce
    ///
    /// let sum: i32 = slice.iter().map(|elem| elem.clone()).sum();
    /// assert_eq!(sum, 6);
    ///
    /// // mutate
    ///
    /// for (i, elem) in slice.iter().enumerate() {
    ///     match i {
    ///         2 => elem.set(42),
    ///         _ => elem.update(|x| *x *= 2),
    ///     }
    /// }
    /// assert_eq!(&slice, &[0, 2, 42, 6]);
    ///
    /// let old_vals: Vec<_> = slice.iter().map(|elem| elem.replace(7)).collect();
    /// assert_eq!(&old_vals, &[0, 2, 42, 6]);
    /// assert_eq!(&slice, &[7, 7, 7, 7]);
    ///
    /// assert_eq!(&vec, &[10, 7, 7, 7, 7, 14, 15]);
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = &ConcurrentElem<T>> {
        // TODO: this must be iter-from to jump directly to the 'a'-th element!
        unsafe { self.vec.core.iter(self.vec.len()) }
            .skip(self.a)
            .take(self.len)
    }

    /// Returns an iterator to cloned values of the elements of the slice.
    ///
    /// Note that `slice.iter_cloned()` is short-hand for `slice.iter().map(|elem| elem.clone())`.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    /// vec.extend([0, 42, 7, 3]);
    ///
    /// let slice = vec.slice(1..=2);
    ///
    /// let mut iter = slice.iter_cloned();
    ///
    /// assert_eq!(iter.next(), Some(42));
    /// assert_eq!(iter.next(), Some(7));
    /// assert_eq!(iter.next(), None);
    ///
    /// let sum: i32 = slice.iter_cloned().sum();
    /// assert_eq!(sum, 49);
    /// ```
    pub fn iter_cloned(&self) -> impl Iterator<Item = T> + '_
    where
        T: Clone,
    {
        // TODO: this must be iter-from to jump directly to the 'a'-th element!
        unsafe { self.vec.core.iter(self.vec.len()) }
            .skip(self.a)
            .take(self.len)
            .map(|elem| elem.clone())
    }
}

// helpers

pub(crate) fn begin_and_len<R: RangeBounds<usize>>(range: R, parent_len: usize) -> (usize, usize) {
    let a = match range.start_bound() {
        Bound::Included(i) => *i,
        Bound::Unbounded => 0,
        Bound::Excluded(i) => i + 1,
    };

    match a < parent_len {
        true => {
            let b = match range.end_bound() {
                Bound::Excluded(i) => *i,
                Bound::Included(i) => i + 1,
                Bound::Unbounded => parent_len,
            }
            .min(parent_len);

            let len = b - a;
            (a, len)
        }
        false => (0, 0),
    }
}
