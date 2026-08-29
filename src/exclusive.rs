use crate::{ConcurrentVec, elem::ConcurrentElement};
use core::sync::atomic::Ordering;
use orx_pinned_vec::IntoConcurrentPinnedVec;

impl<T, P> ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
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
