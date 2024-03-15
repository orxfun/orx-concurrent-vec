use orx_split_vec::prelude::PinnedVec;
use orx_split_vec::{Doubling, Fragment, GrowthWithConstantTimeAccess, Linear, SplitVec};
use std::{cmp::Ordering, fmt::Debug, sync::atomic::AtomicUsize};

const ORDERING: core::sync::atomic::Ordering = core::sync::atomic::Ordering::Relaxed;

/// An efficient and convenient thread-safe grow-only read-and-write collection, ideal for collecting results concurrently.
/// * **convenient**: the vector can be shared among threads simply as a shared reference, not even requiring `Arc`,
/// * **efficient**: for collecting results concurrently:
///   * rayon is significantly faster than `ConcurrentVec` when the elements are small and there is an extreme load (no work at all among push calls),
///   * `ConcurrentVec` is significantly faster than rayon when elements are large or there there is some computation happening to evaluate the elements before the push calls,
///   * you may see the details of the benchmarks at [benches/grow.rs](https://github.com/orxfun/orx-concurrent-bag/blob/main/benches/grow.rs).
///
/// The bag preserves the order of elements with respect to the order the `push` method is called.
///
/// # Examples
///
/// Safety guarantees to push to the bag with an immutable reference makes it easy to share the bag among threads.
///
/// ## Using `std::sync::Arc`
///
/// Following the common approach of using an `Arc`, we can share the vector among threads and collect results concurrently.
///
/// ```rust
/// use orx_concurrent_vec::*;
/// use std::{sync::Arc, thread};
///
/// let (num_threads, num_items_per_thread) = (4, 8);
///
/// let convec = Arc::new(ConcurrentVec::new());
/// let mut thread_vec: Vec<thread::JoinHandle<()>> = Vec::new();
///
/// for i in 0..num_threads {
///     let convec = convec.clone();
///     thread_vec.push(thread::spawn(move || {
///         for j in 0..num_items_per_thread {
///             convec.push(i * 1000 + j); // concurrently collect results simply by calling `push`
///         }
///     }));
/// }
///
/// for handle in thread_vec {
///     handle.join().unwrap();
/// }
///
/// let mut vec_from_convec: Vec<_> = convec.iter().copied().collect();
/// vec_from_convec.sort();
/// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
/// expected.sort();
/// assert_eq!(vec_from_convec, expected);
/// ```
///
/// ## Using `std::thread::scope`
///
/// An even more convenient approach would be to use thread scopes. This allows to use shared reference of the vec across threads, instead of `Arc`.
///
/// ```rust
/// use orx_concurrent_vec::*;
/// use std::thread;
///
/// let (num_threads, num_items_per_thread) = (4, 8);
///
/// let convec = ConcurrentVec::new();
/// let convec_ref = &convec; // just take a reference
/// std::thread::scope(|s| {
///     for i in 0..num_threads {
///         s.spawn(move || {
///             for j in 0..num_items_per_thread {
///                 convec_ref.push(i * 1000 + j); // concurrently collect results simply by calling `push`
///             }
///         });
///     }
/// });
///
/// let mut vec_from_convec: Vec<_> = convec.iter().copied().collect();
/// vec_from_convec.sort();
/// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
/// expected.sort();
/// assert_eq!(vec_from_convec, expected);
/// ```
///
/// # Safety
///
/// `ConcurrentVec` uses a [`SplitVec`](https://crates.io/crates/orx-split-vec) as the underlying storage.
/// `SplitVec` implements [`PinnedVec`](https://crates.io/crates/orx-pinned-vec) which guarantees that elements which are already pushed to the vector stay pinned to their memory locations.
/// This feature makes it safe to grow with a shared reference on a single thread, as implemented by [`ImpVec`](https://crates.io/crates/orx-imp-vec).
///
/// In order to achieve this feature in a concurrent program, `ConcurrentVec` pairs the `SplitVec` with an `AtomicUsize`.
/// * `AtomicUsize` fixes the target memory location of each element to be pushed at the time the `push` method is called. Regardless of whether or not writing to memory completes before another element is pushed, every pushed element receives a unique position reserved for it.
/// * `SplitVec` guarantees that already pushed elements are not moved around in memory and new elements are written to the reserved position.
///
/// The approach guarantees that
/// * only one thread can write to the memory location of an element being pushed to the vec,
/// * at any point in time, only one thread is responsible for the allocation of memory if the vec requires new memory,
/// * no thread reads an element which is being written, reading is allowed only after the element is completely written,
/// * hence, there exists no race condition.
///
/// This pair allows a lightweight and convenient concurrent bag which is ideal for collecting results concurrently.
///
/// # Write-Only vs Read-Write
///
/// The concurrent vec is read-and-write & grow-only vec which is convenient and efficient for collecting elements.
/// While allowing growth by pushing elements from multiple threads, each thread can safely read already pushed elements.
///
/// See [`ConcurrentBag`](https://crates.io/crates/orx-concurrent-bag) for a write-only variant which allows only writing during growth.
/// The advantage of the bag, on the other hand, is that it stores elements as `T` rather than `Option<T>`.
#[derive(Debug)]
pub struct ConcurrentVec<T, G = Doubling>
where
    G: GrowthWithConstantTimeAccess,
{
    split: SplitVec<Option<T>, G>,
    len: AtomicUsize,
}

unsafe impl<T, G: GrowthWithConstantTimeAccess> Sync for ConcurrentVec<T, G> {}

unsafe impl<T, G: GrowthWithConstantTimeAccess> Send for ConcurrentVec<T, G> {}

impl<T> ConcurrentVec<T, Doubling> {
    /// Creates a new empty concurrent vec.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let convec = ConcurrentVec::new();
    /// convec.push('a');
    /// convec.push('b');
    ///
    /// assert_eq!(vec!['a', 'b'], convec.iter().copied().collect::<Vec<_>>());
    /// ```
    pub fn new() -> Self {
        Self::with_doubling_growth()
    }

    /// Creates a new empty concurrent vec with doubling growth strategy.
    ///
    /// Each fragment of the underlying split vector will have a capacity which is double the capacity of the prior fragment.
    ///
    /// More information about doubling strategy can be found here [`orx_split_vec::Doubling`](https://docs.rs/orx-split-vec/latest/orx_split_vec/struct.Doubling.html).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// // fragments will have capacities 4, 8, 16, etc.
    /// let convec = ConcurrentVec::with_doubling_growth();
    /// convec.push('a');
    /// convec.push('b');
    ///
    /// assert_eq!(vec!['a', 'b'], convec.iter().copied().collect::<Vec<_>>());
    /// ```
    pub fn with_doubling_growth() -> Self {
        let mut vec = SplitVec::new();
        let first_fragment = unsafe { vec.fragments_mut().get_unchecked_mut(0) };
        Self::init_fragment(first_fragment);
        Self {
            split: vec,
            len: AtomicUsize::new(0),
        }
    }
}

impl<T> Default for ConcurrentVec<T, Doubling> {
    /// Creates a new empty concurrent vec.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let convec = ConcurrentVec::default();
    /// convec.push('a');
    /// convec.push('b');
    ///
    /// assert_eq!(vec!['a', 'b'], convec.iter().copied().collect::<Vec<_>>());
    /// ```
    fn default() -> Self {
        Self::new()
    }
}

impl<T> ConcurrentVec<T, Linear> {
    /// Creates a new empty concurrent vec with linear growth strategy.
    ///
    /// Each fragment of the underlying split vector will have a capacity of `2 ^ constant_fragment_capacity_exponent`.
    ///
    /// More information about doubling strategy can be found here [`orx_split_vec::Linear`](https://docs.rs/orx-split-vec/latest/orx_split_vec/struct.Linear.html).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// // each fragment will have a capacity of 2^5 = 32
    /// let convec = ConcurrentVec::with_linear_growth(5);
    /// convec.push('a');
    /// convec.push('b');
    ///
    /// assert_eq!(vec!['a', 'b'], convec.iter().copied().collect::<Vec<_>>());
    /// ```
    pub fn with_linear_growth(constant_fragment_capacity_exponent: usize) -> Self {
        let mut vec = SplitVec::with_linear_growth(constant_fragment_capacity_exponent);
        let first_fragment = unsafe { vec.fragments_mut().get_unchecked_mut(0) };
        Self::init_fragment(first_fragment);
        Self {
            split: vec,
            len: AtomicUsize::new(0),
        }
    }
}

impl<T, G: GrowthWithConstantTimeAccess> From<SplitVec<Option<T>, G>> for ConcurrentVec<T, G> {
    fn from(split: SplitVec<Option<T>, G>) -> Self {
        let len = AtomicUsize::new(split.len());
        Self { split, len }
    }
}

impl<T, G: GrowthWithConstantTimeAccess> ConcurrentVec<T, G> {
    /// Consumes the concurrent vec and returns the inner storage, the `SplitVec`.
    ///
    /// Note that
    /// * it is cheap to wrap a `SplitVec` as a `ConcurrentVec` using thee `From` trait;
    /// * and similarly to convert a `ConcurrentVec` to the underlying `SplitVec` using `into_inner` method.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::prelude::*;
    ///
    /// let convec = ConcurrentVec::new();
    ///
    /// convec.push('a');
    /// convec.push('b');
    /// convec.push('c');
    /// convec.push('d');
    /// assert_eq!(vec!['a', 'b', 'c', 'd'], convec.iter().copied().collect::<Vec<_>>());
    ///
    /// let mut split = convec.into_inner();
    /// assert_eq!(vec![Some('a'), Some('b'), Some('c'), Some('d')], split.iter().copied().collect::<Vec<_>>());
    ///
    /// split.push(Some('e'));
    /// *split.get_mut(0).expect("exists") = Some('x');
    ///
    /// assert_eq!(vec![Some('x'), Some('b'), Some('c'), Some('d'), Some('e')], split.iter().copied().collect::<Vec<_>>());
    ///
    /// let mut convec: ConcurrentVec<_> = split.into();
    /// assert_eq!(vec!['x', 'b', 'c', 'd', 'e'], convec.iter().copied().collect::<Vec<_>>());
    ///
    /// convec.clear();
    /// assert!(convec.is_empty());
    ///
    /// let split = convec.into_inner();
    /// assert!(split.is_empty());
    pub fn into_inner(self) -> SplitVec<Option<T>, G> {
        let (len, mut split) = (self.len(), self.split);
        Self::correct_split_lengths(&mut split, len);
        split
    }

    /// ***O(1)*** Returns the number of elements which are pushed to the vector, including the elements which received their reserved locations and currently being pushed.
    ///
    /// In order to get number of elements which are completely pushed to the vector, excluding elements that are currently being pushed, you may use
    /// `convec.len_exact()`, or equivalently, `convec.iter().count()`; however, with ***O(n)*** time complexity.
    ///
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
    /// assert_eq!(2, convec.len());
    /// ```
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len.load(ORDERING)
        // unsafe { self.len.as_ptr().read() }
    }

    /// ***O(n)*** Returns the number of elements which are completely pushed to the vector, excluding elements which received their reserved locations and currently being pushed.
    ///
    /// In order to get number of elements for which the `push` method is called, including elements that are currently being pushed, you may use
    /// `convec.len()` with ***O(1)*** time complexity.
    ///
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
    /// assert_eq!(2, convec.len_exact());
    /// assert_eq!(2, convec.iter().count());
    /// ```
    #[inline(always)]
    pub fn len_exact(&self) -> usize {
        self.iter().count()
    }

    /// Returns whether the vector is empty (`len() == 0`) or not.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::ConcurrentVec;
    ///
    /// let mut convec = ConcurrentVec::new();
    ///
    /// assert!(convec.is_empty());
    ///
    /// convec.push('a');
    /// convec.push('b');
    /// assert!(!convec.is_empty());
    ///
    /// convec.clear();
    /// assert!(convec.is_empty());
    /// ```
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
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
    /// let mut iter = unsafe { convec.iter() };
    /// assert_eq!(iter.next(), Some(&'a'));
    /// assert_eq!(iter.next(), Some(&'b'));
    /// assert_eq!(iter.next(), None);
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.split.iter().take(self.len()).flatten()
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
    /// let convec = ConcurrentVec::new();
    /// convec.push('a');
    /// convec.push('b');
    ///
    /// assert_eq!(convec.get(0), Some(&'a'));
    /// assert_eq!(convec.get(1), Some(&'b'));
    /// assert_eq!(convec.get(2), None);
    /// ```
    #[allow(clippy::missing_panics_doc, clippy::unwrap_in_result)]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.len() && index < self.split.capacity() {
            let (f, i) = self
                .split
                .growth
                .get_fragment_and_inner_indices_unchecked(index);
            let fragment = self.split.fragments().get(f).expect("exists");
            unsafe { fragment.get_unchecked(i) }.as_ref()
        } else {
            None
        }
    }

    /// Concurrent & thread-safe method to push the given `value` to the back of the vector.
    ///
    /// It preserves the order of elements with respect to the order the `push` method is called.
    ///
    /// # Examples
    ///
    /// Allowing to safely push to the vector with an immutable reference, it is trivial to share the vec among threads.
    ///
    /// ## Using `std::sync::Arc`
    ///
    /// Following the common approach of using an `Arc`, we can share the vec among threads and collect results concurrently.
    ///
    /// ```rust
    /// use orx_concurrent_vec::prelude::*;
    /// use std::{sync::Arc, thread};
    ///
    /// let (num_threads, num_items_per_thread) = (4, 8);
    ///
    /// let convec = Arc::new(ConcurrentVec::new());
    /// let mut thread_vec: Vec<thread::JoinHandle<()>> = Vec::new();
    ///
    /// for i in 0..num_threads {
    ///     let convec = convec.clone();
    ///     thread_vec.push(thread::spawn(move || {
    ///         for j in 0..num_items_per_thread {
    ///             convec.push(i * 1000 + j); // concurrently collect results simply by calling `push`
    ///         }
    ///     }));
    /// }
    ///
    /// for handle in thread_vec {
    ///     handle.join().unwrap();
    /// }
    ///
    /// let mut vec_from_convec: Vec<_> = convec.iter().copied().collect();
    /// vec_from_convec.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(vec_from_convec, expected);
    /// ```
    ///
    /// ## Using `std::thread::scope`
    ///
    /// An even more convenient approach would be to use thread scopes.
    /// This allows to use shared reference to the vec directly, instead of `Arc`.
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    /// use std::thread;
    ///
    /// let (num_threads, num_items_per_thread) = (4, 8);
    ///
    /// let convec = ConcurrentVec::new();
    /// let convec_ref = &convec; // just take a reference
    /// std::thread::scope(|s| {
    ///     for i in 0..num_threads {
    ///         s.spawn(move || {
    ///             for j in 0..num_items_per_thread {
    ///                 convec_ref.push(i * 1000 + j); // concurrently collect results simply by calling `push`
    ///             }
    ///         });
    ///     }
    /// });
    ///
    /// let mut vec_from_convec: Vec<_> = convec.iter().copied().collect();
    /// vec_from_convec.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(vec_from_convec, expected);
    /// ```
    ///
    /// # Safety
    ///
    /// `ConcurrentVec` uses a [`SplitVec`](https://crates.io/crates/orx-split-vec) as the underlying storage.
    /// `SplitVec` implements [`PinnedVec`](https://crates.io/crates/orx-pinned-vec) which guarantees that elements which are already pushed to the vector stay pinned to their memory locations.
    /// This feature makes it safe to grow with a shared reference on a single thread, as implemented by [`ImpVec`](https://crates.io/crates/orx-imp-vec).
    ///
    /// In order to achieve this feature in a concurrent program, `ConcurrentVec` pairs the `SplitVec` with an `AtomicUsize`.
    /// * `AtomicUsize` fixes the target memory location of each element being pushed at the point the `push` method is called.
    /// Regardless of whether or not writing to memory completes before another element is pushed, every pushed element receives a unique position reserved for it.
    /// * `SplitVec` guarantees that already pushed elements are not moved around in memory and new elements are written to the reserved position.
    ///
    /// This pair allows a lightweight and convenient concurrent vec which is ideal for collecting results concurrently.
    pub fn push(&self, value: T) {
        self.push_optional(Some(value));
    }

    /// Concurrent & thread-safe method to push `None` (no value) to the back of the vector.
    ///
    /// This method is useful to reserve an element, or push a hole, which can later be written/filled by the `put` method.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::prelude::*;
    ///
    /// let (num_threads, num_items_per_thread) = (4, 8);
    ///
    /// let convec = ConcurrentVec::new();
    /// let convec_ref = &convec; // just take a reference
    /// std::thread::scope(|s| {
    ///     for i in 0..3 {
    ///         s.spawn(move || {
    ///             if i == 1 {
    ///                 convec_ref.push_none();
    ///             } else {
    ///                 convec_ref.push(42);
    ///             }
    ///         });
    ///     }
    /// });
    ///
    /// let mut vec_from_convec: Vec<_> = convec.into_inner().iter().copied().collect();
    /// vec_from_convec.sort();
    /// let mut expected = vec![None, Some(42), Some(42)];
    /// assert_eq!(vec_from_convec, expected);
    /// ```
    pub fn push_none(&self) {
        self.push_optional(None);
    }

    /// Clears the vec removing all already pushed elements.
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
    /// let mut vec = ConcurrentVec::new();
    ///
    /// vec.push('a');
    /// vec.push('b');
    ///
    /// vec.clear();
    /// assert!(vec.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.len.store(0, ORDERING);
        self.split.clear();
    }

    // unsafe
    /// Takes the `index`-th element out of the vector and returns it; leaves `None` at its place.
    ///
    /// # Safety
    ///
    /// `ConcurrentVec::take` does not use a lock or mutex.
    /// In other words, caller is responsible to prevent race conditions.
    #[allow(clippy::unwrap_in_result, clippy::missing_panics_doc)]
    pub unsafe fn take(&self, index: usize) -> Option<T> {
        if index < self.len() && index < self.split.capacity() {
            let split = std::hint::black_box(unsafe { into_mut(&self.split) });
            let (f, i) = split.growth.get_fragment_and_inner_indices_unchecked(index);
            let fragment = split.fragments_mut().get_mut(f).expect("exists");
            unsafe { fragment.get_unchecked_mut(i) }.take()
        } else {
            None
        }
    }

    /// Puts the `value` at `index`-th position of the vector.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// # Safety
    ///
    /// `ConcurrentVec::put` does not use a lock or mutex.
    /// In other words, caller is responsible to prevent race conditions.
    pub unsafe fn put(&self, index: usize, value: T) {
        let split = std::hint::black_box(unsafe { into_mut(&self.split) });
        let (f, i) = split.growth.get_fragment_and_inner_indices_unchecked(index);
        let fragment = split.fragments_mut().get_mut(f).expect("exists");
        *unsafe { fragment.get_unchecked_mut(i) } = Some(value);
    }

    // helpers
    fn init_fragment(fragment: &mut Fragment<Option<T>>) {
        debug_assert_eq!(0, fragment.len());

        let len = fragment.capacity();
        unsafe { fragment.set_len(len) }

        for x in fragment.iter_mut() {
            *x = None;
        }
    }

    fn correct_split_lengths(split: &mut SplitVec<Option<T>, G>, len: usize) {
        let mut remaining = len;

        let fragments = unsafe { split.fragments_mut() };

        for fragment in fragments {
            let capacity = fragment.capacity();
            if remaining <= capacity {
                unsafe { fragment.set_len(remaining) };
            } else {
                unsafe { fragment.set_len(capacity) };
                remaining -= capacity;
            }
        }

        unsafe { split.set_len(len) };
    }

    fn push_optional(&self, value: Option<T>) {
        let idx = self.len.fetch_add(1, ORDERING);

        loop {
            let capacity = self.split.capacity();

            match idx.cmp(&capacity) {
                Ordering::Less => {
                    let split = std::hint::black_box(unsafe { into_mut(&self.split) });
                    if let Some(ptr) = unsafe { split.ptr_mut(idx) } {
                        unsafe { *ptr = value };
                        break;
                    }
                }
                Ordering::Equal => {
                    let split = unsafe { into_mut(&self.split) };
                    let next_capacity = split.growth.new_fragment_capacity(split.fragments());
                    let mut fragment = Vec::with_capacity(next_capacity).into();
                    Self::init_fragment(&mut fragment);
                    fragment[0] = value;
                    let fragments = unsafe { split.fragments_mut() };
                    fragments.push(fragment);
                    break;
                }
                Ordering::Greater => {}
            }
        }
    }
}

#[allow(invalid_reference_casting)]
unsafe fn into_mut<'a, T>(reference: &T) -> &'a mut T {
    &mut *(reference as *const T as *mut T)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_len_empty_clear() {
        fn test<G: GrowthWithConstantTimeAccess>(convec: ConcurrentVec<char, G>) {
            let mut convec = convec;

            assert!(convec.is_empty());
            assert_eq!(0, convec.len());

            convec.push('a');

            assert!(!convec.is_empty());
            assert_eq!(1, convec.len());

            convec.push('b');
            convec.push('c');
            convec.push('d');

            assert!(!convec.is_empty());
            assert_eq!(4, convec.len());

            convec.clear();
            assert!(convec.is_empty());
            assert_eq!(0, convec.len());
        }

        test(ConcurrentVec::new());
        test(ConcurrentVec::default());
        test(ConcurrentVec::with_doubling_growth());
        test(ConcurrentVec::with_linear_growth(2));
        test(ConcurrentVec::with_linear_growth(4));
    }

    #[test]
    fn debug() {
        let convec = ConcurrentVec::new();

        convec.push('a');
        convec.push('b');
        convec.push('c');
        convec.push('d');

        let str = format!("{:?}", convec);
        assert_eq!(
            str,
            "ConcurrentVec { split: SplitVec [\n    [Some('a'), Some('b'), Some('c'), Some('d')]\n]\n, len: 4 }"
        );
    }

    #[test]
    fn iter() {
        let mut convec = ConcurrentVec::new();

        assert_eq!(0, convec.iter().count());

        convec.push('a');

        assert_eq!(vec!['a'], convec.iter().copied().collect::<Vec<_>>());

        convec.push('b');
        convec.push('c');
        convec.push('d');

        assert_eq!(
            vec!['a', 'b', 'c', 'd'],
            convec.iter().copied().collect::<Vec<_>>()
        );

        convec.clear();
        assert_eq!(0, convec.iter().count());
    }

    #[test]
    fn get() {
        let mut convec = ConcurrentVec::new();

        assert_eq!(convec.get(0), None);

        convec.push('a');

        assert_eq!(convec.get(0), Some(&'a'));

        convec.push('b');
        convec.push('c');
        convec.push('d');

        assert_eq!(convec.get(0), Some(&'a'));
        assert_eq!(convec.get(1), Some(&'b'));
        assert_eq!(convec.get(2), Some(&'c'));
        assert_eq!(convec.get(3), Some(&'d'));

        convec.clear();
        assert_eq!(convec.get(0), None);
    }

    #[test]
    fn into_inner_from() {
        let convec = ConcurrentVec::new();

        convec.push('a');
        convec.push('b');
        convec.push('c');
        convec.push('d');
        assert_eq!(
            vec!['a', 'b', 'c', 'd'],
            convec.iter().copied().collect::<Vec<_>>()
        );

        let mut split = convec.into_inner();
        assert_eq!(
            vec![Some('a'), Some('b'), Some('c'), Some('d')],
            split.iter().copied().collect::<Vec<_>>()
        );

        split.push(Some('e'));
        *split.get_mut(0).expect("exists") = Some('x');

        assert_eq!(
            vec![Some('x'), Some('b'), Some('c'), Some('d'), Some('e')],
            split.iter().copied().collect::<Vec<_>>()
        );

        let mut convec: ConcurrentVec<_> = split.into();
        assert_eq!(
            vec!['x', 'b', 'c', 'd', 'e'],
            convec.iter().copied().collect::<Vec<_>>()
        );

        convec.clear();
        assert!(convec.is_empty());

        let split = convec.into_inner();
        assert!(split.is_empty());
    }
}
