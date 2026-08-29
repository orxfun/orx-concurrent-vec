use crate::ConcurrentSlice;
use crate::elem::ConcurrentElement;
use crate::{helpers::DefaultPinVec, state::ConcurrentVecState};
use core::ops::RangeBounds;
use core::sync::atomic::Ordering;
use orx_pinned_concurrent_col::PinnedConcurrentCol;
use orx_pinned_vec::IntoConcurrentPinnedVec;

/// A thread-safe, efficient and lock-free vector allowing concurrent grow, read and update operations.
///
/// ConcurrentVec provides safe api for the following three sets of concurrent operations, grow & read & update.
///
/// # Examples
///
/// ```rust
/// use orx_concurrent_vec::*;
/// use std::time::Duration;
///
/// #[derive(Debug, Default)]
/// struct Metric {
///     sum: i32,
///     count: i32,
/// }
/// impl Metric {
///     fn aggregate(self, value: &i32) -> Self {
///         Self {
///             sum: self.sum + value,
///             count: self.count + 1,
///         }
///     }
/// }
///
/// // record measurements in random intervals, roughly every 2ms
/// let measurements = ConcurrentVec::new();
///
/// // collect metrics every 100 milliseconds
/// let metrics = ConcurrentVec::new();
///
/// std::thread::scope(|s| {
///     // thread to store measurements as they arrive
///     s.spawn(|| {
///         for i in 0..100 {
///             std::thread::sleep(Duration::from_millis(i % 5));
///
///             // collect measurements and push to measurements vec
///             measurements.push(i as i32);
///         }
///     });
///
///     // thread to collect metrics every 100 milliseconds
///     s.spawn(|| {
///         for _ in 0..10 {
///             // safely read from measurements vec to compute the metric at that instant
///             let metric =
///                 measurements.fold(Metric::default(), |x, value| x.aggregate(value));
///
///             // push result to metrics
///             metrics.push(metric);
///
///             std::thread::sleep(Duration::from_millis(100));
///         }
///     });
/// });
///
/// let measurements: Vec<_> = measurements.to_vec();
/// let averages: Vec<_> = metrics.to_vec();
///
/// assert_eq!(measurements.len(), 100);
/// assert_eq!(averages.len(), 10);
/// ```
pub struct ConcurrentVec<T, P = DefaultPinVec<T>>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    pub(crate) core: PinnedConcurrentCol<ConcurrentElement<T>, P::ConPinnedVec, ConcurrentVecState>,
}

impl<T, P> ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    /// Consumes the concurrent vec and returns the underlying pinned vector.
    ///
    /// Any `PinnedVec` implementation can be converted to a `ConcurrentVec` using the `From` trait.
    /// Similarly, underlying pinned vector can be obtained by calling the consuming `into_inner` method.
    pub fn into_inner(self) -> P {
        let len = self.core.state().len();
        // # SAFETY: ConcurrentBag only allows to push to the end of the bag, keeping track of the length.
        // Therefore, the underlying pinned vector is in a valid condition at any given time.
        unsafe { self.core.into_inner(len) }
    }

    /// Returns the number of elements which are pushed to the vec,
    /// including the elements which received their reserved locations and are currently being pushed.
    #[inline(always)]
    pub(crate) fn reserved_len(&self) -> usize {
        let len = self.core.state().len();
        let cap = self.core.capacity();
        match len <= cap {
            true => len,
            false => cap,
        }
    }

    /// Returns the number of elements which are pushed to the vec,
    /// excluding the elements which received their reserved locations and are currently being pushed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::ConcurrentVec;
    ///
    /// let vec = ConcurrentVec::new();
    /// vec.push('a');
    /// vec.push('b');
    ///
    /// assert_eq!(2, vec.len());
    /// ```
    pub fn len(&self) -> usize {
        let len = self.len_written().load(Ordering::Acquire);
        let cap = self.capacity();
        let len_reserved = self.len_reserved().load(Ordering::Relaxed);
        let until = match len_reserved <= cap {
            true => len_reserved,
            false => cap,
        };

        let iter = unsafe { self.core.iter_over_range(len..until) };
        let mut num_pushed = 0;
        for x in iter {
            match x.0.is_some_with_order(Ordering::Relaxed) {
                true => num_pushed += 1,
                false => break,
            }
        }
        let new_len = len + num_pushed;

        self.len_written().fetch_max(new_len, Ordering::Release);

        new_len
    }

    /// Returns whether or not the bag is empty.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::ConcurrentVec;
    ///
    /// let mut vec = ConcurrentVec::new();
    ///
    /// assert!(vec.is_empty());
    ///
    /// vec.push('a');
    /// vec.push('b');
    ///
    /// assert!(!vec.is_empty());
    ///
    /// vec.clear();
    /// assert!(vec.is_empty());
    /// ```
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the current allocated capacity of the collection.
    pub fn capacity(&self) -> usize {
        self.core.capacity()
    }

    /// Returns maximum possible capacity that the collection can reach without calling [`ConcurrentVec::reserve_maximum_capacity`].
    ///
    /// Importantly note that maximum capacity does not correspond to the allocated memory.
    pub fn maximum_capacity(&self) -> usize {
        self.core.maximum_capacity()
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
    pub fn slice<R: RangeBounds<usize>>(&self, range: R) -> ConcurrentSlice<'_, T, P> {
        let [a, b] = orx_pinned_vec::utils::slice::vec_range_limits(&range, Some(self.len()));
        let len = b - a;
        ConcurrentSlice::new(self, a, len)
    }

    /// Creates and returns a slice of all elements of the vec.
    ///
    /// Note that `vec.as_slice()` is equivalent to `vec.slice(..)`.
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
    /// let slice = vec.as_slice();
    /// assert_eq!(&slice, &[0, 1, 2, 3, 4]);
    /// ```
    pub fn as_slice(&self) -> ConcurrentSlice<'_, T, P> {
        self.slice(0..self.len())
    }

    /// Returns the element at the `i`-th position;
    /// returns None if the index is out of bounds.
    ///
    /// The safe api of the `ConcurrentVec` never gives out `&T` or `&mut T` references.
    /// Instead, returns a [`ConcurrentElement`] which provides thread safe concurrent read and write
    /// methods on the element.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter([0, 1, 2, 3]);
    ///
    /// assert!(vec.get(4).is_none());
    ///
    /// let cloned = vec.get(2).map(|elem| elem.cloned());
    /// assert_eq!(cloned, Some(2));
    ///
    /// let double = vec.get(2).map(|elem| elem.map(|x| x * 2));
    /// assert_eq!(double, Some(4));
    ///
    /// let elem = vec.get(2).unwrap();
    /// assert_eq!(elem, &2);
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
    /// assert_eq!(&vec, &[0, 1, 7, 3]);
    /// ```
    #[inline(always)]
    pub fn get(&self, i: usize) -> Option<&ConcurrentElement<T>> {
        match i < self.len() {
            true => unsafe { self.core.get(i) },
            false => None,
        }
    }

    /// Returns the cloned value of element at the `i`-th position;
    /// returns None if the index is out of bounds.
    ///
    /// Note that `vec.get_cloned(i)` is short-hand for `vec.get(i).map(|elem| elem.cloned())`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter([0, 1, 2, 3]);
    ///
    /// assert_eq!(vec.get_cloned(2), Some(2));
    /// assert_eq!(vec.get_cloned(4), None);
    /// ```
    #[inline(always)]
    pub fn get_cloned(&self, i: usize) -> Option<T>
    where
        T: Clone,
    {
        match i < self.reserved_len() {
            true => unsafe { self.core.get(i) }.and_then(|elem| elem.0.clone_into_option()),
            false => None,
        }
    }

    /// Returns the copied value of element at the `i`-th position;
    /// returns None if the index is out of bounds.
    ///
    /// Note that `vec.get_copied(i)` is short-hand for `vec.get(i).map(|elem| elem.copied())`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter([0, 1, 2, 3]);
    ///
    /// assert_eq!(vec.get_copied(2), Some(2));
    /// assert_eq!(vec.get_copied(4), None);
    /// ```
    #[inline(always)]
    pub fn get_copied(&self, i: usize) -> Option<T>
    where
        T: Copy,
    {
        self.get_cloned(i)
    }

    /// Returns an iterator to the elements of the vec.
    ///
    /// The safe api of the `ConcurrentVec` never gives out `&T` or `&mut T` references.
    /// Instead, the iterator yields [`ConcurrentElement`] which provides thread safe concurrent read and write
    /// methods on the element.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter([0, 1, 2, 3]);
    ///
    /// // read - map
    ///
    /// let doubles: Vec<_> = vec.iter().map(|elem| elem.map(|x| x * 2)).collect();
    /// assert_eq!(doubles, [0, 2, 4, 6]);
    ///
    /// // read - reduce
    ///
    /// let sum: i32 = vec.iter().map(|elem| elem.cloned()).sum();
    /// assert_eq!(sum, 6);
    ///
    /// // mutate
    ///
    /// for (i, elem) in vec.iter().enumerate() {
    ///     match i {
    ///         2 => elem.set(42),
    ///         _ => elem.update(|x| *x *= 2),
    ///     }
    /// }
    /// assert_eq!(&vec, &[0, 2, 42, 6]);
    ///
    /// let old_vals: Vec<_> = vec.iter().map(|elem| elem.replace(7)).collect();
    /// assert_eq!(&old_vals, &[0, 2, 42, 6]);
    /// assert_eq!(&vec, &[7, 7, 7, 7]);
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = &ConcurrentElement<T>> {
        unsafe { self.core.iter(self.len()) }
    }

    /// Returns an iterator to cloned values of the elements of the vec.
    ///
    /// Note that `vec.iter_cloned()` is short-hand for `vec.iter().map(|elem| elem.cloned())`.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    /// vec.extend([42, 7]);
    ///
    /// let mut iter = vec.iter_cloned();
    ///
    /// assert_eq!(iter.next(), Some(42));
    /// assert_eq!(iter.next(), Some(7));
    /// assert_eq!(iter.next(), None);
    ///
    /// let sum: i32 = vec.iter_cloned().sum();
    /// assert_eq!(sum, 49);
    /// ```
    pub fn iter_cloned(&self) -> impl Iterator<Item = T> + '_
    where
        T: Clone,
    {
        unsafe { self.core.iter(self.len()) }.map(|elem| elem.cloned())
    }

    /// Binary searches this vec for a given element using a comparator function.
    ///
    /// This method assumes that the vec is sorted in ascending order according to the
    /// comparator function. If the vec is not sorted, the result is unspecified.
    ///
    /// Note: This method snapshots the vec length at the start of the search to handle
    /// concurrent growth safely. If the vector grows during the search, the search will only
    /// consider elements that existed at the start.
    ///
    /// If the value is found, returns `Ok(index)` where `index` is the position of the element
    /// in the vec. If the value is not found, returns `Err(insertion_point)` where
    /// `insertion_point` is the index where the element should be inserted to maintain order.
    ///
    /// This method delegates to `ConcurrentSlice::binary_search_by`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    /// use std::cmp::Ordering;
    ///
    /// let vec = ConcurrentVec::from_iter([1, 3, 5, 7, 9]);
    ///
    /// // Found: element exists at index 2
    /// assert_eq!(vec.binary_search_by(|x| x.map(|val| {
    ///     if *val == 5 { Ordering::Equal } else if *val < 5 { Ordering::Less } else { Ordering::Greater }
    /// })), Ok(2));
    ///
    /// // Not found: element should be inserted at index 3
    /// assert_eq!(vec.binary_search_by(|x| x.map(|val| {
    ///     if *val == 6 { Ordering::Equal } else if *val < 6 { Ordering::Less } else { Ordering::Greater }
    /// })), Err(3));
    /// ```
    pub fn binary_search_by<F>(&self, f: F) -> Result<usize, usize>
    where
        F: FnMut(&ConcurrentElement<T>) -> core::cmp::Ordering,
    {
        self.as_slice().binary_search_by(f)
    }
}

// Binary search convenience methods for Ord types
impl<T, P> ConcurrentVec<T, P>
where
    T: Ord,
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    /// Binary searches this vec for a given element.
    ///
    /// This method assumes that the vec is sorted in ascending order.
    /// If the vec is not sorted, the result is unspecified.
    ///
    /// If the value is found, returns `Ok(index)` where `index` is the position of the element
    /// in the vec. If the value is not found, returns `Err(insertion_point)` where
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
    ///
    /// // Found: element exists at index 2
    /// assert_eq!(vec.binary_search(&5), Ok(2));
    ///
    /// // Not found: element should be inserted at index 3
    /// assert_eq!(vec.binary_search(&6), Err(3));
    /// ```
    pub fn binary_search(&self, x: &T) -> Result<usize, usize> {
        self.as_slice().binary_search(x)
    }
}

// Binary search by key for types with extracted keys
impl<T, P> ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    /// Binary searches this vec for a given key using a key extraction function.
    ///
    /// This method assumes that the vec is sorted by the extracted keys in ascending order.
    /// If the vec is not sorted, the result is unspecified.
    ///
    /// If a value with the matching key is found, returns `Ok(index)` where `index` is the
    /// position of the element in the vec. If no matching value is found, returns `Err(insertion_point)`
    /// where `insertion_point` is the index where an element with the matching key should be
    /// inserted to maintain order.
    ///
    /// This is a convenience method that delegates to `binary_search_by_key` on a slice.
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
    ///
    /// // Found: element with key 3 exists at index 1
    /// assert_eq!(vec.binary_search_by_key(&3, |elem| {
    ///     elem.map(|pair| pair.key)
    /// }), Ok(1));
    ///
    /// // Not found: element with key 4 should be inserted at index 2
    /// assert_eq!(vec.binary_search_by_key(&4, |elem| {
    ///     elem.map(|pair| pair.key)
    /// }), Err(2));
    /// ```
    pub fn binary_search_by_key<K, F>(&self, b: &K, f: F) -> Result<usize, usize>
    where
        K: Ord,
        F: FnMut(&ConcurrentElement<T>) -> K,
    {
        self.as_slice().binary_search_by_key(b, f)
    }
}

// HELPERS

impl<T, P> ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    pub(crate) fn new_from_pinned(pinned_vec: P) -> Self {
        Self {
            core: PinnedConcurrentCol::new_from_pinned(pinned_vec),
        }
    }
}

unsafe impl<T: Sync, P: IntoConcurrentPinnedVec<ConcurrentElement<T>>> Sync
    for ConcurrentVec<T, P>
{
}

unsafe impl<T: Send, P: IntoConcurrentPinnedVec<ConcurrentElement<T>>> Send
    for ConcurrentVec<T, P>
{
}
