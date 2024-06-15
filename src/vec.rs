use orx_concurrent_bag::ConcurrentBag;
use orx_pinned_vec::PinnedVec;
use orx_split_vec::{Doubling, SplitVec};
use std::ops::{Deref, DerefMut};

/// An efficient, convenient and lightweight grow-only read & write concurrent data structure allowing high performance concurrent collection.
///
/// * **convenient**: `ConcurrentVec` can safely be shared among threads simply as a shared reference. It is a [`PinnedConcurrentCol`](https://crates.io/crates/orx-pinned-concurrent-col) with a special concurrent state implementation. Underlying [`PinnedVec`](https://crates.io/crates/orx-pinned-vec) and concurrent bag can be converted back and forth to each other.
/// * **efficient**: `ConcurrentVec` is a lock free structure making use of a few atomic primitives, this leads to high performance concurrent growth. You may see the details in <a href="#section-benchmarks">benchmarks</a> and further <a href="#section-performance-notes">performance notes</a>.
///
/// Note that `ConcurrentVec` is a read & write collection with the cost to store values wrapped with an optional and initializing memory on allocation. See [`ConcurrentBag`](https://crates.io/crates/orx-concurrent-bag) for a write only and a more performant variant. Having almost identical api, switching between `ConcurrentVec` and `ConcurrentBag` is straightforward.
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
///
/// ## Construction
///
/// `ConcurrentVec` can be constructed by wrapping any pinned vector; i.e., `ConcurrentVec<T>` implements `From<P: PinnedVec<Option<T>>>`.
/// Likewise, a concurrent vector can be unwrapped without any cost to the underlying pinned vector with `into_inner` method.
///
/// Further, there exist `with_` methods to directly construct the concurrent bag with common pinned vector implementations.
///
/// ```rust
/// use orx_concurrent_vec::*;
///
/// // default pinned vector -> SplitVec<T, Doubling>
/// let vec: ConcurrentVec<char> = ConcurrentVec::new();
/// let vec: ConcurrentVec<char> = Default::default();
/// let vec: ConcurrentVec<char> = ConcurrentVec::with_doubling_growth();
/// let vec: ConcurrentVec<char, SplitVec<Option<char>, Doubling>> = ConcurrentVec::with_doubling_growth();
///
/// let vec: ConcurrentVec<char> = SplitVec::new().into();
/// let vec: ConcurrentVec<char, SplitVec<Option<char>, Doubling>> = SplitVec::new().into();
///
/// // SplitVec with [Linear](https://docs.rs/orx-split-vec/latest/orx_split_vec/struct.Linear.html) growth
/// // each fragment will have capacity 2^10 = 1024
/// // and the split vector can grow up to 32 fragments
/// let vec: ConcurrentVec<char, SplitVec<Option<char>, Linear>> = ConcurrentVec::with_linear_growth(10, 32);
/// let vec: ConcurrentVec<char, SplitVec<Option<char>, Linear>> = SplitVec::with_linear_growth_and_fragments_capacity(10, 32).into();
///
/// // [FixedVec](https://docs.rs/orx-fixed-vec/latest/orx_fixed_vec/) with fixed capacity.
/// // Fixed vector cannot grow; hence, pushing the 1025-th element to this vec will cause a panic!
/// let vec: ConcurrentVec<char, FixedVec<Option<char>>> = ConcurrentVec::with_fixed_capacity(1024);
/// let vec: ConcurrentVec<char, FixedVec<Option<char>>> = FixedVec::new(1024).into();
/// ```
///
/// Of course, the pinned vector to be wrapped does not need to be empty.
///
/// ```rust
/// use orx_concurrent_vec::*;
///
/// let split_vec: SplitVec<Option<i32>> = (0..1024).map(Some).collect();
/// let vec: ConcurrentVec<_> = split_vec.into();
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
#[derive(Debug)]
pub struct ConcurrentVec<T, P = SplitVec<Option<T>, Doubling>>
where
    P: PinnedVec<Option<T>>,
{
    bag: ConcurrentBag<Option<T>, P>,
}

impl<T, P> ConcurrentVec<T, P>
where
    P: PinnedVec<Option<T>>,
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
        self.bag.into_inner()
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
    /// Please see the [`orx_pinned_concurrent_col::PinnedConcurrentCol::maximum_capacity`] for details.
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
    /// [`ConcurrentVec::push`] implementation is lock-free and focuses on efficiency.
    /// However, we need to be aware of the potential [false sharing](https://en.wikipedia.org/wiki/False_sharing) risk.
    /// False sharing might lead to significant performance degradation.
    /// However, it is possible to avoid in many cases.
    ///
    /// ## When?
    ///
    /// Performance degradation due to false sharing might be observed when both of the following conditions hold:
    /// * **small data**: data to be pushed is small, the more elements fitting in a cache line the bigger the risk,
    /// * **little work**: multiple threads/cores are pushing to the concurrent bag with high frequency; i.e.,
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
    /// * Threads end up frequently reloading cache lines instead of doing the actual work of writing elements to the bag.
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
    /// let bag = ConcurrentVec::new();
    ///
    /// // just take a reference and share among threads
    /// let bag_ref = &bag;
    /// let batch_size = 16;
    ///
    /// std::thread::scope(|s| {
    ///     for i in 0..num_threads {
    ///         s.spawn(move || {
    ///             for j in (0..num_items_per_thread).step_by(batch_size) {
    ///                 let iter = (j..(j + batch_size)).map(|j| i * 1000 + j);
    ///                 // concurrently collect results simply by calling `extend`
    ///                 bag_ref.extend(iter);
    ///             }
    ///         });
    ///     }
    /// });
    ///
    /// let mut vec_from_bag: Vec<_> = bag.into_inner().iter().flatten().copied().collect();
    /// vec_from_bag.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(vec_from_bag, expected);
    /// ```
    ///
    /// ## Solution-II: Padding
    ///
    /// Another approach to deal with false sharing is to add padding (unused bytes) between elements.
    /// There exist wrappers which automatically adds cache padding, such as crossbeam's [`CachePadded`](https://docs.rs/crossbeam-utils/latest/crossbeam_utils/struct.CachePadded.html).
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
    /// * This method does not allocate to buffer.
    /// * All it does is to increment the atomic counter by the length of the iterator (`push` would increment by 1) and reserve the range of positions for this operation.
    /// * If there is not sufficient space, the vector grows first; iterating over and writing elements to the vector happens afterwards.
    /// * Therefore, other threads do not wait for the `extend` method to complete, they can concurrently write.
    /// * This is a simple and effective approach to deal with the false sharing problem which could be observed in *small data & little work* situations.
    ///
    /// For this reason, the method requires an `ExactSizeIterator`.
    /// There exists the variant [`ConcurrentBag::extend_n_items`] method which accepts any iterator together with the correct length to be passed by the caller.
    /// It is `unsafe` as the caller must guarantee that the iterator yields at least the number of elements explicitly passed in as an argument.
    ///
    /// # Panics
    ///
    /// Panics if not all of the `values` fit in the concurrent vector's maximum capacity.
    ///
    /// Note that this is an important safety assertion in the concurrent context; however, not a practical limitation.
    /// Please see the [`orx_pinned_concurrent_col::PinnedConcurrentCol::maximum_capacity`] for details.
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
    /// let bag = ConcurrentVec::new();
    ///
    /// // just take a reference and share among threads
    /// let bag_ref = &bag;
    /// let batch_size = 16;
    ///
    /// std::thread::scope(|s| {
    ///     for i in 0..num_threads {
    ///         s.spawn(move || {
    ///             for j in (0..num_items_per_thread).step_by(batch_size) {
    ///                 let iter = (j..(j + batch_size)).map(|j| i * 1000 + j);
    ///                 // concurrently collect results simply by calling `extend`
    ///                 bag_ref.extend(iter);
    ///             }
    ///         });
    ///     }
    /// });
    ///
    /// let mut vec_from_bag: Vec<_> = bag.into_inner().iter().flatten().copied().collect();
    /// vec_from_bag.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(vec_from_bag, expected);
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
    /// * **little work**: multiple threads/cores are pushing to the concurrent bag with high frequency; i.e.,
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
    /// * Threads end up frequently reloading cache lines instead of doing the actual work of writing elements to the bag.
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
    /// We can already achieve a very high performance by simply `extend`ing the bag by batches of 16 elements.
    ///
    /// As the element size gets larger, required batch size to achieve a high performance gets smaller and smaller.
    ///
    /// The example code above already demonstrates the solution to a potentially problematic case in the [`ConcurrentVec::push`] example.
    ///
    /// ## Solution-II: Padding
    ///
    /// Another common approach to deal with false sharing is to add padding (unused bytes) between elements.
    /// There exist wrappers which automatically adds cache padding, such as crossbeam's [`CachePadded`](https://docs.rs/crossbeam-utils/latest/crossbeam_utils/struct.CachePadded.html).
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
    /// Please see the [`orx_pinned_concurrent_col::PinnedConcurrentCol::maximum_capacity`] for details.
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
    /// let bag = ConcurrentVec::new();
    ///
    /// // just take a reference and share among threads
    /// let bag_ref = &bag;
    /// let batch_size = 16;
    ///
    /// std::thread::scope(|s| {
    ///     for i in 0..num_threads {
    ///         s.spawn(move || {
    ///             for j in (0..num_items_per_thread).step_by(batch_size) {
    ///                 let iter = (j..(j + batch_size)).map(|j| i * 1000 + j);
    ///                 // concurrently collect results simply by calling `extend_n_items`
    ///                 unsafe { bag_ref.extend_n_items(iter, batch_size) };
    ///             }
    ///         });
    ///     }
    /// });
    ///
    /// let mut vec_from_bag: Vec<_> = bag.into_inner().iter().flatten().copied().collect();
    /// vec_from_bag.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(vec_from_bag, expected);
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
    /// * **little work**: multiple threads/cores are pushing to the concurrent bag with high frequency; i.e.,
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
    /// * Threads end up frequently reloading cache lines instead of doing the actual work of writing elements to the bag.
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
    /// We can already achieve a very high performance by simply `extend`ing the bag by batches of 16 elements.
    ///
    /// As the element size gets larger, required batch size to achieve a high performance gets smaller and smaller.
    ///
    /// The example code above already demonstrates the solution to a potentially problematic case in the [`ConcurrentVec::push`] example.
    ///
    /// ## Solution-II: Padding
    ///
    /// Another common approach to deal with false sharing is to add padding (unused bytes) between elements.
    /// There exist wrappers which automatically adds cache padding, such as crossbeam's [`CachePadded`](https://docs.rs/crossbeam-utils/latest/crossbeam_utils/struct.CachePadded.html).
    /// However, this solution leads to increased memory requirement.
    #[inline(always)]
    pub unsafe fn extend_n_items<IntoIter>(&self, values: IntoIter, num_items: usize) -> usize
    where
        IntoIter: IntoIterator<Item = T>,
    {
        let values = values.into_iter().map(Some);
        self.bag.extend_n_items::<_>(values, num_items)
    }
}

// HELPERS

impl<T, P> ConcurrentVec<T, P>
where
    P: PinnedVec<Option<T>>,
{
    #[inline]
    pub(crate) fn new_from_pinned(pinned_vec: P) -> Self {
        let bag: ConcurrentBag<_, _> = pinned_vec.into();
        Self { bag }
    }
}

// TRAITS

impl<T, P> Deref for ConcurrentVec<T, P>
where
    P: PinnedVec<Option<T>>,
{
    type Target = ConcurrentBag<Option<T>, P>;

    fn deref(&self) -> &Self::Target {
        &self.bag
    }
}

impl<T, P> DerefMut for ConcurrentVec<T, P>
where
    P: PinnedVec<Option<T>>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bag
    }
}

unsafe impl<T: Sync, P: PinnedVec<Option<T>>> Sync for ConcurrentVec<T, P> {}

unsafe impl<T: Send, P: PinnedVec<Option<T>>> Send for ConcurrentVec<T, P> {}
