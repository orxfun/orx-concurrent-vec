use crate::ConcurrentVec;
use orx_concurrent_option::ConcurrentOption;
use orx_pinned_vec::IntoConcurrentPinnedVec;
use std::ops::{Index, IndexMut};

pub(crate) const OUT_OF_BOUNDS: &str = "index out of bounds";

impl<P, T> Index<usize> for ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentOption<T>>,
{
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect(OUT_OF_BOUNDS)
    }
}

impl<P, T> IndexMut<usize> for ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentOption<T>>,
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).expect(OUT_OF_BOUNDS)
    }
}
