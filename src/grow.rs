use crate::{ConcurrentVec, elem::ConcurrentElement};
use core::sync::atomic::Ordering;
use orx_pinned_vec::IntoConcurrentPinnedVec;

impl<T, P> ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    /// Concurrent, thread-safe method to push the given `value` to the back of the bag, and returns the position or index of the pushed value.
    ///
    /// It preserves the order of elements with respect to the order the `push` method is called.
    ///
    /// # Panics
    ///
    /// Panics if the concurrent bag is already at its maximum capacity; i.e., if `self.len() == self.maximum_capacity()`.
    ///
    /// Note that this is an important safety assertion in the concurrent context; however, not a practical limitation.
    /// Please see the [`orx_pinned_concurrent_col::PinnedConcurrentCol::maximum_capacity`] for details.
    ///
    /// # Examples
    ///
    /// We can directly take a shared reference of the bag, share it among threads and collect results concurrently.
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let (num_threads, num_items_per_thread) = (4, 1_024);
    ///
    /// let vec = ConcurrentVec::new();
    ///
    /// std::thread::scope(|s| {
    ///     let vec = &vec;
    ///     for i in 0..num_threads {
    ///         s.spawn(move || {
    ///             for j in 0..num_items_per_thread {
    ///                 // concurrently collect results simply by calling `push`
    ///                 vec.push(i * 1000 + j);
    ///             }
    ///         });
    ///     }
    /// });
    ///
    /// let mut vec = vec.to_vec();
    /// vec.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(vec, expected);
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
    /// ### Solution: `extend` rather than `push`
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
    /// let vec = ConcurrentVec::new();
    /// let batch_size = 16;
    ///
    /// std::thread::scope(|s| {
    ///     let vec = &vec;
    ///     for i in 0..num_threads {
    ///         s.spawn(move || {
    ///             for j in (0..num_items_per_thread).step_by(batch_size) {
    ///                 let iter = (j..(j + batch_size)).map(|j| i * 1000 + j);
    ///                 // concurrently collect results simply by calling `extend`
    ///                 vec.extend(iter);
    ///             }
    ///         });
    ///     }
    /// });
    ///
    /// let mut vec = vec.to_vec();
    /// vec.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(vec, expected);
    /// ```
    pub fn push(&self, value: T) -> usize {
        let idx = self.len_reserved().fetch_add(1, Ordering::Relaxed);

        // # SAFETY: ConcurrentVec ensures that each `idx` will be written only and exactly once.
        let maybe = unsafe { self.core.single_item_as_ref(idx) };
        unsafe { maybe.0.initialize_unchecked(value) };

        idx
    }

    /// Pushes the value which will be computed as a function of the index where it will be written.
    ///
    /// Note that we cannot guarantee the index of the element by `push`ing since there might be many
    /// pushes happening concurrently. In cases where we absolutely need to know the index, in other
    /// words, when the value depends on the index, we can use `push_for_idx`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    /// vec.push(0);
    /// vec.push_for_idx(|i| i * 2);
    /// vec.push_for_idx(|i| i + 10);
    /// vec.push(42);
    ///
    /// assert_eq!(&vec, &[0, 2, 12, 42]);
    /// ```
    pub fn push_for_idx<F>(&self, f: F) -> usize
    where
        F: FnOnce(usize) -> T,
    {
        let idx = self.len_reserved().fetch_add(1, Ordering::Relaxed);
        let value = f(idx);

        // # SAFETY: ConcurrentVec ensures that each `idx` will be written only and exactly once.
        let maybe = unsafe { self.core.single_item_as_ref(idx) };
        unsafe { maybe.0.initialize_unchecked(value) };

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
    /// * If there is not sufficient space, the vector grows first; iterating over and writing elements to the vec happens afterwards.
    /// * Therefore, other threads do not wait for the `extend` method to complete, they can concurrently write.
    /// * This is a simple and effective approach to deal with the false sharing problem.
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
    /// Please see the [`orx_pinned_concurrent_col::PinnedConcurrentCol::maximum_capacity`] for details.
    ///
    /// # Examples
    ///
    /// We can directly take a shared reference of the bag and share it among threads.
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let (num_threads, num_items_per_thread) = (4, 1_024);
    ///
    /// let vec = ConcurrentVec::new();
    /// let batch_size = 16;
    ///
    /// std::thread::scope(|s| {
    ///     let vec = &vec;
    ///     for i in 0..num_threads {
    ///         s.spawn(move || {
    ///             for j in (0..num_items_per_thread).step_by(batch_size) {
    ///                 let iter = (j..(j + batch_size)).map(|j| i * 1000 + j);
    ///                 // concurrently collect results simply by calling `extend`
    ///                 vec.extend(iter);
    ///             }
    ///         });
    ///     }
    /// });
    ///
    /// let mut vec: Vec<_> = vec.to_vec();
    /// vec.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(vec, expected);
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
    /// ### Solution: `extend` rather than `push`
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
    pub fn extend<IntoIter, Iter>(&self, values: IntoIter) -> usize
    where
        IntoIter: IntoIterator<Item = T, IntoIter = Iter>,
        Iter: Iterator<Item = T> + ExactSizeIterator,
    {
        let values = values.into_iter();
        let num_items = values.len();
        self.extend_n_items::<_>(values, num_items)
    }

    /// Extends the vector with the values of the iterator which is created as a function of the
    /// index that the first element of the iterator will be written to.
    ///
    /// Note that we cannot guarantee the index of the element by `extend`ing since there might be many
    /// pushes or extends happening concurrently. In cases where we absolutely need to know the index, in other
    /// words, when the values depend on the indices, we can use `extend_for_idx`.
    ///
    /// # Panics
    ///
    /// Panics if the iterator created by `f` does not yield `num_items` elements.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    ///
    /// vec.push(0);
    ///
    /// let iter = |begin_idx: usize| ((begin_idx..(begin_idx + 3)).map(|i| i * 5));
    /// vec.extend_for_idx(|begin_idx| iter(begin_idx), 3);
    /// vec.push(42);
    ///
    /// assert_eq!(&vec, &[0, 5, 10, 15, 42]);
    /// ```
    pub fn extend_for_idx<IntoIter, Iter, F>(&self, f: F, num_items: usize) -> usize
    where
        IntoIter: IntoIterator<Item = T, IntoIter = Iter>,
        Iter: Iterator<Item = T> + ExactSizeIterator,
        F: FnOnce(usize) -> IntoIter,
    {
        let begin_idx = self.len_reserved().fetch_add(num_items, Ordering::Relaxed);
        let slices = unsafe { self.core.n_items_buffer_as_slices(begin_idx, num_items) };
        let mut values = f(begin_idx).into_iter();

        assert_eq!(values.len(), num_items);

        for slice in slices {
            for maybe in slice {
                let value = values
                    .next()
                    .expect("provided iterator is shorter than expected num_items");
                unsafe { maybe.0.initialize_unchecked(value) };
            }
        }

        begin_idx
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
    /// * Iterating over and writing elements to the vec happens afterwards.
    /// * This is a simple, effective and memory efficient solution to the false sharing problem.
    ///
    /// For this reason, the method requires the additional `num_items` argument.
    /// There exists the variant [`ConcurrentVec::extend`] method which accepts only an `ExactSizeIterator`.
    ///
    /// # Panics
    ///
    /// Panics if the iterator created by `f` does not yield `num_items` elements.
    ///
    /// # Examples
    ///
    /// We can directly take a shared reference of the bag and share it among threads.
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let (num_threads, num_items_per_thread) = (4, 1_024);
    ///
    /// let vec = ConcurrentVec::new();
    /// let batch_size = 16;
    ///
    /// std::thread::scope(|s| {
    ///     let vec = &vec;
    ///     for i in 0..num_threads {
    ///         s.spawn(move || {
    ///             for j in (0..num_items_per_thread).step_by(batch_size) {
    ///                 let iter = (j..(j + batch_size)).map(|j| i * 1000 + j);
    ///                 // concurrently collect results simply by calling `extend_n_items`
    ///                 unsafe { vec.extend_n_items(iter, batch_size) };
    ///             }
    ///         });
    ///     }
    /// });
    ///
    /// let mut vec: Vec<_> = vec.to_vec();
    /// vec.sort();
    /// let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
    /// expected.sort();
    /// assert_eq!(vec, expected);
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
    /// ### Solution: `extend` rather than `push`
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
    pub fn extend_n_items<IntoIter>(&self, values: IntoIter, num_items: usize) -> usize
    where
        IntoIter: IntoIterator<Item = T>,
    {
        let begin_idx = self.len_reserved().fetch_add(num_items, Ordering::Relaxed);
        let slices = unsafe { self.core.n_items_buffer_as_slices(begin_idx, num_items) };
        let mut values = values.into_iter();

        for slice in slices {
            for maybe in slice {
                let value = values
                    .next()
                    .expect("provided iterator is shorter than expected num_items");
                unsafe { maybe.0.initialize_unchecked(value) };
            }
        }

        begin_idx
    }
}
