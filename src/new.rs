use crate::vec::ConcurrentVec;
use orx_fixed_vec::FixedVec;
use orx_pinned_vec::IntoConcurrentPinnedVec;
use orx_split_vec::{Doubling, Linear, SplitVec};

impl<T> Default for ConcurrentVec<T, SplitVec<T, Doubling>> {
    /// Creates a new concurrent bag by creating and wrapping up a new [`SplitVec<T, Doubling>`](https://docs.rs/orx-split-vec/latest/orx_split_vec/struct.Doubling.html) as the underlying storage.
    fn default() -> Self {
        Self::with_doubling_growth()
    }
}

impl<T> ConcurrentVec<T, SplitVec<T, Doubling>> {
    /// Creates a new concurrent bag by creating and wrapping up a new [`SplitVec<T, Doubling>`](https://docs.rs/orx-split-vec/latest/orx_split_vec/struct.Doubling.html) as the underlying storage.
    pub fn new() -> Self {
        Self::with_doubling_growth()
    }

    /// Creates a new concurrent bag by creating and wrapping up a new [`SplitVec<T, Doubling>`](https://docs.rs/orx-split-vec/latest/orx_split_vec/struct.Doubling.html) as the underlying storage.
    pub fn with_doubling_growth() -> Self {
        Self::new_from_pinned(SplitVec::with_doubling_growth_and_fragments_capacity(32))
    }
}

impl<T> ConcurrentVec<T, SplitVec<T, Linear>> {
    /// Creates a new concurrent bag by creating and wrapping up a new [`SplitVec<T, Linear>`](https://docs.rs/orx-split-vec/latest/orx_split_vec/struct.Linear.html) as the underlying storage.
    ///
    /// * Each fragment of the split vector will have a capacity of  `2 ^ constant_fragment_capacity_exponent`.
    /// * Further, fragments collection of the split vector will have a capacity of `fragments_capacity` on initialization.
    ///
    /// This leads to a [`orx_pinned_concurrent_col::PinnedConcurrentCol::maximum_capacity`] of `fragments_capacity * 2 ^ constant_fragment_capacity_exponent`.
    ///
    /// Whenever this capacity is not sufficient, fragments capacity can be increased by using the  [`orx_pinned_concurrent_col::PinnedConcurrentCol::reserve_maximum_capacity`] method.
    pub fn with_linear_growth(
        constant_fragment_capacity_exponent: usize,
        fragments_capacity: usize,
    ) -> Self {
        Self::new_from_pinned(SplitVec::with_linear_growth_and_fragments_capacity(
            constant_fragment_capacity_exponent,
            fragments_capacity,
        ))
    }
}

impl<T> ConcurrentVec<T, FixedVec<T>> {
    /// Creates a new concurrent bag by creating and wrapping up a new [`FixedVec<T>`](https://docs.rs/orx-fixed-vec/latest/orx_fixed_vec/) as the underlying storage.
    ///
    /// # Safety
    ///
    /// Note that a `FixedVec` cannot grow; i.e., it has a hard upper bound on the number of elements it can hold, which is the `fixed_capacity`.
    ///
    /// Pushing to the vector beyond this capacity leads to "out-of-capacity" error.
    ///
    /// This maximum capacity can be accessed by [`orx_pinned_concurrent_col::PinnedConcurrentCol::capacity`] or [`orx_pinned_concurrent_col::PinnedConcurrentCol::maximum_capacity`] methods.
    pub fn with_fixed_capacity(fixed_capacity: usize) -> Self {
        Self::new_from_pinned(FixedVec::new(fixed_capacity))
    }
}

// from
impl<T, P> From<P> for ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<T>,
{
    /// `ConcurrentVec<T>` uses any `PinnedVec<T>` implementation as the underlying storage.
    ///
    /// Therefore, without a cost
    /// * `ConcurrentVec<T>` can be constructed from any `PinnedVec<T>`, and
    /// * the underlying `PinnedVec<T>` can be obtained by `ConcurrentVec::into_inner(self)` method.
    fn from(pinned_vec: P) -> Self {
        Self::new_from_pinned(pinned_vec)
    }
}
