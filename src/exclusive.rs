use crate::{ConcurrentVec, elem::ConcurrentElement};
use alloc::vec::Vec;
use core::sync::atomic::Ordering;
use orx_pinned_vec::IntoConcurrentPinnedVec;

impl<T, P> ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    /// Sorts the vector in ascending order.
    ///
    /// This is a stable sort.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let mut vec = ConcurrentVec::from_iter([3, 1, 2]);
    /// vec.sort();
    ///
    /// assert_eq!(&vec, &[1, 2, 3]);
    /// ```
    pub fn sort(&mut self)
    where
        T: Ord,
    {
        self.sort_by(T::cmp);
    }

    /// Sorts the vector with a comparator function.
    ///
    /// This is a stable sort.
    ///
    /// The comparator is called before the elements are reordered, so a panic in the
    /// comparator leaves the vector unchanged.
    ///
    /// # Panics
    ///
    /// Panics if the comparator panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let mut vec = ConcurrentVec::from_iter([1, 3, 2]);
    /// vec.sort_by(|a, b| b.cmp(a));
    ///
    /// assert_eq!(&vec, &[3, 2, 1]);
    /// ```
    pub fn sort_by<F>(&mut self, mut compare: F)
    where
        F: FnMut(&T, &T) -> core::cmp::Ordering,
    {
        let len = self.len();
        let mut order = Vec::with_capacity(len);
        order.extend(0..len);

        order.sort_by(|&a, &b| {
            // SAFETY: every index in `order` is in 0..len, whose elements are initialized;
            // &mut self excludes concurrent access for the duration of this operation.
            let left = unsafe { self.core.get(a) }.expect("sort element must exist");
            if a == b {
                return left
                    .0
                    .map(|value| compare(value, value))
                    .expect("sort element must have a value");
            }
            // SAFETY: `b` is also in 0..len and all such elements are initialized.
            let right = unsafe { self.core.get(b) }.expect("sort element must exist");

            left.0
                .map(|left| {
                    right
                        .0
                        .map(|right| compare(left, right))
                        .expect("sort element must have a value")
                })
                .expect("sort element must have a value")
        });

        let visited = usize::MAX;
        for start in 0..len {
            if order[start] == visited {
                continue;
            }

            let mut current = start;
            loop {
                let source = order[current];
                order[current] = visited;
                if source == start {
                    break;
                }

                // SAFETY: `current` and `source` are distinct indices in 0..len, so both
                // pointers refer to initialized elements. Exclusive access prevents concurrent use.
                unsafe {
                    let current_ptr = self.core.ptr_mut(current);
                    let source_ptr = self.core.ptr_mut(source);
                    core::ptr::swap(current_ptr, source_ptr);
                }
                current = source;
            }
        }
    }

    /// Sorts the vector with a key extraction function.
    ///
    /// This is a stable sort.
    ///
    /// # Panics
    ///
    /// Panics if the key extraction function panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let mut vec = ConcurrentVec::from_iter([(2, 'a'), (1, 'b'), (2, 'c')]);
    /// vec.sort_by_key(|item| item.0);
    ///
    /// assert_eq!(&vec, &[(1, 'b'), (2, 'a'), (2, 'c')]);
    /// ```
    pub fn sort_by_key<K, F>(&mut self, mut f: F)
    where
        K: Ord,
        F: FnMut(&T) -> K,
    {
        self.sort_by(|a, b| f(a).cmp(&f(b)));
    }

    /// Inserts an element at position `index`, shifting all elements after it to the right.
    ///
    /// # Panics
    ///
    /// Panics if `index > self.len()`, or if the vector cannot grow to accommodate the new element.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let mut vec = ConcurrentVec::from_iter(['a', 'c']);
    /// vec.insert(1, 'b');
    /// vec.insert(3, 'd');
    ///
    /// assert_eq!(&vec, &['a', 'b', 'c', 'd']);
    /// ```
    pub fn insert(&mut self, index: usize, element: T) {
        let len = self.len();
        assert!(index <= len, "insertion index out of bounds");
        let new_len = len.checked_add(1).expect("capacity overflow");

        // SAFETY: all positions below `len` are initialized and `&mut self` excludes
        // concurrent access. The new position is filled before the method returns.
        let _ = unsafe { self.core.single_item_as_ref(len) };

        for i in (index..len).rev() {
            // SAFETY: each source and destination is within the allocated capacity and
            // initialized as a ConcurrentElement; exclusive access prevents concurrent use.
            let value = unsafe { self.core.get_mut(i) }
                .expect("insert source must exist")
                .0
                .exclusive_take()
                .expect("insert source must contain a value");
            unsafe { self.core.get_mut(i + 1) }
                .expect("insert destination must exist")
                .0
                .set_some(value);
        }

        // SAFETY: index is at most len, and the slot is empty after shifting.
        unsafe { self.core.get_mut(index) }
            .expect("insert destination must exist")
            .0
            .set_some(element);

        self.len_written().store(new_len, Ordering::Relaxed);
        self.len_reserved().store(new_len, Ordering::Relaxed);
    }

    /// Removes and returns the element at position `index`, shifting all elements after it to the left.
    ///
    /// # Panics
    ///
    /// Panics if `index >= self.len()`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let mut vec = ConcurrentVec::from_iter(['a', 'b', 'c']);
    ///
    /// assert_eq!(vec.remove(1), 'b');
    /// assert_eq!(&vec, &['a', 'c']);
    /// ```
    pub fn remove(&mut self, index: usize) -> T {
        let len = self.len();
        assert!(index < len, "removal index out of bounds");

        // SAFETY: index is within the initialized range and we have exclusive access.
        let value = unsafe { self.core.get_mut(index) }
            .expect("remove position must exist")
            .0
            .exclusive_take()
            .expect("remove position must contain a value");

        for i in index..len - 1 {
            // SAFETY: source and destination are within the initialized range, and
            // exclusive access prevents concurrent use.
            let next = unsafe { self.core.get_mut(i + 1) }
                .expect("remove source must exist")
                .0
                .exclusive_take()
                .expect("remove source must contain a value");
            unsafe { self.core.get_mut(i) }
                .expect("remove destination must exist")
                .0
                .set_some(next);
        }

        let new_len = len - 1;
        self.len_written().store(new_len, Ordering::Relaxed);
        self.len_reserved().store(new_len, Ordering::Relaxed);

        value
    }

    /// Clears the concurrent bag.
    pub fn clear(&mut self) {
        unsafe { self.core.clear(self.core.state().len()) };
    }

    /// Removes the last element from the vector and returns it, or `None` if it is empty.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let mut vec = ConcurrentVec::new();
    /// vec.push('a');
    /// vec.push('b');
    ///
    /// assert_eq!(vec.pop(), Some('b'));
    /// assert_eq!(vec.pop(), Some('a'));
    /// assert_eq!(vec.pop(), None);
    /// ```
    pub fn pop(&mut self) -> Option<T> {
        let len = self.len();
        match len {
            0 => None,
            n => {
                let last_idx = n - 1;
                // SAFETY: the element exists and we have &mut reference to vec
                let elem = unsafe { self.core.get_mut(last_idx) }?;
                let value = elem.0.exclusive_take();

                self.len_written().store(last_idx, Ordering::Relaxed);
                self.len_reserved().store(last_idx, Ordering::Relaxed);

                value
            }
        }
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
}
