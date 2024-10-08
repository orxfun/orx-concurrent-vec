#![doc = include_str!("../README.md")]
#![warn(
    missing_docs,
    clippy::unwrap_in_result,
    clippy::unwrap_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::float_cmp,
    clippy::float_cmp_const,
    clippy::missing_panics_doc,
    clippy::todo
)]
#![no_std]

extern crate alloc;

mod common_traits;
/// Concurrent counterpart of a slice.
mod concurrent_slice;
/// A concurrent element providing thread safe access to elements of the concurrent vector or slice.
mod elem;
/// Methods requiring `&mut self` reference.
mod exclusive;
/// Methods adding elements.
mod grow;
mod helpers;
/// Methods providing shorthands for reducing verbosity of iterators.
mod iter_shorthands;
/// Methods that mutate existing elements.
mod mut_elem;
mod new;
mod partial_eq;
mod split;
mod state;
mod to_vec;
/// Unsafe methods providing direct access to elements.
mod unsafe_api;
mod vec;

pub use concurrent_slice::ConcurrentSlice;
pub use elem::ConcurrentElement;
pub use orx_fixed_vec::FixedVec;
pub use orx_pinned_vec::{
    ConcurrentPinnedVec, IntoConcurrentPinnedVec, PinnedVec, PinnedVecGrowthError,
};
pub use orx_split_vec::{Doubling, Linear, SplitVec};
pub use vec::ConcurrentVec;
