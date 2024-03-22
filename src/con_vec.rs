use crate::errors::{ERR_FAILED_TO_GROW, ERR_FAILED_TO_PUSH};
use orx_fixed_vec::FixedVec;
use orx_split_vec::{Doubling, Linear, PinnedVec, Recursive, SplitVec};
use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};

/// An efficient, convenient and lightweight grow-only concurrent collection, ideal for collecting results concurrently.
///
/// The vec preserves the order of elements with respect to the order the `push` method is called.
///
/// # Examples
///
/// Safety guarantees to push to the vec with an immutable reference makes it easy to share the vec among threads.
///
/// ## Using `std::sync::Arc`
///
/// We can share our vec among threads using `Arc` and collect results concurrently.
///
/// ```rust
/// use orx_concurrent_vec::*;
/// use std::{sync::Arc, thread};
///
/// let (num_threads, num_items_per_thread) = (4, 8);
///
/// let con_vec = Arc::new(ConcurrentVec::new());
/// let mut thread_vec: Vec<thread::JoinHandle<()>> = Vec::new();
///
/// for i in 0..num_threads {
///     let con_vec = con_vec.clone();
///     thread_vec.push(thread::spawn(move || {
///         for j in 0..num_items_per_thread {
///             // concurrently collect results simply by calling `push`
///             con_vec.push(i * 1000 + j);
///         }
///     }));
/// }
///
/// for handle in thread_vec {
///     handle.join().unwrap();
/// }
///
/// let mut vec_from_con_vec: Vec<_> = con_vec.iter().copied().collect();
/// vec_from_con_vec.sort();
/// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
/// expected.sort();
/// assert_eq!(vec_from_con_vec, expected);
/// ```
///
/// ## Using `std::thread::scope`
///
/// An even more convenient approach would be to use thread scopes. This allows to use shared reference of the vec across threads, instead of `Arc`.
///
/// ```rust
/// use orx_concurrent_vec::*;
///
/// let (num_threads, num_items_per_thread) = (4, 8);
///
/// let con_vec = ConcurrentVec::new();
/// let con_vec_ref = &con_vec; // just take a reference
/// std::thread::scope(|s| {
///     for i in 0..num_threads {
///         s.spawn(move || {
///             for j in 0..num_items_per_thread {
///                 // concurrently collect results simply by calling `push`
///                 con_vec_ref.push(i * 1000 + j);
///             }
///         });
///     }
/// });
///
/// let mut vec_from_con_vec: Vec<_> = con_vec.iter().copied().collect();
/// vec_from_con_vec.sort();
/// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
/// expected.sort();
/// assert_eq!(vec_from_con_vec, expected);
/// ```
///
/// # Safety
///
/// `ConcurrentVec` uses a [`PinnedVec`](https://crates.io/crates/orx-pinned-vec) implementation as the underlying storage (see [`SplitVec`](https://crates.io/crates/orx-split-vec) and [`Fixed`](https://crates.io/crates/orx-fixed-vec)).
/// `PinnedVec` guarantees that elements which are already pushed to the vector stay pinned to their memory locations unless explicitly changed due to removals, which is not the case here since `ConcurrentVec` is a grow-only collection.
/// This feature makes it safe to grow with a shared reference on a single thread, as implemented by [`ImpVec`](https://crates.io/crates/orx-imp-vec).
///
/// In order to achieve this feature in a concurrent program, `ConcurrentVec` pairs the `PinnedVec` with an `AtomicUsize`.
/// * `len: AtomicSize`: fixes the target memory location of each element to be pushed at the time the `push` method is called. Regardless of whether or not writing to memory completes before another element is pushed, every pushed element receives a unique position reserved for it.
/// * `PinnedVec` guarantees that already pushed elements are not moved around in memory during growth. This also enables the following mode of concurrency:
///   * one thread might allocate new memory in order to grow when capacity is reached,
///   * while another thread might concurrently be writing to any of the already allocation memory locations.
///
/// The approach guarantees that
/// * only one thread can write to the memory location of an element being pushed to the vec,
/// * at any point in time, only one thread is responsible for the allocation of memory if the vec requires new memory,
/// * no thread reads any of the written elements (reading happens after converting the vec `into_inner`),
/// * hence, there exists no race condition.
///
/// # Construction
///
/// As explained above, `ConcurrentVec` is simply a tuple of a `PinnedVec` and an `AtomicUsize`.
/// Therefore, it can be constructed by wrapping any pinned vector; i.e., `ConcurrentVec<T>` implements `From<P: PinnedVec<T>>`.
/// Further, there exist `with_` methods to directly construct the concurrent vec with common pinned vector implementations.
///
/// ```rust
/// use orx_concurrent_vec::*;
///
/// // default pinned vector -> SplitVec<T, Doubling>
/// let con_vec: ConcurrentVec<char> = ConcurrentVec::new();
/// let con_vec: ConcurrentVec<char> = Default::default();
/// let con_vec: ConcurrentVec<char> = ConcurrentVec::with_doubling_growth();
/// let con_vec: ConcurrentVec<char, SplitVec<Option<char>, Doubling>> = ConcurrentVec::with_doubling_growth();
///
/// let con_vec: ConcurrentVec<char> = SplitVec::new().into();
/// let con_vec: ConcurrentVec<char, SplitVec<Option<char>, Doubling>> = SplitVec::new().into();
///
/// // SplitVec with [Recursive](https://docs.rs/orx-split-vec/latest/orx_split_vec/struct.Recursive.html) growth
/// let con_vec: ConcurrentVec<char, SplitVec<Option<char>, Recursive>> =
///     ConcurrentVec::with_recursive_growth();
/// let con_vec: ConcurrentVec<char, SplitVec<Option<char>, Recursive>> =
///     SplitVec::with_recursive_growth().into();
///
/// // SplitVec with [Linear](https://docs.rs/orx-split-vec/latest/orx_split_vec/struct.Linear.html) growth
/// // each fragment will have capacity 2^10 = 1024
/// let con_vec: ConcurrentVec<char, SplitVec<Option<char>, Linear>> = ConcurrentVec::with_linear_growth(10);
/// let con_vec: ConcurrentVec<char, SplitVec<Option<char>, Linear>> = SplitVec::with_linear_growth(10).into();
///
/// // [FixedVec](https://docs.rs/orx-fixed-vec/latest/orx_fixed_vec/) with fixed capacity.
/// // Fixed vector cannot grow; hence, pushing the 1025-th element to this vec will cause a panic!
/// let con_vec: ConcurrentVec<char, FixedVec<Option<char>>> = ConcurrentVec::with_fixed_capacity(1024);
/// let con_vec: ConcurrentVec<char, FixedVec<Option<char>>> = FixedVec::new(1024).into();
/// ```
///
/// Of course, the pinned vector to be wrapped does not need to be empty.
///
/// ```rust
/// use orx_concurrent_vec::*;
///
/// let split_vec: SplitVec<Option<i32>> = (0..1024).map(Some).collect();
/// let con_vec: ConcurrentVec<_> = split_vec.into();
/// ```
///
/// # Write-Only vs Read-Write
///
/// The concurrent vec is read and write & grow-only vector which is convenient and efficient for collecting elements.
/// It guarantees that threads can only read elements which are already written and can never change.
/// This flexibility has the minor additional cost of values being wrapped by an `Option`.
///
/// See [`ConcurrentBag`](https://crates.io/crates/orx-concurrent-bag) for a read-only variant which stores values directly as `T`.
pub struct ConcurrentVec<T, P = SplitVec<Option<T>, Doubling>>
where
    P: PinnedVec<Option<T>>,
{
    pinned: UnsafeCell<P>,
    len: AtomicUsize,
    capacity: AtomicUsize,
    phantom: PhantomData<T>,
}

// new
impl<T> ConcurrentVec<T, SplitVec<Option<T>, Doubling>> {
    /// Creates a new concurrent vector by creating and wrapping up a new `SplitVec<T, Doubling>` as the underlying storage.
    pub fn with_doubling_growth() -> Self {
        Self::new_from_pinned(SplitVec::with_doubling_growth())
    }

    /// Creates a new concurrent vector by creating and wrapping up a new `SplitVec<T, Doubling>` as the underlying storage.
    pub fn new() -> Self {
        Self::new_from_pinned(SplitVec::new())
    }
}

impl<T> Default for ConcurrentVec<T, SplitVec<Option<T>, Doubling>> {
    /// Creates a new concurrent vector by creating and wrapping up a new `SplitVec<T, Doubling>` as the underlying storage.
    fn default() -> Self {
        Self::with_doubling_growth()
    }
}

impl<T> ConcurrentVec<T, SplitVec<Option<T>, Recursive>> {
    /// Creates a new concurrent vector by creating and wrapping up a new `SplitVec<T, Recursive>` as the underlying storage.
    pub fn with_recursive_growth() -> Self {
        Self::new_from_pinned(SplitVec::with_recursive_growth())
    }
}

impl<T> ConcurrentVec<T, SplitVec<Option<T>, Linear>> {
    /// Creates a new concurrent vector by creating and wrapping up a new `SplitVec<T, Linear>` as the underlying storage.
    ///
    /// Note that choosing a small `constant_fragment_capacity_exponent` for a large vec to be filled might lead to too many growth calls which might be computationally costly.
    pub fn with_linear_growth(constant_fragment_capacity_exponent: usize) -> Self {
        Self::new_from_pinned(SplitVec::with_linear_growth(
            constant_fragment_capacity_exponent,
        ))
    }
}

impl<T> ConcurrentVec<T, FixedVec<Option<T>>> {
    /// Creates a new concurrent vector by creating and wrapping up a new `FixedVec<T>` as the underlying storage.
    ///
    /// # Safety
    ///
    /// Note that a `FixedVec` cannot grow.
    /// Therefore, pushing the `(fixed_capacity + 1)`-th element to the vec will lead to a panic.
    pub fn with_fixed_capacity(fixed_capacity: usize) -> Self {
        Self::new_from_pinned(FixedVec::new(fixed_capacity))
    }
}

impl<T, P> From<P> for ConcurrentVec<T, P>
where
    P: PinnedVec<Option<T>>,
{
    fn from(value: P) -> Self {
        Self::new_from_pinned(value)
    }
}

// impl
impl<T, P> ConcurrentVec<T, P>
where
    P: PinnedVec<Option<T>>,
{
    /// Consumes the concurrent vector and returns the underlying pinned vector.
    ///
    /// Note that
    /// * it is cheap to wrap a `SplitVec` as a `ConcurrentVec` using thee `From` trait;
    /// * and similarly to convert a `ConcurrentVec` to the underlying `SplitVec` using `into_inner` method.
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
    /// assert_eq!(vec![Some('a'), Some('b'), Some('c'), Some('d')], split.iter().copied().collect::<Vec<_>>());
    ///
    /// split.push(Some('e'));
    /// *split.get_mut(0).expect("exists") = Some('x');
    ///
    /// assert_eq!(vec![Some('x'), Some('b'), Some('c'), Some('d'), Some('e')], split.iter().copied().collect::<Vec<_>>());
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
        let (len, mut pinned) = (self.len(), self.pinned.into_inner());
        unsafe { pinned.set_len(len) };
        pinned
    }

    /// ***O(1)*** Returns the number of elements which are pushed to the vector, including the elements which received their reserved locations and are currently being pushed.
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
        self.len.load(Ordering::SeqCst)
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
        self.capacity.load(Ordering::Relaxed)
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
    /// let mut iter = convec.iter();
    /// assert_eq!(iter.next(), Some(&'a'));
    /// assert_eq!(iter.next(), Some(&'b'));
    /// assert_eq!(iter.next(), None);
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        unsafe {
            self.correct_pinned_len();
            let pinned = &*self.pinned.get() as &P;
            pinned.iter().take(self.len()).flatten()
        }
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
        if index < self.len() && index < self.capacity() {
            unsafe {
                self.correct_pinned_len();
                let pinned = &*self.pinned.get() as &P;
                pinned.get(index).and_then(|x| x.as_ref())
            }
        } else {
            None
        }
    }

    /// Concurrent & thread-safe method to push the given `value` to the back of the vec.
    ///
    /// It preserves the order of elements with respect to the order the `push` method is called.
    ///
    /// # Examples
    ///
    /// Allowing to safely push to the vec with an immutable reference, it is trivial to share the vec among threads.
    ///
    /// ## Using `std::sync::Arc`
    ///
    /// We can share our vec among threads using `Arc` and collect results concurrently.
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    /// use std::{sync::Arc, thread};
    ///
    /// let (num_threads, num_items_per_thread) = (4, 8);
    ///
    /// let con_vec = Arc::new(ConcurrentVec::new());
    /// let mut thread_vec: Vec<thread::JoinHandle<()>> = Vec::new();
    ///
    /// for i in 0..num_threads {
    ///     let con_vec = con_vec.clone();
    ///     thread_vec.push(thread::spawn(move || {
    ///         for j in 0..num_items_per_thread {
    ///             // concurrently collect results simply by calling `push`
    ///             con_vec.push(i * 1000 + j);
    ///         }
    ///     }));
    /// }
    ///
    /// for handle in thread_vec {
    ///     handle.join().unwrap();
    /// }
    ///
    /// let mut vec_from_con_vec: Vec<_> = con_vec.iter().copied().collect();
    /// vec_from_con_vec.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(vec_from_con_vec, expected);
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
    /// let con_vec = ConcurrentVec::new();
    /// let con_vec_ref = &con_vec; // just take a reference
    /// std::thread::scope(|s| {
    ///     for i in 0..num_threads {
    ///         s.spawn(move || {
    ///             for j in 0..num_items_per_thread {
    ///                 // concurrently collect results simply by calling `push`
    ///                 con_vec_ref.push(i * 1000 + j);
    ///             }
    ///         });
    ///     }
    /// });
    ///
    /// let mut vec_from_con_vec: Vec<_> = con_vec.iter().copied().collect();
    /// vec_from_con_vec.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(vec_from_con_vec, expected);
    /// ```
    ///
    /// # Safety
    ///
    /// `ConcurrentVec` uses a [`PinnedVec`](https://crates.io/crates/orx-pinned-vec) implementation as the underlying storage (see [`SplitVec`](https://crates.io/crates/orx-split-vec) and [`Fixed`](https://crates.io/crates/orx-fixed-vec)).
    /// `PinnedVec` guarantees that elements which are already pushed to the vector stay pinned to their memory locations unless explicitly changed due to removals, which is not the case here since `ConcurrentVec` is a grow-only collection.
    /// This feature makes it safe to grow with a shared reference on a single thread, as implemented by [`ImpVec`](https://crates.io/crates/orx-imp-vec).
    ///
    /// In order to achieve this feature in a concurrent program, `ConcurrentVec` pairs the `PinnedVec` with an `AtomicUsize`.
    /// * `len: AtomicSize`: fixes the target memory location of each element to be pushed at the time the `push` method is called. Regardless of whether or not writing to memory completes before another element is pushed, every pushed element receives a unique position reserved for it.
    /// * `PinnedVec` guarantees that already pushed elements are not moved around in memory during growth. This also enables the following mode of concurrency:
    ///   * one thread might allocate new memory in order to grow when capacity is reached,
    ///   * while another thread might concurrently be writing to any of the already allocation memory locations.
    ///
    /// The approach guarantees that
    /// * only one thread can write to the memory location of an element being pushed to the vec,
    /// * at any point in time, only one thread is responsible for the allocation of memory if the vec requires new memory,
    /// * no thread reads any of the written elements (reading happens after converting the vec `into_inner`),
    /// * hence, there exists no race condition.
    ///
    /// # Panics
    ///
    /// Panics if the underlying pinned vector fails to grow.
    /// * Note that `FixedVec` cannot grow beyond its fixed capacity;
    /// * `SplitVec`, on the other hand, can grow without dynamically.
    pub fn push(&self, value: T) {
        let idx = self.len.fetch_add(1, Ordering::SeqCst);

        loop {
            let capacity = self.capacity.load(Ordering::SeqCst);

            match idx.cmp(&capacity) {
                std::cmp::Ordering::Less => {
                    // no need to grow, just push
                    let pinned = unsafe { &mut *self.pinned.get() };
                    let ptr = unsafe { pinned.get_ptr_mut(idx) }.expect(ERR_FAILED_TO_PUSH);
                    unsafe { *ptr = Some(value) };
                    break;
                }
                std::cmp::Ordering::Equal => {
                    // we are responsible for growth

                    let pinned = unsafe { &mut *self.pinned.get() };

                    let new_capacity =
                        unsafe { pinned.grow_to(capacity + 1) }.expect(ERR_FAILED_TO_GROW);

                    let ptr = unsafe { pinned.get_ptr_mut(idx) }.expect(ERR_FAILED_TO_PUSH);
                    unsafe { *ptr = Some(value) };

                    self.capacity.store(new_capacity, Ordering::SeqCst);

                    break;
                }

                std::cmp::Ordering::Greater => { /* wait for thread responsible for growth */ }
            }
        }
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
    /// let mut con_vec = ConcurrentVec::new();
    ///
    /// con_vec.push('a');
    /// con_vec.push('b');
    ///
    /// con_vec.clear();
    /// assert!(con_vec.is_empty());
    /// ```
    pub fn clear(&mut self) {
        let pinned = self.pinned.get_mut();
        pinned.clear();
        let capacity = pinned.capacity();

        self.capacity.store(capacity, Ordering::SeqCst);
        self.len.store(0, Ordering::SeqCst);
    }

    // helpers
    fn new_from_pinned(pinned: P) -> Self {
        Self {
            len: pinned.len().into(),
            capacity: pinned.capacity().into(),
            pinned: pinned.into(),
            phantom: Default::default(),
        }
    }

    pub(crate) unsafe fn correct_pinned_len(&self) {
        let pinned = unsafe { &mut *self.pinned.get() };
        unsafe { pinned.set_len(self.len()) };
    }
}

unsafe impl<T, P: PinnedVec<Option<T>>> Sync for ConcurrentVec<T, P> {}

unsafe impl<T, P: PinnedVec<Option<T>>> Send for ConcurrentVec<T, P> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_len_empty_clear() {
        fn test<P: PinnedVec<Option<char>>>(con_vec: ConcurrentVec<char, P>) {
            let mut con_vec = con_vec;

            assert!(con_vec.is_empty());
            assert_eq!(0, con_vec.len());
            assert_eq!(0, con_vec.len_exact());

            con_vec.push('a');

            assert!(!con_vec.is_empty());
            assert_eq!(1, con_vec.len());
            assert_eq!(1, con_vec.len_exact());

            con_vec.push('b');
            con_vec.push('c');
            con_vec.push('d');

            assert!(!con_vec.is_empty());
            assert_eq!(4, con_vec.len());
            assert_eq!(4, con_vec.len_exact());

            con_vec.clear();
            assert!(con_vec.is_empty());
            assert_eq!(0, con_vec.len());
            assert_eq!(0, con_vec.len_exact());

            con_vec.push('a');
            con_vec.push('b');

            let mut pinned = con_vec.into_inner();
            *pinned.get_mut(0).expect("is-some") = None;

            let con_vec: ConcurrentVec<_, _> = pinned.into();
            assert_eq!(2, con_vec.len());
            assert_eq!(1, con_vec.len_exact());
        }

        test(ConcurrentVec::new());
        test(ConcurrentVec::default());
        test(ConcurrentVec::with_doubling_growth());
        test(ConcurrentVec::with_recursive_growth());
        test(ConcurrentVec::with_linear_growth(2));
        test(ConcurrentVec::with_linear_growth(4));
        test(ConcurrentVec::with_fixed_capacity(64));
    }

    #[test]
    fn capacity() {
        let split: SplitVec<_, Doubling> = (0..4).map(Some).collect();
        let bag: ConcurrentVec<_, _> = split.into();
        assert_eq!(bag.capacity(), 4);
        bag.push(42);
        assert_eq!(bag.capacity(), 12);

        let split: SplitVec<_, Recursive> = (0..4).map(Some).collect();
        let bag: ConcurrentVec<_, _> = split.into();
        assert_eq!(bag.capacity(), 4);
        bag.push(42);
        assert_eq!(bag.capacity(), 12);

        let mut split: SplitVec<_, Linear> = SplitVec::with_linear_growth(2);
        split.extend_from_slice(&[0, 1, 2, 3].map(Some));
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
            "ConcurrentVec { pinned: ['a', 'b', 'c', 'd', 'e'], len: 5, capacity: 12 }"
        );
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
}
