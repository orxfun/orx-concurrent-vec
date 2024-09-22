use crate::{ConcurrentElement, ConcurrentVec};
use core::marker::PhantomData;
use orx_fixed_vec::IntoConcurrentPinnedVec;

impl<T, P> IntoIterator for ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    type Item = T;

    type IntoIter = ElementValuesIter<T, P>;

    fn into_iter(self) -> Self::IntoIter {
        let x = self.into_inner();
        let y: P::IntoIter = x.into_iter();
        ElementValuesIter {
            iter: y,
            phantom: PhantomData,
        }
    }
}

/// A consuming iterator yielding values of the vector.
///
/// It can be created by calling `into_iter` on the ConcurrentVec.
///
/// # Examples
///
/// ```
/// use orx_concurrent_vec::*;
///
/// let vec = ConcurrentVec::new();
/// vec.extend(['a', 'b', 'c']);
///
/// let mut iter = vec.into_iter();
/// assert_eq!(iter.next(), Some('a'));
/// assert_eq!(iter.next(), Some('b'));
/// assert_eq!(iter.next(), Some('c'));
/// assert_eq!(iter.next(), None);
/// ```
pub struct ElementValuesIter<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    iter: P::IntoIter,
    phantom: PhantomData<T>,
}

impl<T, P> Iterator for ElementValuesIter<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|e| e.0.unwrap())
    }
}
