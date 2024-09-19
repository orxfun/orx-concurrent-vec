use crate::concurrent_slice::begin_and_len;
use crate::elem::ConcurrentElem;
use crate::ConcurrentSlice;
use crate::{helpers::DefaultPinVec, state::ConcurrentVecState};
use core::ops::RangeBounds;
use core::sync::atomic::AtomicUsize;
use orx_pinned_concurrent_col::PinnedConcurrentCol;
use orx_pinned_vec::IntoConcurrentPinnedVec;

/// An efficient, convenient and lightweight grow-only read & write concurrent data structure allowing high performance concurrent collection.
///
/// * **convenient**: `ConcurrentVec` can safely be shared among threads simply as a shared reference. It is a [`PinnedConcurrentCol`](https://crates.io/crates/orx-pinned-concurrent-col) with a special concurrent state implementation. Underlying [`PinnedVec`](https://crates.io/crates/orx-pinned-vec) and concurrent bag can be converted back and forth to each other.
/// * **efficient**: `ConcurrentVec` is a lock free structure making use of a few atomic primitives, this leads to high performance concurrent growth. You may see the details in <a href="#section-benchmarks">benchmarks</a> and further <a href="#section-performance-notes">performance notes</a>.
///
/// Note that `ConcurrentVec` is a read & write collection with the cost to store values wrapped with an optional and initializing memory on allocation. See [`ConcurrentBag`](https://crates.io/crates/orx-concurrent-bag) for a write/grow only variant.
///
/// # Examples
///
/// Underlying `PinnedVec` guarantees make it straightforward to safely grow with a shared reference which leads to a convenient api as demonstrated below.
///
/// The following example demonstrates use of two collections together:
/// * A `ConcurrentVec` is used to collect measurements taken in random intervals.
///   * Concurrent vec is used since while collecting measurements, another thread will be reading them to compute statistics (read & write).
/// * A `ConcurrentBag` is used to collect statistics from the measurements at defined time intervals.
///   * Concurrent bag is used since we do not need to read the statistics until the process completes (write-only).
///
/// TODO:
/// ```rust ignore
/// use orx_concurrent_vec::*;
/// use orx_concurrent_bag::*;
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
///
///     fn average(&self) -> i32 {
///         match self.count {
///             0 => 0,
///             _ => self.sum / self.count,
///         }
///     }
/// }
///
/// // record measurements in random intervals, roughly every 2ms (read & write -> ConcurrentVec)
/// let measurements = ConcurrentVec::new();
/// let rf_measurements = &measurements; // just &self to share among threads
///
/// // collect metrics every 100 milliseconds (only write -> ConcurrentBag)
/// let metrics = ConcurrentBag::new();
/// let rf_metrics = &metrics; // just &self to share among threads
///
/// std::thread::scope(|s| {
///     // thread to store measurements as they arrive
///     s.spawn(move || {
///         for i in 0..100 {
///             std::thread::sleep(Duration::from_millis(i % 5));
///
///             // collect measurements and push to measurements vec
///             // simply by calling `push`
///             rf_measurements.push(i as i32);
///         }
///     });
///
///     // thread to collect metrics every 100 milliseconds
///     s.spawn(move || {
///         for _ in 0..10 {
///             // safely read from measurements vec to compute the metric
///             let metric = rf_measurements
///                 .iter()
///                 .fold(Metric::default(), |x, value| x.aggregate(value));
///
///             // push result to metrics bag
///             rf_metrics.push(metric);
///
///             std::thread::sleep(Duration::from_millis(100));
///         }
///     });
/// });
///
/// let measurements: Vec<_> = measurements
///     .into_inner()
///     .into_iter()
///     .map(|x| x.unwrap())
///     .collect();
/// dbg!(&measurements);
///
/// let averages: Vec<_> = metrics
///     .into_inner()
///     .into_iter()
///     .map(|x| x.average())
///     .collect();
/// println!("averages = {:?}", &averages);
///
/// assert_eq!(measurements.len(), 100);
/// assert_eq!(averages.len(), 10);
/// ```
///
/// ## Construction
///
/// `ConcurrentBag` can be constructed by wrapping any pinned vector; i.e., `ConcurrentBag<T>` implements `From<P: PinnedVec<T>>`.
/// Likewise, a concurrent vector can be unwrapped without any cost to the underlying pinned vector with `into_inner` method.
///
/// Further, there exist `with_` methods to directly construct the concurrent bag with common pinned vector implementations.
///
/// ```rust
/// use orx_concurrent_bag::*;
///
/// // default pinned vector -> SplitVec<T, Doubling>
/// let bag: ConcurrentBag<char> = ConcurrentBag::new();
/// let bag: ConcurrentBag<char> = Default::default();
/// let bag: ConcurrentBag<char> = ConcurrentBag::with_doubling_growth();
/// let bag: ConcurrentBag<char, SplitVec<char, Doubling>> = ConcurrentBag::with_doubling_growth();
///
/// let bag: ConcurrentBag<char> = SplitVec::new().into();
/// let bag: ConcurrentBag<char, SplitVec<char, Doubling>> = SplitVec::new().into();
///
/// // SplitVec with [Linear](https://docs.rs/orx-split-vec/latest/orx_split_vec/struct.Linear.html) growth
/// // each fragment will have capacity 2^10 = 1024
/// // and the split vector can grow up to 32 fragments
/// let bag: ConcurrentBag<char, SplitVec<char, Linear>> = ConcurrentBag::with_linear_growth(10, 32);
/// let bag: ConcurrentBag<char, SplitVec<char, Linear>> = SplitVec::with_linear_growth_and_fragments_capacity(10, 32).into();
///
/// // [FixedVec](https://docs.rs/orx-fixed-vec/latest/orx_fixed_vec/) with fixed capacity.
/// // Fixed vector cannot grow; hence, pushing the 1025-th element to this bag will cause a panic!
/// let bag: ConcurrentBag<char, FixedVec<char>> = ConcurrentBag::with_fixed_capacity(1024);
/// let bag: ConcurrentBag<char, FixedVec<char>> = FixedVec::new(1024).into();
/// ```
///
/// Of course, the pinned vector to be wrapped does not need to be empty.
///
/// ```rust
/// use orx_concurrent_bag::*;
///
/// let split_vec: SplitVec<i32> = (0..1024).collect();
/// let bag: ConcurrentBag<_> = split_vec.into();
/// ```
///
/// # Concurrent State and Properties
///
/// The concurrent state is modeled simply by an atomic length.
/// Combination of this state and `PinnedConcurrentCol` leads to the following properties:
/// * Writing to the collection does not block. Multiple writes can happen concurrently.
/// * Each position is written only and exactly once.
/// * Only one growth can happen at a given time.
/// * Underlying pinned vector can be extracted any time.
/// * Safe reading is only possible after converting the bag into the underlying `PinnedVec`.
/// No read & write race condition exists.
pub struct ConcurrentVec<T, P = DefaultPinVec<T>>
where
    P: IntoConcurrentPinnedVec<ConcurrentElem<T>>,
{
    pub(crate) core: PinnedConcurrentCol<ConcurrentElem<T>, P::ConPinnedVec, ConcurrentVecState>,
}

impl<T, P> ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElem<T>>,
{
    /// Consumes the concurrent bag and returns the underlying pinned vector.
    ///
    /// Any `PinnedVec` implementation can be converted to a `ConcurrentBag` using the `From` trait.
    /// Similarly, underlying pinned vector can be obtained by calling the consuming `into_inner` method.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_bag::*;
    ///
    /// let bag = ConcurrentBag::new();
    ///
    /// bag.push('a');
    /// bag.push('b');
    /// bag.push('c');
    /// bag.push('d');
    /// assert_eq!(vec!['a', 'b', 'c', 'd'], unsafe { bag.iter() }.copied().collect::<Vec<_>>());
    ///
    /// let mut split = bag.into_inner();
    /// assert_eq!(vec!['a', 'b', 'c', 'd'], split.iter().copied().collect::<Vec<_>>());
    ///
    /// split.push('e');
    /// *split.get_mut(0).expect("exists") = 'x';
    ///
    /// assert_eq!(vec!['x', 'b', 'c', 'd', 'e'], split.iter().copied().collect::<Vec<_>>());
    ///
    /// let mut bag: ConcurrentBag<_> = split.into();
    /// assert_eq!(vec!['x', 'b', 'c', 'd', 'e'], unsafe { bag.iter() }.copied().collect::<Vec<_>>());
    ///
    /// bag.clear();
    /// assert!(bag.is_empty());
    ///
    /// let split = bag.into_inner();
    /// assert!(split.is_empty());
    pub fn into_inner(self) -> P {
        let len = self.core.state().len();
        // # SAFETY: ConcurrentBag only allows to push to the end of the bag, keeping track of the length.
        // Therefore, the underlying pinned vector is in a valid condition at any given time.
        unsafe { self.core.into_inner(len) }
    }

    /// ***O(1)*** Returns the number of elements which are pushed to the bag, including the elements which received their reserved locations and are currently being pushed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_bag::ConcurrentBag;
    ///
    /// let bag = ConcurrentBag::new();
    /// bag.push('a');
    /// bag.push('b');
    ///
    /// assert_eq!(2, bag.len());
    /// ```
    #[inline(always)]
    pub fn len(&self) -> usize {
        let len = self.core.state().len();
        let cap = self.core.capacity();
        match len <= cap {
            true => len,
            false => cap,
        }
    }

    /// Returns whether or not the bag is empty.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_bag::ConcurrentBag;
    ///
    /// let mut bag = ConcurrentBag::new();
    ///
    /// assert!(bag.is_empty());
    ///
    /// bag.push('a');
    /// bag.push('b');
    ///
    /// assert!(!bag.is_empty());
    ///
    /// bag.clear();
    /// assert!(bag.is_empty());
    /// ```
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears the concurrent bag.
    pub fn clear(&mut self) {
        unsafe { self.core.clear(self.core.state().len()) };
    }

    /// Note that [`ConcurrentVec::maximum_capacity`] returns the maximum possible number of elements that the underlying pinned vector can grow to without reserving maximum capacity.
    ///
    /// In other words, the pinned vector can automatically grow up to the [`ConcurrentVec::maximum_capacity`] with `write` and `write_n_items` methods, using only a shared reference.
    ///
    /// When required, this maximum capacity can be attempted to increase by this method with a mutable reference.
    ///
    /// Importantly note that maximum capacity does not correspond to the allocated memory.
    ///
    /// Among the common pinned vector implementations:
    /// * `SplitVec<_, Doubling>`: supports this method; however, it does not require for any practical size.
    /// * `SplitVec<_, Linear>`: is guaranteed to succeed and increase its maximum capacity to the required value.
    /// * `FixedVec<_>`: is the most strict pinned vector which cannot grow even in a single-threaded setting. Currently, it will always return an error to this call.
    ///
    /// # Safety
    /// This method is unsafe since the concurrent pinned vector might contain gaps. The vector must be gap-free while increasing the maximum capacity.
    ///
    /// This method can safely be called if entries in all positions 0..len are written.
    pub fn reserve_maximum_capacity(&mut self, new_maximum_capacity: usize) -> usize {
        unsafe {
            self.core
                .reserve_maximum_capacity(self.core.state().len(), new_maximum_capacity)
        }
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
    pub fn slice<R: RangeBounds<usize>>(&self, range: R) -> ConcurrentSlice<T, P> {
        let (a, len) = begin_and_len(range, self.len());
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
    pub fn as_slice(&self) -> ConcurrentSlice<T, P> {
        self.slice(..)
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
    /// let vec = ConcurrentVec::from_iter([0, 1, 2, 3]);
    ///
    /// assert!(vec.get(4).is_none());
    ///
    /// let cloned = vec.get(2).map(|elem| elem.clone());
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
    pub fn get(&self, i: usize) -> Option<&ConcurrentElem<T>> {
        match i < self.len() {
            true => unsafe { self.core.get(i) },
            false => None,
        }
    }

    /// Returns the cloned value of element at the `i`-th position;
    /// returns None if the index is out of bounds.
    ///
    /// Note that `vec.get_cloned(i)` is short-hand for `vec.get(i).map(|elem| elem.clone())`.
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
        match i < self.len() {
            true => unsafe { self.core.get(i) }.map(|elem| elem.clone()),
            false => None,
        }
    }

    /// Returns an iterator to the elements of the vec.
    ///
    /// The safe api of the `ConcurrentVec` never gives out `&T` or `&mut T` references.
    /// Instead, the iterator yields [`ConcurrentElem`] which provides thread safe concurrent read and write
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
    /// let sum: i32 = vec.iter().map(|elem| elem.clone()).sum();
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
    pub fn iter(&self) -> impl Iterator<Item = &ConcurrentElem<T>> {
        unsafe { self.core.iter(self.len()) }
    }

    /// Returns an iterator to cloned values of the elements of the vec.
    ///
    /// Note that `vec.iter_cloned()` is short-hand for `vec.iter().map(|elem| elem.clone())`.
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
        unsafe { self.core.iter(self.len()) }.map(|elem| elem.clone())
    }
}

// HELPERS

impl<T, P> ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElem<T>>,
{
    pub(crate) fn new_from_pinned(pinned_vec: P) -> Self {
        let core = PinnedConcurrentCol::new_from_pinned(pinned_vec);
        Self { core }
    }
}

unsafe impl<T: Sync, P: IntoConcurrentPinnedVec<ConcurrentElem<T>>> Sync for ConcurrentVec<T, P> {}

unsafe impl<T: Send, P: IntoConcurrentPinnedVec<ConcurrentElem<T>>> Send for ConcurrentVec<T, P> {}
