use super::ConcurrentSlice;
use crate::ConcurrentElem;
use core::sync::atomic::Ordering;
use orx_pinned_vec::IntoConcurrentPinnedVec;

impl<'a, T, P> ConcurrentSlice<'a, T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElem<T>>,
{
    /// Returns:
    /// * a raw `*const T` pointer to the underlying data if element at the `i`-th position is pushed,
    /// * `None` otherwise.
    ///
    /// # Safety
    ///
    /// Please see below the safety guarantees and potential safety risks using the pointer obtained by this method.
    ///
    /// ## Safety Guarantees
    ///
    /// Pointer obtained by this method will be valid:
    ///
    /// * `ConcurrentVec` prevents access to elements which are not added yet.
    /// * `ConcurrentOption` wrapper prevents access during initialization, and hence, prevents data race during initialization.
    /// * `PinnedVec` storage makes sure that memory location of the elements never change.
    ///
    /// Therefore, the caller can hold on the obtained pointer throughout the lifetime of the vec.
    /// It is guaranteed that it will be valid pointing to the correct position with initialized data.
    ///
    /// ## Unsafe Bits
    ///
    /// However, this method still leaks out a pointer, using which can cause data races as follows:
    /// * The value of the position can be `replace`d or `set` or `update`d concurrently by another thread.
    /// * If at the same instant, we attempt to read using this pointer, we would end up with a data-race.
    ///
    /// ## Safe Usage
    ///
    /// This method can be safely used as long as the caller is able to guarantee that the position will not be being mutated
    /// while using the pointer to directly access the data.
    ///
    /// A common use case to this is the grow-only scenarios where added elements are not mutated:
    /// * elements can be added to the vector by multiple threads,
    /// * while already pushed elements can safely be accessed by other threads using `get_raw`.
    ///
    /// See [`get`] for an example safe use case.
    ///
    /// [`get`]: crate::ConcurrentVec::get
    pub fn get_raw(&self, i: usize) -> Option<*const T> {
        self.idx(i).and_then(|i| self.vec.get_raw(i))
    }

    /// Returns a reference to the element at the `i`-th position of the vec.
    /// It returns `None` if index is out of bounds.
    ///
    /// See also [`get`] and [`get_cloned`] for thread-safe alternatives of concurrent access to data.
    ///
    /// [`get`]: crate::ConcurrentVec::get
    /// [`get_cloned`]: crate::ConcurrentVec::get_cloned
    ///
    /// # Safety
    ///
    /// All methods that leak out `&T` or `&mut T` references are marked as unsafe.
    /// Please see the reason and possible scenarios to use it safely below.
    ///
    /// ## Safety Guarantees
    ///
    /// Reference obtained by this method will be valid:
    ///
    /// * `ConcurrentVec` prevents access to elements which are not added yet.
    /// * `ConcurrentOption` wrapper prevents access during initialization, and hence, prevents data race during initialization.
    /// * `PinnedVec` storage makes sure that memory location of the elements never change.
    ///
    /// Therefore, the caller can hold on the obtained reference throughout the lifetime of the vec.
    /// It is guaranteed that the reference will be valid pointing to the correct position.
    ///
    /// ## Unsafe Bits
    ///
    /// However, this method still leaks out a reference, which can cause data races as follows:
    /// * The value of the position can be `replace`d or `set` or `update`d concurrently by another thread.
    /// * If at the same instant, we attempt to read using this reference, we would end up with a data-race.
    ///
    /// ## Safe Usage
    ///
    /// This method can be safely used as long as the caller is able to guarantee that the position will not be being mutated
    /// while using the reference to directly access the data.
    ///
    /// A common use case to this is the grow-only scenarios where added elements are not mutated:
    /// * elements can be added to the vector by multiple threads,
    /// * while already pushed elements can safely be accessed by other threads using `get`.
    ///
    /// # Examples
    ///
    /// As explained above, the following constructs a safe usage example of the unsafe get method.
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
    ///             // since pushed elements are not being mutated
    ///             let len = rf_measurements.len();
    ///             let mut metric = Metric::default();
    ///             for i in 0..len {
    ///                 if let Some(value) = unsafe { rf_measurements.get_ref(i) } {
    ///                     metric = metric.aggregate(value);
    ///                 }
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
    /// let measurements = measurements.to_vec();
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
    pub unsafe fn get_ref(&self, i: usize) -> Option<&T> {
        self.idx(i).and_then(|i| self.vec.get_ref(i))
    }

    /// Returns an iterator to references of elements of the vec.
    ///
    /// See also [`iter`] and [`iter_cloned`] for thread-safe alternatives of concurrent access to elements.
    ///
    /// [`iter`]: crate::ConcurrentVec::iter
    /// [`iter_cloned`]: crate::ConcurrentVec::iter_cloned
    ///
    /// # Safety
    ///
    /// All methods that leak out `&T` or `&mut T` references are marked as unsafe.
    /// Please see the reason and possible scenarios to use it safely below.
    ///
    /// ## Safety Guarantees
    ///
    /// References obtained by this method will be valid:
    ///
    /// * `ConcurrentVec` prevents access to elements which are not added yet.
    /// * `ConcurrentOption` wrapper prevents access during initialization, and hence, prevents data race during initialization.
    /// * `PinnedVec` storage makes sure that memory location of the elements never change.
    ///
    /// Therefore, the caller can hold on the obtained references throughout the lifetime of the vec.
    /// It is guaranteed that the references will be valid pointing to the correct positions.
    ///
    /// ## Unsafe Bits
    ///
    /// However, this method still leaks out references that can cause data races as follows:
    /// * Values of elements in the vector can be concurrently mutated by methods such as `replace` or `update` by other threads.
    /// * If at the same instant, we attempt to read using these references, we would end up with a data-race.
    ///
    /// ## Safe Usage
    ///
    /// This method can be safely used as long as the caller is able to guarantee that the position will not be being mutated
    /// while using these references to directly access the data.
    ///
    /// A common use case to this is the grow-only scenarios where added elements are not mutated:
    /// * elements can be added to the vector by multiple threads,
    /// * while already pushed elements can safely be accessed by other threads using `iter`.
    ///
    /// # Examples
    ///
    /// As explained above, the following constructs a safe usage example of the unsafe iter method.
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
    ///             // since pushed elements are never mutated
    ///             let metric = unsafe {
    ///                 rf_measurements
    ///                     .iter_ref()
    ///                     .fold(Metric::default(), |x, value| x.aggregate(value))
    ///             };
    ///
    ///             // push result to metrics bag
    ///             rf_metrics.push(metric);
    ///
    ///             std::thread::sleep(Duration::from_millis(100));
    ///         }
    ///     });
    /// });
    ///
    /// let measurements: Vec<_> = measurements.to_vec();
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
    pub unsafe fn iter_ref(&self) -> impl Iterator<Item = &T> {
        // TODO: this must be iter-from to jump directly to the 'a'-th element!
        let x = self
            .vec
            .core
            .iter(self.vec.len())
            .skip(self.a)
            .take(self.len);
        x.flat_map(|x| unsafe { x.0.as_ref_with_order(Ordering::SeqCst) })
    }

    // mut

    /// Returns:
    /// * a raw `*mut T` pointer to the underlying data if element at the `i`-th position is pushed,
    /// * `None` otherwise.
    ///
    /// # Safety
    ///
    /// Please see below the safety guarantees and potential safety risks using the pointer obtained by this method.
    ///
    /// ## Safety Guarantees
    ///
    /// Pointer obtained by this method will be valid:
    ///
    /// * `ConcurrentVec` prevents access to elements which are not added yet.
    /// * `ConcurrentOption` wrapper prevents access during initialization, and hence, prevents data race during initialization.
    /// * `PinnedVec` storage makes sure that memory location of the elements never change.
    ///
    /// Therefore, the caller can hold on the obtained pointer throughout the lifetime of the vec.
    /// It is guaranteed that it will be valid pointing to the correct position with initialized data.
    ///
    /// ## Unsafe Bits
    ///
    /// However, this method still leaks out a pointer, using which can cause data races as follows:
    /// * The value of the position can be `replace`d or `set` or `update`d concurrently by another thread.
    /// * If at the same instant, we attempt to read using this pointer, we would end up with a data-race.
    ///
    /// ## Safe Usage
    ///
    /// This method can be safely used as long as the caller is able to guarantee that the position will not be being
    /// read or written by another thread while using the pointer to directly access the data.
    pub fn get_raw_mut(&self, i: usize) -> Option<*mut T> {
        self.idx(i).and_then(|i| self.vec.get_raw_mut(i))
    }

    /// Returns a mutable reference to the element at the `i`-th position of the vec.
    /// It returns `None` if index is out of bounds.
    ///
    /// See also [`get`] and [`swap`] for thread-safe alternatives of concurrent mutation of elements.
    ///
    /// [`get`]: crate::ConcurrentVec::get
    /// [`swap`]: crate::ConcurrentVec::swap
    ///
    /// # Safety
    ///
    /// All methods that leak out `&T` or `&mut T` references are marked as unsafe.
    /// Please see the reason and possible scenarios to use it safely below.
    ///
    /// ## Safety Guarantees
    ///
    /// Reference obtained by this method will be valid:
    ///
    /// * `ConcurrentVec` prevents access to elements which are not added yet.
    /// * `ConcurrentOption` wrapper prevents access during initialization, and hence, prevents data race during initialization.
    /// * `PinnedVec` storage makes sure that memory location of the elements never change.
    ///
    /// Therefore, the caller can hold on the obtained reference throughout the lifetime of the vec.
    /// It is guaranteed that the reference will be valid pointing to the correct position.
    ///
    /// ## Unsafe Bits
    ///
    /// However, this method still leaks out a reference, which can cause data races as follows:
    /// * The value of the position can be `replace`d or `set` or `update`d concurrently by another thread.
    /// * And it maybe read by safe access methods such as `use_element` or `cloned`.
    /// * If at the same instant, we attempt to read or write using this reference, we would end up with a data-race.
    ///
    /// ## Safe Usage
    ///
    /// This method can be safely used as long as the caller is able to guarantee that the position will not be being
    /// read or written by another thread while using the reference to directly access the data.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    /// vec.extend(['a', 'b', 'c', 'd']);
    ///
    /// assert_eq!(unsafe { vec.get_mut(4) }, None);
    ///
    /// *unsafe { vec.get_mut(1).unwrap() } = 'x';
    /// assert_eq!(unsafe { vec.get_ref(1) }, Some(&'x'));
    ///
    /// assert_eq!(&vec, &['a', 'x', 'c', 'd']);
    /// ```
    pub unsafe fn get_mut(&self, i: usize) -> Option<&mut T> {
        self.idx(i).and_then(|i| self.vec.get_mut(i))
    }

    /// Returns an iterator to mutable references of elements of the vec.
    ///
    /// See also [`iter`], [`fill`] and [`fill_with`] for thread-safe alternatives of concurrent mutation of elements.
    ///
    /// [`iter`]: crate::ConcurrentVec::iter
    /// [`fill`]: crate::ConcurrentVec::fill
    /// [`fill_with`]: crate::ConcurrentVec::fill_with
    ///
    /// # Safety
    ///
    /// All methods that leak out `&T` or `&mut T` references are marked as unsafe.
    /// Please see the reason and possible scenarios to use it safely below.
    ///
    /// ## Safety Guarantees
    ///
    /// References obtained by this method will be valid:
    ///
    /// * `ConcurrentVec` prevents access to elements which are not added yet.
    /// * `ConcurrentOption` wrapper prevents access during initialization, and hence, prevents data race during initialization.
    /// * `PinnedVec` storage makes sure that memory location of the elements never change.
    ///
    /// Therefore, the caller can hold on the obtained references throughout the lifetime of the vec.
    /// It is guaranteed that the references will be valid pointing to the correct position.
    ///
    /// ## Unsafe Bits
    ///
    /// However, this method still leaks out references, which can cause data races as follows:
    /// * Values of elements can be concurrently read by other threads.
    /// * Likewise, they can be concurrently mutated by thread-safe mutation methods.
    /// * If at the same instant, we attempt to read or write using these references, we would end up with a data-race.
    ///
    /// ## Safe Usage
    ///
    /// This method can be safely used as long as the caller is able to guarantee that the elements will not be being
    /// read or written by another thread while using the reference to directly access the data.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter([0, 1, 2, 3]);
    ///
    /// let iter = unsafe { vec.iter_mut() };
    /// for x in iter {
    ///     *x *= 2;
    /// }
    ///
    /// assert_eq!(&vec, &[0, 2, 4, 6]);
    /// ```
    pub unsafe fn iter_mut(&self) -> impl Iterator<Item = &mut T> {
        let x = self
            .vec
            .core
            .iter(self.vec.len())
            .skip(self.a)
            .take(self.len);
        x.flat_map(|x| x.0.get_raw_mut().map(|p| &mut *p))
    }
}
