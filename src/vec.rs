use orx_concurrent_bag::ConcurrentBag;
use orx_fixed_vec::{FixedVec, PinnedVec, PinnedVecGrowthError};
use orx_split_vec::{Doubling, Linear, SplitVec};

/// An efficient, convenient and lightweight grow-only read-and-write concurrent collection.
///
/// * **convenient**: `ConcurrentVec` can safely be shared among threads simply as a shared reference. Further, it is a wrapper around any [`PinnedVec`](https://crates.io/crates/orx-pinned-vec) implementation adding concurrent safety guarantees. Underlying pinned vector of `Option<T>` and concurrent vec of `T` can be converted to each other back and forth without any cost.
/// * **lightweight**: This crate takes a simplistic approach built on pinned vector guarantees which leads to concurrent programs with few dependencies and small binaries.
/// * **efficient**: `ConcurrentVec` is a lock free structure making use of a few atomic primitives, this leads to high performance growth.
///
/// Note that `ConcurrentVec` is closely related to [`ConcurrentBag`](https://crates.io/crates/orx-concurrent-bag) with the following differences:
/// * `ConcurrentBag` is a write-only and grow-only concurrent collection which internally stores elements directly as `T`,
/// * `ConcurrentVec` is a read-and-write and grow-only concurrent collection which internally stores elements directly as `Option<T>`.
///
/// # Examples
///
/// Safety guarantees to push to the concurrent vec with a shared reference makes it easy to share the concurrent vec among threads. `std::sync::Arc` can be used; however, it is not required as demonstrated below.
///
/// ```rust
/// use orx_concurrent_vec::*;
/// use orx_concurrent_bag::*;
/// use std::time::Duration;
///
/// #[derive(Default, Debug)]
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
/// // record measurements in random intervals (read & write -> ConcurrentVec)
/// let measurements = ConcurrentVec::new();
/// let rf_measurements = &measurements; // just take a reference and share among threads
///
/// // collect metrics every 50 milliseconds (only write -> ConcurrentBag)
/// let metrics = ConcurrentBag::new();
/// let rf_metrics = &metrics; // just take a reference and share among threads
///
/// std::thread::scope(|s| {
///     // thread to store measurements
///     s.spawn(move || {
///         for i in 0..100 {
///             std::thread::sleep(Duration::from_millis(i % 5));
///
///             // concurrently collect measurements simply by calling `push`
///             rf_measurements.push(i as i32);
///         }
///     });
///
///     // thread to collect metrics every 50 milliseconds
///     s.spawn(move || {
///         for _ in 0..10 {
///             // concurrently read from measurements vec
///             let metric = rf_measurements
///                 .iter()
///                 .fold(Metric::default(), |x, value| x.aggregate(value));
///
///             // push results to metrics bag
///             // we may also use `ConcurrentVec` but we prefer `ConcurrentBag`
///             // since we don't concurrently read metrics
///             rf_metrics.push(metric);
///
///             std::thread::sleep(Duration::from_millis(50));
///         }
///     });
///
///     // thread to print out the values to the stdout every 100 milliseconds
///     s.spawn(move || {
///         let mut idx = 0;
///         loop {
///             let current_len = rf_measurements.len_exact();
///             let begin = idx;
///
///             for i in begin..current_len {
///                 // concurrently read from measurements vec
///                 if let Some(value) = rf_measurements.get(i) {
///                     println!("[{}] = {:?}", i, value);
///                     idx += 1;
///                 } else {
///                     idx = i;
///                     break;
///                 }
///             }
///
///             if current_len == 100 {
///                 break;
///             }
///
///             std::thread::sleep(Duration::from_millis(100));
///         }
///     });
/// });
///
/// assert_eq!(measurements.len(), 100);
/// assert_eq!(metrics.len(), 10);
/// ```
#[derive(Debug)]
pub struct ConcurrentVec<T, P = SplitVec<Option<T>, Doubling>>
where
    P: PinnedVec<Option<T>>,
{
    bag: ConcurrentBag<Option<T>, P>,
}

// new
impl<T> ConcurrentVec<T, SplitVec<Option<T>, Doubling>> {
    /// Creates a new concurrent vector by creating and wrapping up a new `SplitVec<Option<T>, Doubling>` as the underlying storage.
    pub fn with_doubling_growth() -> Self {
        let pinned = SplitVec::with_doubling_growth_and_fragments_capacity(32);
        Self::from_pinned_vec(pinned)
    }

    /// Creates a new concurrent vector by creating and wrapping up a new `SplitVec<Option<T>, Doubling>` as the underlying storage.
    pub fn new() -> Self {
        Self::with_doubling_growth()
    }
}

impl<T> Default for ConcurrentVec<T, SplitVec<Option<T>, Doubling>> {
    /// Creates a new concurrent vector by creating and wrapping up a new `SplitVec<Option<T>, Doubling>` as the underlying storage.
    fn default() -> Self {
        Self::with_doubling_growth()
    }
}

impl<T> ConcurrentVec<T, SplitVec<Option<T>, Linear>> {
    /// Creates a new concurrent vector by creating and wrapping up a new `SplitVec<Option<T>, Linear>` as the underlying storage.
    ///
    /// # Notes
    ///
    /// * `Linear` can be chosen over `Doubling` whenever memory efficiency is more critical since linear allocations lead to less waste.
    /// * Choosing a small `constant_fragment_capacity_exponent` for a large bag to be filled might lead to too many growth calls.
    /// * Furthermore, `Linear` growth strategy leads to a hard upper bound on the maximum capacity, please see the Safety section.
    ///
    /// # Safety
    ///
    /// `SplitVec<T, Linear>` can grow indefinitely (almost).
    ///
    /// However, `ConcurrentVec<T, SplitVec<T, Linear>>` has an upper bound on its capacity.
    /// This capacity is computed as:
    ///
    /// ```rust ignore
    /// 2usize.pow(constant_fragment_capacity_exponent) * fragments_capacity
    /// ```
    ///
    /// For instance maximum capacity is 2^10 * 64 = 65536 when `constant_fragment_capacity_exponent=10` and `fragments_capacity=64`.
    ///
    /// Note that setting a relatively high and safe `fragments_capacity` is **not** costly, size of each element 2*usize.
    ///
    /// Pushing to the vector beyond this capacity leads to "out-of-capacity" error.
    ///
    /// This maximum capacity can be accessed by [`ConcurrentVec::maximum_capacity`] method.
    pub fn with_linear_growth(
        constant_fragment_capacity_exponent: usize,
        fragments_capacity: usize,
    ) -> Self {
        let pinned = SplitVec::with_linear_growth_and_fragments_capacity(
            constant_fragment_capacity_exponent,
            fragments_capacity,
        );
        Self::from_pinned_vec(pinned)
    }
}

impl<T> ConcurrentVec<T, FixedVec<Option<T>>> {
    /// Creates a new concurrent vector by creating and wrapping up a new `FixedVec<Option<T>>` as the underlying storage.
    ///
    /// # Safety
    ///
    /// Note that a `FixedVec` cannot grow; i.e., it has a hard upper bound on the number of elements it can hold, which is the `fixed_capacity`.
    ///
    /// Pushing to the vector beyond this capacity leads to "out-of-capacity" error.
    ///
    /// This maximum capacity can be accessed by [`ConcurrentVec::maximum_capacity`] method.
    pub fn with_fixed_capacity(fixed_capacity: usize) -> Self {
        let pinned = FixedVec::new(fixed_capacity);
        Self::from_pinned_vec(pinned)
    }
}

impl<T, P> From<P> for ConcurrentVec<T, P>
where
    P: PinnedVec<Option<T>>,
{
    /// `ConcurrentVec<T>` is just a wrapper around `ConcurrentBag<Option<T>>` with additional guarantees to enable safe concurrent reading.
    ///
    /// Further, `ConcurrentBag<Option<T>>` wraps any `PinnedVec<Option<T>>` implementation.
    ///
    /// Therefore, without a cost
    /// * `ConcurrentVec<T>` can be constructed from any `PinnedVec<Option<T>>`, and
    /// * the underlying `PinnedVec<Option<T>>` can be obtained by `ConcurrentVec::into_inner(self)` method.
    fn from(pinned: P) -> Self {
        Self::from_pinned_vec(pinned)
    }
}

impl<T, P> From<ConcurrentBag<Option<T>, P>> for ConcurrentVec<T, P>
where
    P: PinnedVec<Option<T>>,
{
    /// `ConcurrentVec<T>` is just a wrapper around `ConcurrentBag<Option<T>>` with additional guarantees to enable safe concurrent reading.
    ///
    /// Therefore, without a cost
    /// * `ConcurrentVec<T>` can be constructed from any `ConcurrentBag<Option<T>>` by calling `bag.into()`, and
    /// * the underlying `ConcurrentBag<Option<T>>` can be obtained from `ConcurrentVec<T>` by calling `con_vec.into()` method.
    fn from(bag: ConcurrentBag<Option<T>, P>) -> Self {
        let pinned = bag.into_inner();
        Self::from_pinned_vec(pinned)
    }
}

impl<T, P> From<ConcurrentVec<T, P>> for ConcurrentBag<Option<T>, P>
where
    P: PinnedVec<Option<T>>,
{
    /// `ConcurrentVec<T>` is just a wrapper around `ConcurrentBag<Option<T>>` with additional guarantees to enable safe concurrent reading.
    ///
    /// Therefore, without a cost
    /// * `ConcurrentVec<T>` can be constructed from any `ConcurrentBag<Option<T>>` by calling `bag.into()`, and
    /// * the underlying `ConcurrentBag<Option<T>>` can be obtained from `ConcurrentVec<T>` by calling `con_vec.into()` method.
    fn from(con_vec: ConcurrentVec<T, P>) -> Self {
        let pinned = con_vec.bag.into_inner();
        ConcurrentBag::from(pinned)
    }
}

// impl
impl<T, P> ConcurrentVec<T, P>
where
    P: PinnedVec<Option<T>>,
{
    /// Consumes the concurrent bag and returns the underlying pinned vector.
    ///
    /// Any `PinnedVec<Option<T>>` implementation can be converted to a `ConcurrentVec<T>` using the `From` trait.
    /// Similarly, underlying pinned vector can be obtained by calling the consuming `into_inner` method.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let con_vec = ConcurrentVec::new();
    ///
    /// con_vec.push('a');
    /// con_vec.push('b');
    /// con_vec.push('c');
    /// con_vec.push('d');
    /// assert_eq!(vec!['a', 'b', 'c', 'd'], con_vec.iter().copied().collect::<Vec<_>>());
    ///
    /// let mut split = con_vec.into_inner();
    /// assert_eq!(vec!['a', 'b', 'c', 'd'], split.iter().copied().flatten().collect::<Vec<_>>());
    ///
    /// split.push(Some('e'));
    /// *split.get_mut(0).expect("exists") = Some('x');
    ///
    /// assert_eq!(vec!['x', 'b', 'c', 'd', 'e'], split.iter().copied().flatten().collect::<Vec<_>>());
    ///
    /// let mut con_vec: ConcurrentVec<_> = split.into();
    /// assert_eq!(vec!['x', 'b', 'c', 'd', 'e'], con_vec.iter().copied().collect::<Vec<_>>());
    ///
    /// con_vec.clear();
    /// assert!(con_vec.is_empty());
    ///
    /// let split = con_vec.into_inner();
    /// assert!(split.is_empty());
    pub fn into_inner(self) -> P {
        self.bag.into_inner()
    }

    /// ***O(1)*** Returns the number of elements which are pushed to the vec, including the elements which received their reserved locations and are currently being pushed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::ConcurrentVec;
    ///
    /// let con_vec = ConcurrentVec::new();
    /// con_vec.push('a');
    /// con_vec.push('b');
    ///
    /// assert_eq!(2, con_vec.len());
    /// ```
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.bag.len()
    }

    /// ***O(n)*** Returns the number of elements which are completely pushed to the vector, excluding elements which received their reserved locations and currently being pushed.
    ///
    /// Note that `con_vec.len_exact()` is basically equivalent to `con_vec.iter().count()`.
    ///
    /// In order to get number of elements for which the `push` method is called, including elements that are currently being pushed, you may use
    /// `convec.len()` with ***O(1)*** time complexity.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::ConcurrentVec;
    ///
    /// let con_vec = ConcurrentVec::new();
    /// con_vec.push('a');
    /// con_vec.push('b');
    ///
    /// assert_eq!(2, con_vec.len_exact());
    /// assert_eq!(2, con_vec.iter().count());
    /// ```
    #[inline(always)]
    pub fn len_exact(&self) -> usize {
        self.iter().count()
    }

    /// ***O(1)*** Returns the current capacity of the concurrent vec; i.e., the underlying pinned vector storage.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::ConcurrentVec;
    ///
    /// let con_vec = ConcurrentVec::new();
    /// con_vec.push('a');
    /// con_vec.push('b');
    ///
    /// assert_eq!(4, con_vec.capacity());
    ///
    /// con_vec.push('c');
    /// con_vec.push('d');
    /// con_vec.push('e');
    ///
    /// assert_eq!(12, con_vec.capacity());
    /// ```
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.bag.capacity()
    }

    /// Note that a `ConcurrentVec` contains two capacity methods:
    /// * `capacity`: returns current capacity of the underlying pinned vector; this capacity can grow concurrently with a `&self` reference; i.e., during a `push` call
    /// * `maximum_capacity`: returns the maximum potential capacity that the pinned vector can grow up to with a `&self` reference;
    ///   * `push`ing a new element while the bag is at `maximum_capacity` leads to panic.
    ///
    /// On the other hand, maximum capacity can safely be increased by a mutually exclusive `&mut self` reference using the `reserve_maximum_capacity`.
    /// However, underlying pinned vector must be able to provide pinned-element guarantees during this operation.
    ///
    /// Among the common pinned vector implementations:
    /// * `SplitVec<_, Doubling>`: supports this method; however, it does not require for any practical size.
    /// * `SplitVec<_, Linear>`: is guaranteed to succeed and increase its maximum capacity to the required value.
    /// * `FixedVec<_>`: is the most strict pinned vector which cannot grow even in a single-threaded setting. Currently, it will always return an error to this call.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    /// use orx_pinned_vec::PinnedVecGrowthError;
    ///
    ///  // SplitVec<_, Doubling> (default)
    /// let con_vec: ConcurrentVec<char> = ConcurrentVec::new();
    /// assert_eq!(con_vec.capacity(), 4); // only allocates the first fragment of 4
    /// assert_eq!(con_vec.maximum_capacity(), 17_179_869_180); // it can grow safely & exponentially
    ///
    /// let con_vec: ConcurrentVec<char, _> = ConcurrentVec::with_doubling_growth();
    /// assert_eq!(con_vec.capacity(), 4);
    /// assert_eq!(con_vec.maximum_capacity(), 17_179_869_180);
    ///
    /// // SplitVec<_, Linear>
    /// let mut con_vec: ConcurrentVec<char, _> = ConcurrentVec::with_linear_growth(10, 20);
    /// assert_eq!(con_vec.capacity(), 2usize.pow(10)); // only allocates first fragment of 1024
    /// assert_eq!(con_vec.maximum_capacity(), 2usize.pow(10) * 20); // it can concurrently allocate 19 more
    ///
    /// // SplitVec<_, Linear> -> reserve_maximum_capacity
    /// let result = con_vec.reserve_maximum_capacity(2usize.pow(10) * 30);
    /// assert_eq!(result, Ok(2usize.pow(10) * 30));
    ///
    /// // actually no new allocation yet; precisely additional memory for 10 pairs of pointers is used
    /// assert_eq!(con_vec.capacity(), 2usize.pow(10)); // still only the first fragment capacity
    ///
    /// dbg!(con_vec.maximum_capacity(), 2usize.pow(10) * 30);
    /// assert_eq!(con_vec.maximum_capacity(), 2usize.pow(10) * 30); // now it can safely reach 2^10 * 30
    ///
    /// // FixedVec<_>: pre-allocated, exact and strict
    /// let mut con_vec: ConcurrentVec<char, _> = ConcurrentVec::with_fixed_capacity(42);
    /// assert_eq!(con_vec.capacity(), 42);
    /// assert_eq!(con_vec.maximum_capacity(), 42);
    ///
    /// let result = con_vec.reserve_maximum_capacity(43);
    /// assert_eq!(
    ///     result,
    ///     Err(PinnedVecGrowthError::FailedToGrowWhileKeepingElementsPinned)
    /// );
    /// ```
    pub fn reserve_maximum_capacity(
        &mut self,
        maximum_capacity: usize,
    ) -> Result<usize, PinnedVecGrowthError> {
        self.bag.reserve_maximum_capacity(maximum_capacity)
    }

    /// Returns the maximum possible capacity that the concurrent vector can reach.
    /// This is equivalent to the maximum capacity that the underlying pinned vector can safely reach in a concurrent program.
    ///
    /// Note that `maximum_capacity` differs from `capacity` due to the following:
    /// * `capacity` represents currently allocated memory owned by the underlying pinned vector.
    /// The vector can possibly grow beyond this value.
    /// * `maximum_capacity` represents the hard upper limit on the length of the concurrent vector.
    /// Attempting to grow the vector beyond this value leads to an error.
    /// Note that this is not necessarily a limitation for the underlying split vector; the limitation might be due to the additional safety requirements of concurrent programs.
    ///
    /// # Safety
    ///
    /// Calling `push` while the vector is at its `maximum_capacity` leads to a panic.    
    /// This condition is a safety requirement.
    /// Underlying pinned vector cannot safely grow beyond maximum capacity in a possibly concurrent call (with `&self`).
    /// This would lead to UB.
    ///
    /// It is easy to overcome this problem during construction:
    /// * It is not observed with the default pinned vector `SplitVec<_, Doubling>`:
    ///   * `ConcurrentBag::new()`
    ///   * `ConcurrentBag::with_doubling_growth()`
    /// * It can be avoided cheaply when `SplitVec<_, Linear>` is used by setting the second argument to a proper value:
    ///   * `ConcurrentBag::with_linear_growth(10, 32)`
    ///     * Each fragment of this vector will have a capacity of 2^10 = 1024.
    ///     * It can safely grow up to 32 fragments with `&self` reference.
    ///     * Therefore, it is safe to concurrently grow this vector to 32 * 1024 elements.
    ///     * Cost of replacing 32 with 64 is negligible in such a scenario (the difference in memory allocation is precisely 32 * 2*usize).
    /// * Using the variant `FixedVec<_>` requires a hard bound and pre-allocation regardless the program is concurrent or not.
    ///   * `ConcurrentBag::with_fixed_capacity(1024)`
    ///     * Unlike the split vector variants, this variant pre-allocates for 1024 elements.
    ///     * And can never grow beyond this value.
    ///
    /// Alternatively, caller can try to safely reserve the required capacity any time by a mutually exclusive reference:
    /// * `ConcurrentBag.reserve_maximum_capacity(&mut self, maximum_capacity: usize)`
    ///   * this call will always succeed with `SplitVec<_, Doubling>` and `SplitVec<_, Linear>`,
    ///   * will always fail for `FixedVec<_>`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    /// use orx_pinned_vec::PinnedVecGrowthError;
    ///
    ///  // SplitVec<_, Doubling> (default)
    /// let con_vec: ConcurrentVec<char> = ConcurrentVec::new();
    /// assert_eq!(con_vec.capacity(), 4); // only allocates the first fragment of 4
    /// assert_eq!(con_vec.maximum_capacity(), 17_179_869_180); // it can grow safely & exponentially
    ///
    /// let con_vec: ConcurrentVec<char, _> = ConcurrentVec::with_doubling_growth();
    /// assert_eq!(con_vec.capacity(), 4);
    /// assert_eq!(con_vec.maximum_capacity(), 17_179_869_180);
    ///
    /// // SplitVec<_, Linear>
    /// let mut con_vec: ConcurrentVec<char, _> = ConcurrentVec::with_linear_growth(10, 20);
    /// assert_eq!(con_vec.capacity(), 2usize.pow(10)); // only allocates first fragment of 1024
    /// assert_eq!(con_vec.maximum_capacity(), 2usize.pow(10) * 20); // it can concurrently allocate 19 more
    ///
    /// // SplitVec<_, Linear> -> reserve_maximum_capacity
    /// let result = con_vec.reserve_maximum_capacity(2usize.pow(10) * 30);
    /// assert_eq!(result, Ok(2usize.pow(10) * 30));
    ///
    /// // actually no new allocation yet; precisely additional memory for 10 pairs of pointers is used
    /// assert_eq!(con_vec.capacity(), 2usize.pow(10)); // still only the first fragment capacity
    ///
    /// dbg!(con_vec.maximum_capacity(), 2usize.pow(10) * 30);
    /// assert_eq!(con_vec.maximum_capacity(), 2usize.pow(10) * 30); // now it can safely reach 2^10 * 30
    ///
    /// // FixedVec<_>: pre-allocated, exact and strict
    /// let mut con_vec: ConcurrentVec<char, _> = ConcurrentVec::with_fixed_capacity(42);
    /// assert_eq!(con_vec.capacity(), 42);
    /// assert_eq!(con_vec.maximum_capacity(), 42);
    ///
    /// let result = con_vec.reserve_maximum_capacity(43);
    /// assert_eq!(
    ///     result,
    ///     Err(PinnedVecGrowthError::FailedToGrowWhileKeepingElementsPinned)
    /// );
    /// ```
    #[inline]
    pub fn maximum_capacity(&self) -> usize {
        self.bag.maximum_capacity()
    }

    /// ***O(1)*** Returns whether the con_vec is empty (`len() == 0`) or not.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::ConcurrentVec;
    ///
    /// let mut con_vec = ConcurrentVec::new();
    ///
    /// assert!(con_vec.is_empty());
    ///
    /// con_vec.push('a');
    /// con_vec.push('b');
    /// assert!(!con_vec.is_empty());
    ///
    /// con_vec.clear();
    /// assert!(con_vec.is_empty());
    /// ```
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.bag.is_empty()
    }

    /// Returns an iterator to elements of the vector.
    ///
    /// Iteration of elements is in the order the push method is called.
    ///
    /// Note that the iterator skips elements which are currently being written; safely yields only the elements which are completely written.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::ConcurrentVec;
    ///
    /// let convec = ConcurrentVec::new();
    /// convec.push('a');
    /// convec.push('b');
    ///
    /// let mut iter = convec.iter();
    /// assert_eq!(iter.next(), Some(&'a'));
    /// assert_eq!(iter.next(), Some(&'b'));
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline(always)]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let bag_iter = unsafe { self.bag.iter() };
        bag_iter.take(self.len()).flatten()
    }

    /// Returns the element at the `index`-th position of the concurrent vector.
    ///
    /// Returns `None` if:
    /// * the `index` is out of bounds,
    /// * or the element is currently being written to the `index`-th position; however, writing process is not completed yet.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::ConcurrentVec;
    ///
    /// let con_vec = ConcurrentVec::new();
    /// con_vec.push('a');
    /// con_vec.push('b');
    ///
    /// assert_eq!(con_vec.get(0), Some(&'a'));
    /// assert_eq!(con_vec.get(1), Some(&'b'));
    /// assert_eq!(con_vec.get(2), None);
    /// ```
    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<&T> {
        unsafe { self.bag.get(index) }.and_then(|x| x.as_ref())
    }

    // mutate collection
    /// Concurrent, thread-safe method to push the given `value` to the back of the concurrent vector,
    /// and returns the position or index of the pushed value.
    ///
    /// It preserves the order of elements with respect to the order the `push` method is called.
    ///
    /// # Panics
    ///
    /// Panics if the concurrent vector is already at its maximum capacity; i.e., if `self.len() == self.maximum_capacity()`.
    ///
    /// Note that this is an important safety assertion in the concurrent context; however, not a practical limitation.
    /// Please see the [`ConcurrentVec::maximum_capacity`] for details.
    ///
    /// # Examples
    ///
    /// We can directly take a shared reference of the vector and share it among threads.
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let (num_threads, num_items_per_thread) = (4, 1_024);
    ///
    /// let con_vec = ConcurrentVec::new();
    ///
    /// // just take a reference and share among threads
    /// let vec_ref = &con_vec;
    ///
    /// std::thread::scope(|s| {
    ///     for i in 0..num_threads {
    ///         s.spawn(move || {
    ///             for j in 0..num_items_per_thread {
    ///                 // concurrently collect results simply by calling `push`
    ///                 vec_ref.push(i * 1000 + j);
    ///             }
    ///         });
    ///     }
    /// });
    ///
    /// let mut results: Vec<_> = con_vec.into_inner().iter().flatten().copied().collect();
    /// results.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(results, expected);
    /// ```
    ///
    /// # Performance Notes - False Sharing
    ///
    /// [`ConcurrentVec::push`] method is implementation is simple, lock-free and efficient.
    /// However, we need to be aware of the potential [false sharing](https://en.wikipedia.org/wiki/False_sharing) risk.
    /// False sharing might lead to significant performance degradation; fortunately, it is possible to avoid in many cases.
    ///
    /// ## When?
    ///
    /// Performance degradation due to false sharing might be observed when both of the following conditions hold:
    /// * **small data**: data to be pushed is small, the more elements fitting in a cache line the bigger the risk,
    /// * **little work**: multiple threads/cores are pushing to the concurrent vector with high frequency; i.e.,
    ///   * very little or negligible work / time is required in between `push` calls.
    ///
    /// The example above fits this situation.
    /// Each thread only performs one multiplication and addition in between pushing elements, and the elements to be pushed are very small, just one `usize`.
    ///
    /// ## Why?
    ///
    /// * `ConcurrentVec` assigns unique positions to each value to be pushed. There is no *true* sharing among threads in the position level.
    /// * However, cache lines contain more than one position.
    /// * One thread updating a particular position invalidates the entire cache line on an other thread.
    /// * Threads end up frequently reloading cache lines instead of doing the actual work of writing elements to the vector.
    /// * This might lead to a significant performance degradation.
    ///
    /// Following two methods could be approached to deal with this problem.
    ///
    /// ## Solution-I: `extend` rather than `push`
    ///
    /// One very simple, effective and memory efficient solution to this problem is to use [`ConcurrentVec::extend`] rather than `push` in *small data & little work* situations.
    ///
    /// Assume that we will have 4 threads and each will push 1_024 elements.
    /// Instead of making 1_024 `push` calls from each thread, we can make one `extend` call from each.
    /// This would give the best performance.
    /// Further, it has zero buffer or memory cost:
    /// * it is important to note that the batch of 1_024 elements are not stored temporarily in another buffer,
    /// * there is no additional allocation,
    /// * `extend` does nothing more than reserving the position range for the thread by incrementing the atomic counter accordingly.
    ///
    /// However, we do not need to have such a perfect information about the number of elements to be pushed.
    /// Performance gains after reaching the cache line size are much lesser.
    ///
    /// For instance, consider the challenging super small element size case, where we are collecting `i32`s.
    /// We can already achieve a very high performance by simply `extend`ing the bag by batches of 16 elements.
    ///
    /// As the element size gets larger, required batch size to achieve a high performance gets smaller and smaller.
    ///
    /// Required change in the code from `push` to `extend` is not significant.
    /// The example above could be revised as follows to avoid the performance degrading of false sharing.
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let (num_threads, num_items_per_thread) = (4, 1_024);
    ///
    /// let con_vec = ConcurrentVec::new();
    ///
    /// // just take a reference and share among threads
    /// let vec_ref = &con_vec;
    /// let batch_size = 16;
    ///
    /// std::thread::scope(|s| {
    ///     for i in 0..num_threads {
    ///         s.spawn(move || {
    ///             for j in (0..num_items_per_thread).step_by(batch_size) {
    ///                 let iter = (j..(j + batch_size)).map(|j| i * 1000 + j);
    ///                 // concurrently collect results simply by calling `extend`
    ///                 vec_ref.extend(iter);
    ///             }
    ///         });
    ///     }
    /// });
    ///
    /// let mut results: Vec<_> = con_vec.into_inner().iter().flatten().copied().collect();
    /// results.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(results, expected);
    /// ```
    ///
    /// ## Solution-II: Padding
    ///
    /// Another common approach to deal with false sharing is to add padding (unused bytes) between elements.
    /// There exist wrappers which automatically adds cache padding, such as crossbeam's [`CachePadded`](https://docs.rs/crossbeam-utils/latest/crossbeam_utils/struct.CachePadded.html).
    /// In other words, instead of using a `ConcurrentBag<T>`, we can use `ConcurrentBag<CachePadded<T>>`.
    /// However, this solution leads to increased memory requirement.
    #[inline(always)]
    pub fn push(&self, value: T) -> usize {
        self.bag.push(Some(value))
    }

    /// Concurrent, thread-safe method to push all `values` that the given iterator will yield to the back of the vector.
    /// The method returns the position or index of the first pushed value (returns the length of the concurrent vector if the iterator is empty).
    ///
    /// All `values` in the iterator will be added to the vector consecutively:
    /// * the first yielded value will be written to the position which is equal to the current length of the vector, say `begin_idx`, which is the returned value,
    /// * the second yielded value will be written to the `begin_idx + 1`-th position,
    /// * ...
    /// * and the last value will be written to the `begin_idx + values.count() - 1`-th position of the vector.
    ///
    /// Important notes:
    /// * This method does not allocate at all to buffer elements to be pushed.
    /// * All it does is to increment the atomic counter by the length of the iterator (`push` would increment by 1) and reserve the range of positions for this operation.
    /// * Iterating over and writing elements to the vector happens afterwards.
    /// * This is a simple, effective and memory efficient solution to the false sharing problem which could be observed in *small data & little work* situations.
    ///
    /// For this reason, the method requires an `ExactSizeIterator`.
    /// There exists the variant [`ConcurrentVec::extend_n_items`] method which accepts any iterator together with the correct length to be passed by the caller, hence it is `unsafe`.
    ///
    /// # Panics
    ///
    /// Panics if not all of the `values` fit in the concurrent vector's maximum capacity.
    ///
    /// Note that this is an important safety assertion in the concurrent context; however, not a practical limitation.
    /// Please see the [`ConcurrentVec::maximum_capacity`] for details.
    ///
    /// # Examples
    ///
    /// We can directly take a shared reference of the vector and share it among threads.
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let (num_threads, num_items_per_thread) = (4, 1_024);
    ///
    /// let vector = ConcurrentVec::new();
    ///
    /// // just take a reference and share among threads
    /// let vec_ref = &vector;
    /// let batch_size = 16;
    ///
    /// std::thread::scope(|s| {
    ///     for i in 0..num_threads {
    ///         s.spawn(move || {
    ///             for j in (0..num_items_per_thread).step_by(batch_size) {
    ///                 let iter = (j..(j + batch_size)).map(|j| i * 1000 + j);
    ///                 // concurrently collect results simply by calling `extend`
    ///                 vec_ref.extend(iter);
    ///             }
    ///         });
    ///     }
    /// });
    ///
    /// let mut results: Vec<_> = vector.into_inner().iter().flatten().copied().collect();
    /// results.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(results, expected);
    /// ```
    ///
    /// # Performance Notes - False Sharing
    ///
    /// [`ConcurrentVec::push`] method is implementation is simple, lock-free and efficient.
    /// However, we need to be aware of the potential [false sharing](https://en.wikipedia.org/wiki/False_sharing) risk.
    /// False sharing might lead to significant performance degradation; fortunately, it is possible to avoid in many cases.
    ///
    /// ## When?
    ///
    /// Performance degradation due to false sharing might be observed when both of the following conditions hold:
    /// * **small data**: data to be pushed is small, the more elements fitting in a cache line the bigger the risk,
    /// * **little work**: multiple threads/cores are pushing to the concurrent vector with high frequency; i.e.,
    ///   * very little or negligible work / time is required in between `push` calls.
    ///
    /// The example above fits this situation.
    /// Each thread only performs one multiplication and addition for computing elements, and the elements to be pushed are very small, just one `usize`.
    ///
    /// ## Why?
    ///
    /// * `ConcurrentVec` assigns unique positions to each value to be pushed. There is no *true* sharing among threads in the position level.
    /// * However, cache lines contain more than one position.
    /// * One thread updating a particular position invalidates the entire cache line on an other thread.
    /// * Threads end up frequently reloading cache lines instead of doing the actual work of writing elements to the vector.
    /// * This might lead to a significant performance degradation.
    ///
    /// Following two methods could be approached to deal with this problem.
    ///
    /// ## Solution-I: `extend` rather than `push`
    ///
    /// One very simple, effective and memory efficient solution to the false sharing problem is to use [`ConcurrentVec::extend`] rather than `push` in *small data & little work* situations.
    ///
    /// Assume that we will have 4 threads and each will push 1_024 elements.
    /// Instead of making 1_024 `push` calls from each thread, we can make one `extend` call from each.
    /// This would give the best performance.
    /// Further, it has zero buffer or memory cost:
    /// * it is important to note that the batch of 1_024 elements are not stored temporarily in another buffer,
    /// * there is no additional allocation,
    /// * `extend` does nothing more than reserving the position range for the thread by incrementing the atomic counter accordingly.
    ///
    /// However, we do not need to have such a perfect information about the number of elements to be pushed.
    /// Performance gains after reaching the cache line size are much lesser.
    ///
    /// For instance, consider the challenging super small element size case, where we are collecting `i32`s.
    /// We can already achieve a very high performance by simply `extend`ing the vector by batches of 16 elements.
    ///
    /// As the element size gets larger, required batch size to achieve a high performance gets smaller and smaller.
    ///
    /// The example code above already demonstrates the solution to a potentially problematic case in the [`ConcurrentVec::push`] example.
    ///
    /// ## Solution-II: Padding
    ///
    /// Another common approach to deal with false sharing is to add padding (unused bytes) between elements.
    /// There exist wrappers which automatically adds cache padding, such as crossbeam's [`CachePadded`](https://docs.rs/crossbeam-utils/latest/crossbeam_utils/struct.CachePadded.html).
    /// In other words, instead of using a `ConcurrentVec<T>`, we can use `ConcurrentVec<CachePadded<T>>`.
    /// However, this solution leads to increased memory requirement.
    #[inline(always)]
    pub fn extend<IntoIter, Iter>(&self, values: IntoIter) -> usize
    where
        IntoIter: IntoIterator<Item = T, IntoIter = Iter>,
        Iter: Iterator<Item = T> + ExactSizeIterator,
    {
        let values = values.into_iter().map(Some);
        self.bag.extend::<_, _>(values)
    }

    /// Concurrent, thread-safe method to push `num_items` elements yielded by the `values` iterator to the back of the vector.
    /// The method returns the position or index of the first pushed value (returns the length of the concurrent vector if the iterator is empty).
    ///
    /// All `values` in the iterator will be added to the vector consecutively:
    /// * the first yielded value will be written to the position which is equal to the current length of the vector, say `begin_idx`, which is the returned value,
    /// * the second yielded value will be written to the `begin_idx + 1`-th position,
    /// * ...
    /// * and the last value will be written to the `begin_idx + num_items - 1`-th position of the vector.
    ///
    /// Important notes:
    /// * This method does not allocate at all to buffer elements to be pushed.
    /// * All it does is to increment the atomic counter by the length of the iterator (`push` would increment by 1) and reserve the range of positions for this operation.
    /// * Iterating over and writing elements to the vector happens afterwards.
    /// * This is a simple, effective and memory efficient solution to the false sharing problem which could be observed in *small data & little work* situations.
    ///
    /// For this reason, the method requires the additional `num_items` argument.
    /// There exists the variant [`ConcurrentVec::extend`] method which accepts only an `ExactSizeIterator`, hence it is **safe**.
    ///
    /// # Panics
    ///
    /// Panics if `num_items` elements do not fit in the concurrent vector's maximum capacity.
    ///
    /// Note that this is an important safety assertion in the concurrent context; however, not a practical limitation.
    /// Please see the [`ConcurrentVec::maximum_capacity`] for details.
    ///
    /// # Safety
    ///
    /// As explained above, extend method calls first increment the atomic counter by `num_items`.
    /// This thread is responsible for filling these reserved `num_items` positions.
    /// * with safe `extend` method, this is guaranteed and safe since the iterator is an `ExactSizeIterator`;
    /// * however, `extend_n_items` accepts any iterator and `num_items` is provided explicitly by the caller.
    ///
    /// Ideally, the `values` iterator must yield exactly `num_items` elements and the caller is responsible for this condition to hold.
    ///
    /// If the `values` iterator is capable of yielding more than `num_items` elements,
    /// the `extend` call will extend the vector with the first `num_items` yielded elements and ignore the rest of the iterator.
    /// This is most likely a bug; however, not an undefined behavior.
    ///
    /// On the other hand, if the `values` iterator is short of `num_items` elements,
    /// this will lead to uninitialized memory positions in underlying storage of the vector which is UB.
    /// Therefore, this method is `unsafe`.
    ///
    /// # Examples
    ///
    /// We can directly take a shared reference of the vector and share it among threads.
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let (num_threads, num_items_per_thread) = (4, 1_024);
    ///
    /// let vector = ConcurrentVec::new();
    ///
    /// // just take a reference and share among threads
    /// let vec_ref = &vector;
    /// let batch_size = 16;
    ///
    /// std::thread::scope(|s| {
    ///     for i in 0..num_threads {
    ///         s.spawn(move || {
    ///             for j in (0..num_items_per_thread).step_by(batch_size) {
    ///                 let iter = (j..(j + batch_size)).map(|j| i * 1000 + j);
    ///                 // concurrently collect results simply by calling `extend_n_items`
    ///                 unsafe { vec_ref.extend_n_items(iter, batch_size) };
    ///             }
    ///         });
    ///     }
    /// });
    ///
    /// let mut results: Vec<_> = vector.into_inner().iter().flatten().copied().collect();
    /// results.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(results, expected);
    /// ```
    ///
    /// # Performance Notes - False Sharing
    ///
    /// [`ConcurrentVec::push`] method is implementation is simple, lock-free and efficient.
    /// However, we need to be aware of the potential [false sharing](https://en.wikipedia.org/wiki/False_sharing) risk.
    /// False sharing might lead to significant performance degradation; fortunately, it is possible to avoid in many cases.
    ///
    /// ## When?
    ///
    /// Performance degradation due to false sharing might be observed when both of the following conditions hold:
    /// * **small data**: data to be pushed is small, the more elements fitting in a cache line the bigger the risk,
    /// * **little work**: multiple threads/cores are pushing to the concurrent vector with high frequency; i.e.,
    ///   * very little or negligible work / time is required in between `push` calls.
    ///
    /// The example above fits this situation.
    /// Each thread only performs one multiplication and addition for computing elements, and the elements to be pushed are very small, just one `usize`.
    ///
    /// ## Why?
    ///
    /// * `ConcurrentVec` assigns unique positions to each value to be pushed. There is no *true* sharing among threads in the position level.
    /// * However, cache lines contain more than one position.
    /// * One thread updating a particular position invalidates the entire cache line on an other thread.
    /// * Threads end up frequently reloading cache lines instead of doing the actual work of writing elements to the vector.
    /// * This might lead to a significant performance degradation.
    ///
    /// Following two methods could be approached to deal with this problem.
    ///
    /// ## Solution-I: `extend` rather than `push`
    ///
    /// One very simple, effective and memory efficient solution to the false sharing problem is to use [`ConcurrentVec::extend`] rather than `push` in *small data & little work* situations.
    ///
    /// Assume that we will have 4 threads and each will push 1_024 elements.
    /// Instead of making 1_024 `push` calls from each thread, we can make one `extend` call from each.
    /// This would give the best performance.
    /// Further, it has zero buffer or memory cost:
    /// * it is important to note that the batch of 1_024 elements are not stored temporarily in another buffer,
    /// * there is no additional allocation,
    /// * `extend` does nothing more than reserving the position range for the thread by incrementing the atomic counter accordingly.
    ///
    /// However, we do not need to have such a perfect information about the number of elements to be pushed.
    /// Performance gains after reaching the cache line size are much lesser.
    ///
    /// For instance, consider the challenging super small element size case, where we are collecting `i32`s.
    /// We can already achieve a very high performance by simply `extend`ing the vector by batches of 16 elements.
    ///
    /// As the element size gets larger, required batch size to achieve a high performance gets smaller and smaller.
    ///
    /// The example code above already demonstrates the solution to a potentially problematic case in the [`ConcurrentVec::push`] example.
    ///
    /// ## Solution-II: Padding
    ///
    /// Another common approach to deal with false sharing is to add padding (unused bytes) between elements.
    /// There exist wrappers which automatically adds cache padding, such as crossbeam's [`CachePadded`](https://docs.rs/crossbeam-utils/latest/crossbeam_utils/struct.CachePadded.html).
    /// In other words, instead of using a `ConcurrentVec<T>`, we can use `ConcurrentVec<CachePadded<T>>`.
    /// However, this solution leads to increased memory requirement.
    #[inline(always)]
    pub unsafe fn extend_n_items<IntoIter>(&self, values: IntoIter, num_items: usize) -> usize
    where
        IntoIter: IntoIterator<Item = T>,
    {
        let values = values.into_iter().map(Some);
        self.bag.extend_n_items::<_>(values, num_items)
    }

    /// Clears the concurrent vector removing all already pushed elements.
    ///
    /// # Safety
    ///
    /// This method requires a mutually exclusive reference.
    /// This guarantees that there might not be any continuing writing process of a `push` operation.
    /// Therefore, the elements can safely be cleared.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::ConcurrentVec;
    ///
    /// let mut vector = ConcurrentVec::new();
    ///
    /// vector.push('a');
    /// vector.push('b');
    ///
    /// vector.clear();
    /// assert!(vector.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.bag.clear()
    }

    // helpers
    fn from_pinned_vec(pinned: P) -> Self {
        let bag = ConcurrentBag::new_with_mem_zeroing(pinned);
        Self { bag }
    }
}

unsafe impl<T: Sync, P: PinnedVec<Option<T>>> Sync for ConcurrentVec<T, P> {}

unsafe impl<T: Send, P: PinnedVec<Option<T>>> Send for ConcurrentVec<T, P> {}

#[cfg(test)]
mod tests {
    use super::*;
    use orx_split_vec::Recursive;
    use std::{
        collections::HashSet,
        sync::{Arc, Mutex},
        time::Duration,
    };

    #[test]
    fn write_read() {
        #[derive(Default, Debug)]
        struct Metric {
            sum: i32,
            count: i32,
        }
        impl Metric {
            fn aggregate(self, value: &i32) -> Self {
                Self {
                    sum: self.sum + value,
                    count: self.count + 1,
                }
            }
        }

        // record measurements in random intervals (read & write -> ConcurrentVec)
        let measurements = ConcurrentVec::new();
        let rf_measurements = &measurements;

        // collect metrics every 50 milliseconds (only write -> ConcurrentBag)
        let metrics = ConcurrentBag::new();
        let rf_metrics = &metrics;

        std::thread::scope(|s| {
            // thread to store measurements
            s.spawn(move || {
                for i in 0..100 {
                    std::thread::sleep(Duration::from_millis(i % 5));

                    // collect results and push to measurements vec
                    rf_measurements.push(i as i32);
                }
            });

            // thread to collect metrics every 50 milliseconds
            s.spawn(move || {
                for _ in 0..10 {
                    // safely read from measurements vec
                    let metric = rf_measurements
                        .iter()
                        .fold(Metric::default(), |x, value| x.aggregate(value));

                    // push results to metrics bag
                    // we may also use `ConcurrentVec` but we prefer `ConcurrentBag`
                    // since we don't concurrently read metrics
                    rf_metrics.push(metric);

                    std::thread::sleep(Duration::from_millis(50));
                }
            });

            // thread to print out the values to the stdout every 100 milliseconds
            s.spawn(move || {
                let mut idx = 0;
                loop {
                    let current_len = rf_measurements.len_exact();
                    let begin = idx;

                    for i in begin..current_len {
                        // safely read from measurements vec
                        if let Some(value) = rf_measurements.get(i) {
                            println!("[{}] = {:?}", i, value);
                            idx += 1;
                        } else {
                            idx = i;
                            break;
                        }
                    }

                    if current_len == 100 {
                        break;
                    }

                    std::thread::sleep(Duration::from_millis(100));
                }
            });
        });

        assert_eq!(measurements.len(), 100);
        assert_eq!(metrics.len(), 10);
    }

    #[test]
    fn new_len_empty_clear() {
        fn test<P: PinnedVec<Option<char>>>(bag: ConcurrentVec<char, P>) {
            let mut bag = bag;

            assert!(bag.is_empty());
            assert_eq!(0, bag.len());

            bag.push('a');

            assert!(!bag.is_empty());
            assert_eq!(1, bag.len());

            bag.push('b');
            bag.push('c');
            bag.push('d');

            assert!(!bag.is_empty());
            assert_eq!(4, bag.len());

            bag.clear();
            assert!(bag.is_empty());
            assert_eq!(0, bag.len());
        }

        test(ConcurrentVec::new());
        test(ConcurrentVec::default());
        test(ConcurrentVec::with_doubling_growth());
        test(ConcurrentVec::with_linear_growth(2, 8));
        test(ConcurrentVec::with_linear_growth(4, 8));
        test(ConcurrentVec::with_fixed_capacity(64));
    }

    #[test]
    fn capacity() {
        let mut split: SplitVec<_, Doubling> = (0..4).map(Some).collect();
        split.concurrent_reserve(5);
        let bag: ConcurrentVec<_, _> = split.into();
        assert_eq!(bag.capacity(), 4);
        bag.push(42);
        assert_eq!(bag.capacity(), 12);

        let mut split: SplitVec<_, Recursive> = (0..4).map(Some).collect();
        split.concurrent_reserve(5);
        let bag: ConcurrentVec<_, _> = split.into();
        assert_eq!(bag.capacity(), 4);
        bag.push(42);
        assert_eq!(bag.capacity(), 12);

        let mut split: SplitVec<_, Linear> = SplitVec::with_linear_growth(2);
        split.extend_from_slice(&[0, 1, 2, 3].map(Some));
        split.concurrent_reserve(5);
        let bag: ConcurrentVec<_, _> = split.into();
        assert_eq!(bag.capacity(), 4);
        bag.push(42);
        assert_eq!(bag.capacity(), 8);

        let mut fixed: FixedVec<_> = FixedVec::new(5);
        fixed.extend_from_slice(&[0, 1, 2, 3].map(Some));
        let bag: ConcurrentVec<_, _> = fixed.into();
        assert_eq!(bag.capacity(), 5);
        bag.push(42);
        assert_eq!(bag.capacity(), 5);
    }

    #[test]
    #[should_panic]
    fn exceeding_fixed_capacity_panics() {
        let mut fixed: FixedVec<_> = FixedVec::new(5);
        fixed.extend_from_slice(&[0, 1, 2, 3].map(Some));
        let bag: ConcurrentVec<_, _> = fixed.into();
        assert_eq!(bag.capacity(), 5);
        bag.push(42);
        bag.push(7);
    }

    #[test]
    #[should_panic]
    fn exceeding_fixed_capacity_panics_concurrently() {
        let bag = ConcurrentVec::with_fixed_capacity(10);
        let bag_ref = &bag;
        std::thread::scope(|s| {
            for _ in 0..4 {
                s.spawn(move || {
                    for _ in 0..3 {
                        // in total there will be 4*3 = 12 pushes
                        bag_ref.push(42);
                    }
                });
            }
        });
    }

    #[test]
    fn debug() {
        let bag = ConcurrentVec::new();

        bag.push('a');
        bag.push('b');
        bag.push('c');
        bag.push('d');
        bag.push('e');

        let str = format!("{:?}", bag);
        assert_eq!(
            str,
            "ConcurrentVec { bag: ConcurrentBag { pinned: [Some('a'), Some('b'), Some('c'), Some('d'), Some('e')], len: 5, capacity: 12, maximum_capacity: 17179869180 } }"
        );
    }

    #[test]
    fn get() {
        // record measurements in (assume) random intervals
        let measurements = ConcurrentVec::<i32>::new();
        let rf_measurements = &measurements;

        // collect sum of measurements every 50 milliseconds
        let sums = ConcurrentVec::new();
        let rf_sums = &sums;

        // collect average of measurements every 50 milliseconds
        let averages = ConcurrentVec::new();
        let rf_averages = &averages;

        std::thread::scope(|s| {
            s.spawn(move || {
                for i in 0..100 {
                    std::thread::sleep(Duration::from_millis(i % 5));
                    rf_measurements.push(i as i32);
                }
            });

            s.spawn(move || {
                for _ in 0..10 {
                    let mut sum = 0;
                    for i in 0..rf_measurements.len() {
                        sum += rf_measurements.get(i).copied().unwrap_or(0);
                    }
                    rf_sums.push(sum);
                    std::thread::sleep(Duration::from_millis(50));
                }
            });

            s.spawn(move || {
                for _ in 0..10 {
                    let count = rf_measurements.len();
                    if count == 0 {
                        rf_averages.push(0.0);
                    } else {
                        let mut sum = 0;
                        for i in 0..rf_measurements.len() {
                            sum += rf_measurements.get(i).copied().unwrap_or(0);
                        }
                        let average = sum as f32 / count as f32;
                        rf_averages.push(average);
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            });
        });

        assert_eq!(measurements.len(), 100);
        assert_eq!(sums.len(), 10);
        assert_eq!(averages.len(), 10);
    }

    #[test]
    fn iter() {
        let mut bag = ConcurrentVec::new();

        assert_eq!(0, bag.iter().count());

        bag.push('a');

        assert_eq!(vec!['a'], bag.iter().copied().collect::<Vec<_>>());

        bag.push('b');
        bag.push('c');
        bag.push('d');

        assert_eq!(
            vec!['a', 'b', 'c', 'd'],
            bag.iter().copied().collect::<Vec<_>>()
        );

        bag.clear();
        assert_eq!(0, bag.iter().count());
    }

    #[test]
    fn into_inner_from() {
        let bag = ConcurrentVec::new();

        bag.push('a');
        bag.push('b');
        bag.push('c');
        bag.push('d');
        assert_eq!(
            vec!['a', 'b', 'c', 'd'],
            bag.iter().copied().collect::<Vec<_>>()
        );

        let mut split = bag.into_inner();
        assert_eq!(
            vec!['a', 'b', 'c', 'd'],
            split.iter().flatten().copied().collect::<Vec<_>>()
        );

        split.push(Some('e'));
        *split.get_mut(0).expect("exists") = Some('x');

        assert_eq!(
            vec!['x', 'b', 'c', 'd', 'e'],
            split.iter().flatten().copied().collect::<Vec<_>>()
        );

        let mut bag: ConcurrentVec<_> = split.into();
        assert_eq!(
            vec!['x', 'b', 'c', 'd', 'e'],
            bag.iter().copied().collect::<Vec<_>>()
        );

        bag.clear();
        assert!(bag.is_empty());

        let split = bag.into_inner();
        assert!(split.is_empty());
    }

    #[test]
    fn ok_at_num_threads() {
        use std::thread::available_parallelism;
        let default_parallelism_approx = available_parallelism().expect("is-ok").get();
        dbg!(default_parallelism_approx);

        let num_threads = default_parallelism_approx;
        let num_items_per_thread = 16384;

        let bag = ConcurrentVec::new();
        let bag_ref = &bag;
        std::thread::scope(|s| {
            for i in 0..num_threads {
                s.spawn(move || {
                    for j in 0..num_items_per_thread {
                        bag_ref.push((i * 100000 + j) as i32);
                    }
                });
            }
        });

        let pinned = bag.into_inner();
        assert_eq!(pinned.len(), num_threads * num_items_per_thread);
    }

    #[test]
    fn push_indices() {
        let num_threads = 4;
        let num_items_per_thread = 16;

        let indices_set = Arc::new(Mutex::new(HashSet::new()));

        let bag = ConcurrentVec::new();
        let bag_ref = &bag;
        std::thread::scope(|s| {
            for i in 0..num_threads {
                let indices_set = indices_set.clone();
                s.spawn(move || {
                    for j in 0..num_items_per_thread {
                        let idx = bag_ref.push(i * 100000 + j);
                        let mut set = indices_set.lock().expect("is ok");
                        set.insert(idx);
                    }
                });
            }
        });

        let set = indices_set.lock().expect("is ok");
        assert_eq!(set.len(), 4 * 16);
        for i in 0..(4 * 16) {
            assert!(set.contains(&i));
        }
    }

    #[test]
    fn reserve_maximum_capacity() {
        // SplitVec<_, Doubling>
        let bag: ConcurrentVec<char> = ConcurrentVec::new();
        assert_eq!(bag.capacity(), 4); // only allocates the first fragment of 4
        assert_eq!(bag.maximum_capacity(), 17_179_869_180); // it can grow safely & exponentially

        let bag: ConcurrentVec<char, _> = ConcurrentVec::with_doubling_growth();
        assert_eq!(bag.capacity(), 4);
        assert_eq!(bag.maximum_capacity(), 17_179_869_180);

        // SplitVec<_, Linear>
        let mut bag: ConcurrentVec<char, _> = ConcurrentVec::with_linear_growth(10, 20);
        assert_eq!(bag.capacity(), 2usize.pow(10)); // only allocates first fragment of 1024
        assert_eq!(bag.maximum_capacity(), 2usize.pow(10) * 20); // it can concurrently allocate 19 more

        // SplitVec<_, Linear> -> reserve_maximum_capacity
        let result = bag.reserve_maximum_capacity(2usize.pow(10) * 30);
        assert_eq!(result, Ok(2usize.pow(10) * 30));

        // actually no new allocation yet; precisely additional memory for 10 pairs of pointers is used
        assert_eq!(bag.capacity(), 2usize.pow(10)); // first fragment capacity

        dbg!(bag.maximum_capacity(), 2usize.pow(10) * 30);
        assert_eq!(bag.maximum_capacity(), 2usize.pow(10) * 30); // now it can safely reach 2^10 * 30

        // FixedVec<_>: pre-allocated, exact and strict
        let mut bag: ConcurrentVec<char, _> = ConcurrentVec::with_fixed_capacity(42);
        assert_eq!(bag.capacity(), 42);
        assert_eq!(bag.maximum_capacity(), 42);

        let result = bag.reserve_maximum_capacity(43);
        assert_eq!(
            result,
            Err(PinnedVecGrowthError::FailedToGrowWhileKeepingElementsPinned)
        );
    }

    #[test]
    fn extend_indices() {
        let num_threads = 4;
        let num_items_per_thread = 16;

        let indices_set = Arc::new(Mutex::new(HashSet::new()));

        let bag = ConcurrentVec::new();
        let bag_ref = &bag;
        std::thread::scope(|s| {
            for i in 0..num_threads {
                let indices_set = indices_set.clone();
                s.spawn(move || {
                    let iter = (0..num_items_per_thread).map(|j| i * 100000 + j);

                    let begin_idx = bag_ref.extend(iter);

                    let mut set = indices_set.lock().expect("is ok");
                    set.insert(begin_idx);
                });
            }
        });

        let set = indices_set.lock().expect("is ok");
        assert_eq!(set.len(), num_threads);
        for i in 0..num_threads {
            assert!(set.contains(&(i * num_items_per_thread)));
        }
    }

    #[test]
    fn extend_n_items_indices() {
        let num_threads = 4;
        let num_items_per_thread = 16;

        let indices_set = Arc::new(Mutex::new(HashSet::new()));

        let bag = ConcurrentVec::new();
        let bag_ref = &bag;
        std::thread::scope(|s| {
            for i in 0..num_threads {
                let indices_set = indices_set.clone();
                s.spawn(move || {
                    let iter = (0..num_items_per_thread).map(|j| i * 100000 + j);

                    let begin_idx = unsafe { bag_ref.extend_n_items(iter, num_items_per_thread) };

                    let mut set = indices_set.lock().expect("is ok");
                    set.insert(begin_idx);
                });
            }
        });

        let set = indices_set.lock().expect("is ok");
        assert_eq!(set.len(), num_threads);
        for i in 0..num_threads {
            assert!(set.contains(&(i * num_items_per_thread)));
        }
    }
}
