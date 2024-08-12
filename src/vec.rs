use crate::state::ConcurrentVecState;
use orx_concurrent_option::ConcurrentOption;
use orx_pinned_concurrent_col::PinnedConcurrentCol;
use orx_pinned_vec::IntoConcurrentPinnedVec;
use orx_split_vec::{Doubling, SplitVec};
use std::{fmt::Debug, sync::atomic::Ordering};

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
/// ```rust
/// use orx_concurrent_vec::prelude::*;
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
#[derive(Debug)]
pub struct ConcurrentVec<T, P = SplitVec<ConcurrentOption<T>, Doubling>>
where
    P: IntoConcurrentPinnedVec<ConcurrentOption<T>>,
{
    core: PinnedConcurrentCol<ConcurrentOption<T>, P::ConPinnedVec, ConcurrentVecState>,
}

impl<T, P> ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentOption<T>>,
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
        self.core.state().len()
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

    /// Returns a reference to the element at the `index`-th position of the vec.
    /// It returns `None` when index is out of bounds.
    ///
    /// # Safety
    ///
    /// Reference obtained by this method will be valid:
    ///
    /// * `ConcurrentVec` guarantees that each position is written only and exactly once.
    /// * Furthermore, underlying `ConcurrentOption` wrapper prevents access during initialization, preventing data race.
    /// * Finally, underlying `PinnedVec` makes sure that memory location of the elements do not change.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    ///
    /// vec.push('a');
    /// vec.extend(['b', 'c', 'd']);
    ///
    /// assert_eq!(vec.get(0), Some(&'a'));
    /// assert_eq!(vec.get(1), Some(&'b'));
    /// assert_eq!(vec.get(2), Some(&'c'));
    /// assert_eq!(vec.get(3), Some(&'d'));
    /// assert_eq!(vec.get(4), None);
    /// ```
    ///
    /// The following could be considered as a practical use case.
    ///
    /// ```rust
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
    ///             let len = rf_measurements.len();
    ///             let mut metric = Metric::default();
    ///             for i in 0..len {
    ///                 if let Some(value) = rf_measurements.get(i) {
    ///                     metric = metric.aggregate(value);
    ///                     }
    ///             }
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
    pub fn get(&self, index: usize) -> Option<&T> {
        match index < self.len() {
            true => {
                let maybe = unsafe { self.core.get(index) };
                maybe.and_then(|x| unsafe { x.as_ref_with_order(Ordering::SeqCst) })
            }
            false => None,
        }
    }

    /// Returns a mutable reference to the element at the `index`-th position of the bag.
    /// It returns `None` when index is out of bounds.
    ///
    /// # Safety
    ///
    /// At first it might be confusing that `get` method is unsafe; however, `get_mut` is safe.
    /// This is due to `&mut self` requirement of the `get_mut` method.
    ///
    /// The following paragraph from `get` docs demonstrates an example that could lead to undefined behavior.
    /// The race condition (with `get`) could be observed in the following unsafe usage.
    /// Say we have a `bag` of `char`s and we allocate memory to store incoming characters, say 4 positions.
    /// If the following events happen in the exact order in time, we might have undefined behavior (UB):
    /// * `bag.push('a')` is called from thread#1.
    /// * `bag` atomically increases the `len` to 1.
    /// * thread#2 calls `bag.get(0)` which is now in bounds.
    /// * thread#2 receives uninitialized value (UB).
    /// * thread#1 completes writing `'a'` to the 0-th position (one moment too late).
    ///
    /// This scenario would not compile with `get_mut` requiring a `&mut self`. Therefore, `get_mut` is safe.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_bag::*;
    ///
    /// let mut bag = ConcurrentBag::new();
    ///
    /// bag.push('a');
    /// bag.extend(['b', 'c', 'd']);
    ///
    /// assert_eq!(unsafe { bag.get_mut(4) }, None);
    ///
    /// *bag.get_mut(1).unwrap() = 'x';
    /// assert_eq!(unsafe { bag.get(1) }, Some(&'x'));
    /// ```
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        match index < self.len() {
            true => {
                let maybe = unsafe { self.core.get_mut(index) };
                maybe.and_then(|x| x.exclusive_as_mut())
            }
            false => None,
        }
    }

    /// Returns an iterator to elements of the vec.
    ///
    /// Iteration of elements is in the order the push method is called.
    ///
    /// # Safety
    ///
    /// Reference obtained by this method will be valid:
    ///
    /// * `ConcurrentVec` guarantees that each position is written only and exactly once.
    /// * Furthermore, underlying `ConcurrentOption` wrapper prevents access during initialization, preventing data race.
    /// * Finally, underlying `PinnedVec` makes sure that memory location of the elements do not change.
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
    /// let mut iter = vec.iter();
    /// assert_eq!(iter.next(), Some(&'a'));
    /// assert_eq!(iter.next(), Some(&'b'));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// The following could be considered as a practical use case.
    ///
    /// ```rust
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
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let x = unsafe { self.core.iter(self.len()) };
        x.flat_map(|x| unsafe { x.as_ref_with_order(Ordering::SeqCst) })
    }

    /// Returns an iterator to elements of the bag.
    ///
    /// Iteration of elements is in the order the push method is called.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_bag::ConcurrentBag;
    ///
    /// let mut bag = ConcurrentBag::new();
    /// bag.push("a".to_string());
    /// bag.push("b".to_string());
    ///
    /// for x in bag.iter_mut() {
    ///     *x = format!("{}!", x);
    /// }
    ///
    /// let mut iter = unsafe { bag.iter() };
    /// assert_eq!(iter.next(), Some(&String::from("a!")));
    /// assert_eq!(iter.next(), Some(&String::from("b!")));
    /// assert_eq!(iter.next(), None);
    /// ```
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        let x = unsafe { self.core.iter_mut(self.core.state().len()) };
        x.flat_map(|x| x.exclusive_as_mut())
    }

    /// Concurrent, thread-safe method to push the given `value` to the back of the bag, and returns the position or index of the pushed value.
    ///
    /// It preserves the order of elements with respect to the order the `push` method is called.
    ///
    /// # Panics
    ///
    /// Panics if the concurrent bag is already at its maximum capacity; i.e., if `self.len() == self.maximum_capacity()`.
    ///
    /// Note that this is an important safety assertion in the concurrent context; however, not a practical limitation.
    /// Please see the [`PinnedConcurrentCol::maximum_capacity`] for details.
    ///
    /// # Examples
    ///
    /// We can directly take a shared reference of the bag, share it among threads and collect results concurrently.
    ///
    /// ```rust
    /// use orx_concurrent_bag::*;
    ///
    /// let (num_threads, num_items_per_thread) = (4, 1_024);
    ///
    /// let bag = ConcurrentBag::new();
    ///
    /// // just take a reference and share among threads
    /// let bag_ref = &bag;
    ///
    /// std::thread::scope(|s| {
    ///     for i in 0..num_threads {
    ///         s.spawn(move || {
    ///             for j in 0..num_items_per_thread {
    ///                 // concurrently collect results simply by calling `push`
    ///                 bag_ref.push(i * 1000 + j);
    ///             }
    ///         });
    ///     }
    /// });
    ///
    /// let mut vec_from_bag: Vec<_> = bag.into_inner().iter().copied().collect();
    /// vec_from_bag.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(vec_from_bag, expected);
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
    /// * `ConcurrentBag` assigns unique positions to each value to be pushed. There is no *true* sharing among threads in the position level.
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
    /// use orx_concurrent_bag::*;
    ///
    /// let (num_threads, num_items_per_thread) = (4, 1_024);
    ///
    /// let bag = ConcurrentBag::new();
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
    /// let mut vec_from_bag: Vec<_> = bag.into_inner().iter().copied().collect();
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
    /// In other words, instead of using a `ConcurrentBag<T>`, we can use `ConcurrentBag<CachePadded<T>>`.
    /// However, this solution leads to increased memory requirement.
    pub fn push(&self, value: T) -> usize {
        let idx = self.core.state().fetch_increment_len(1);

        // # SAFETY: ConcurrentBag ensures that each `idx` will be written only and exactly once.
        let maybe = unsafe { self.core.single_item_as_ref(idx) };
        // assert!(maybe.is_none_with_order(Ordering::SeqCst));
        // maybe.initiate_if_none(value);
        unsafe { maybe.initialize_unchecked(value) };
        // assert!(maybe.is_none_with_order(Ordering::SeqCst));

        idx
    }

    /// Concurrent, thread-safe method to push all `values` that the given iterator will yield to the back of the bag.
    /// The method returns the position or index of the first pushed value (returns the length of the concurrent bag if the iterator is empty).
    ///
    /// All `values` in the iterator will be added to the bag consecutively:
    /// * the first yielded value will be written to the position which is equal to the current length of the bag, say `begin_idx`, which is the returned value,
    /// * the second yielded value will be written to the `begin_idx + 1`-th position,
    /// * ...
    /// * and the last value will be written to the `begin_idx + values.count() - 1`-th position of the bag.
    ///
    /// Important notes:
    /// * This method does not allocate to buffer.
    /// * All it does is to increment the atomic counter by the length of the iterator (`push` would increment by 1) and reserve the range of positions for this operation.
    /// * If there is not sufficient space, the vector grows first; iterating over and writing elements to the bag happens afterwards.
    /// * Therefore, other threads do not wait for the `extend` method to complete, they can concurrently write.
    /// * This is a simple and effective approach to deal with the false sharing problem which could be observed in *small data & little work* situations.
    ///
    /// For this reason, the method requires an `ExactSizeIterator`.
    /// There exists the variant [`ConcurrentVec::extend_n_items`] method which accepts any iterator together with the correct length to be passed by the caller.
    /// It is `unsafe` as the caller must guarantee that the iterator yields at least the number of elements explicitly passed in as an argument.
    ///
    /// # Panics
    ///
    /// Panics if not all of the `values` fit in the concurrent bag's maximum capacity.
    ///
    /// Note that this is an important safety assertion in the concurrent context; however, not a practical limitation.
    /// Please see the [`PinnedConcurrentCol::maximum_capacity`] for details.
    ///
    /// # Examples
    ///
    /// We can directly take a shared reference of the bag and share it among threads.
    ///
    /// ```rust
    /// use orx_concurrent_bag::*;
    ///
    /// let (num_threads, num_items_per_thread) = (4, 1_024);
    ///
    /// let bag = ConcurrentBag::new();
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
    /// let mut vec_from_bag: Vec<_> = bag.into_inner().iter().copied().collect();
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
    /// * `ConcurrentBag` assigns unique positions to each value to be pushed. There is no *true* sharing among threads in the position level.
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
    /// In other words, instead of using a `ConcurrentBag<T>`, we can use `ConcurrentBag<CachePadded<T>>`.
    /// However, this solution leads to increased memory requirement.
    pub fn extend<IntoIter, Iter>(&self, values: IntoIter) -> usize
    where
        IntoIter: IntoIterator<Item = T, IntoIter = Iter>,
        Iter: Iterator<Item = T> + ExactSizeIterator,
    {
        let values = values.into_iter();
        let num_items = values.len();
        // # SAFETY: ConcurrentBag ensures that each `idx` will be written only and exactly once.
        unsafe { self.extend_n_items::<_>(values, num_items) }
    }

    /// Concurrent, thread-safe method to push `num_items` elements yielded by the `values` iterator to the back of the bag.
    /// The method returns the position or index of the first pushed value (returns the length of the concurrent bag if the iterator is empty).
    ///
    /// All `values` in the iterator will be added to the bag consecutively:
    /// * the first yielded value will be written to the position which is equal to the current length of the bag, say `begin_idx`, which is the returned value,
    /// * the second yielded value will be written to the `begin_idx + 1`-th position,
    /// * ...
    /// * and the last value will be written to the `begin_idx + num_items - 1`-th position of the bag.
    ///
    /// Important notes:
    /// * This method does not allocate at all to buffer elements to be pushed.
    /// * All it does is to increment the atomic counter by the length of the iterator (`push` would increment by 1) and reserve the range of positions for this operation.
    /// * Iterating over and writing elements to the bag happens afterwards.
    /// * This is a simple, effective and memory efficient solution to the false sharing problem which could be observed in *small data & little work* situations.
    ///
    /// For this reason, the method requires the additional `num_items` argument.
    /// There exists the variant [`ConcurrentVec::extend`] method which accepts only an `ExactSizeIterator`, hence it is **safe**.
    ///
    /// # Panics
    ///
    /// Panics if `num_items` elements do not fit in the concurrent bag's maximum capacity.
    ///
    /// Note that this is an important safety assertion in the concurrent context; however, not a practical limitation.
    /// Please see the [`PinnedConcurrentCol::maximum_capacity`] for details.
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
    /// the `extend` call will extend the bag with the first `num_items` yielded elements and ignore the rest of the iterator.
    /// This is most likely a bug; however, not an undefined behavior.
    ///
    /// On the other hand, if the `values` iterator is short of `num_items` elements,
    /// this will lead to uninitialized memory positions in underlying storage of the bag which is UB.
    /// Therefore, this method is `unsafe`.
    ///
    /// # Examples
    ///
    /// We can directly take a shared reference of the bag and share it among threads.
    ///
    /// ```rust
    /// use orx_concurrent_bag::*;
    ///
    /// let (num_threads, num_items_per_thread) = (4, 1_024);
    ///
    /// let bag = ConcurrentBag::new();
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
    /// let mut vec_from_bag: Vec<_> = bag.into_inner().iter().copied().collect();
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
    /// * `ConcurrentBag` assigns unique positions to each value to be pushed. There is no *true* sharing among threads in the position level.
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
    /// In other words, instead of using a `ConcurrentBag<T>`, we can use `ConcurrentBag<CachePadded<T>>`.
    /// However, this solution leads to increased memory requirement.
    pub unsafe fn extend_n_items<IntoIter>(&self, values: IntoIter, num_items: usize) -> usize
    where
        IntoIter: IntoIterator<Item = T>,
    {
        let begin_idx = self.core.state().fetch_increment_len(num_items);
        let slices = self.core.n_items_buffer_as_slices(begin_idx, num_items);
        let mut values = values.into_iter();

        for slice in slices {
            for maybe in slice {
                // assert!(maybe.is_none_with_order(Ordering::SeqCst));
                let value = values
                    .next()
                    .expect("provided iterator is shorter than expected num_items");
                // maybe.initiate_if_none(value);
                unsafe { maybe.initialize_unchecked(value) };
                // assert!(maybe.is_some_with_order(Ordering::SeqCst));
            }
        }

        // let values = values.into_iter().map(ConcurrentOption::from);
        // self.core.write_n_items(begin_idx, num_items, values);
        begin_idx
    }

    /// Reserves and returns an iterator of mutable slices for `num_items` positions starting from the `begin_idx`-th position.
    ///
    /// The caller is responsible for filling all `num_items` positions in the returned iterator of slices with values to avoid gaps.
    ///
    /// # Safety
    ///
    /// This method makes sure that the values are written to positions owned by the underlying pinned vector.
    /// Furthermore, it makes sure that the growth of the vector happens thread-safely whenever necessary.
    ///
    /// On the other hand, it is unsafe due to the possibility of a race condition.
    /// Multiple threads can try to write to the same position at the same time.
    /// The wrapper is responsible for preventing this.
    ///
    /// Furthermore, the caller is responsible to write all positions of the acquired slices to make sure that the collection is gap free.
    ///
    /// Note that although both methods are unsafe, it is much easier to achieve required safety guarantees with `extend` or `extend_n_items`;
    /// hence, they must be preferred unless there is a good reason to acquire mutable slices.
    /// One such example case is to copy results directly into the output's slices, which could be more performant in a very critical scenario.
    pub unsafe fn n_items_buffer_as_mut_slices(
        &self,
        num_items: usize,
    ) -> (usize, P::SliceMutIter<'_>) {
        let begin_idx = self.core.state().fetch_increment_len(num_items);
        (
            begin_idx,
            self.core.n_items_buffer_as_mut_slices(begin_idx, num_items),
        )
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

    // raw

    /// Returns:
    /// * a raw `*const T` pointer to the underlying data if element at the `index`-th position is pushed,
    /// * `None` otherwise.
    ///
    /// # Safety
    ///
    /// Pointer obtained by this method will be pointing to valid data:
    ///
    /// * `ConcurrentVec` guarantees that each position is written only and exactly once.
    /// * Furthermore, underlying `ConcurrentOption` wrapper prevents access during initialization, preventing data race.
    /// * Finally, underlying `PinnedVec` makes sure that memory location of the elements do not change.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    /// assert!(vec.raw_get(0).is_none());
    ///
    /// vec.push('a');
    /// let p = vec.raw_get(0).unwrap();
    ///
    /// vec.extend(['b', 'c', 'd', 'e']);
    ///
    /// assert_eq!(unsafe { p.as_ref() }, Some(&'a'));
    pub fn raw_get(&self, index: usize) -> Option<*const T> {
        match index < self.len() {
            true => {
                let maybe = unsafe { self.core.get(index) };
                maybe.and_then(|x| x.raw_get_with_order(Ordering::SeqCst))
            }
            false => None,
        }
    }

    /// Returns:
    /// * a raw `*mut T` pointer to the underlying data if element at the `index`-th position is pushed,
    /// * `None` otherwise.
    ///
    /// # Safety
    ///
    /// Pointer obtained by this method will be pointing to valid data:
    ///
    /// * `ConcurrentVec` guarantees that each position is written only and exactly once.
    /// * Furthermore, underlying `ConcurrentOption` wrapper prevents access during initialization, preventing data race.
    /// * Finally, underlying `PinnedVec` makes sure that memory location of the elements do not change.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    /// assert!(vec.raw_get_mut(0).is_none());
    ///
    /// vec.push('a');
    /// let p = vec.raw_get_mut(0).unwrap();
    ///
    /// vec.extend(['b', 'c', 'd', 'e']);
    ///
    /// assert_eq!(unsafe { p.as_ref() }, Some(&'a'));
    ///
    /// unsafe { p.write('x') };
    /// assert_eq!(unsafe { p.as_ref() }, Some(&'x'));
    pub fn raw_get_mut(&self, index: usize) -> Option<*mut T> {
        match index < self.len() {
            true => {
                let maybe = unsafe { self.core.get(index) };
                maybe.and_then(|x| x.raw_get_mut_with_order(Ordering::SeqCst))
            }
            false => None,
        }
    }
}

// HELPERS

impl<T, P> ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentOption<T>>,
{
    pub(crate) fn new_from_pinned(pinned_vec: P) -> Self {
        let core = PinnedConcurrentCol::new_from_pinned(pinned_vec);
        Self { core }
    }
}

unsafe impl<T: Sync, P: IntoConcurrentPinnedVec<ConcurrentOption<T>>> Sync for ConcurrentVec<T, P> {}

unsafe impl<T: Send, P: IntoConcurrentPinnedVec<ConcurrentOption<T>>> Send for ConcurrentVec<T, P> {}
