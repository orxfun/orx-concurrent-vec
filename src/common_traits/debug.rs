use crate::{ConcurrentElement, ConcurrentSlice, ConcurrentVec};
use core::fmt::Debug;
use orx_fixed_vec::IntoConcurrentPinnedVec;

impl<T: Debug> Debug for ConcurrentElement<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.map(|x| write!(f, "{:?}", x))
    }
}

impl<T, P> Debug for ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
    T: Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        fmt_elems_iter(f, self.iter())
    }
}

impl<'a, T, P> Debug for ConcurrentSlice<'a, T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
    T: Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        fmt_elems_iter(f, self.iter())
    }
}

// helper

fn fmt_elems_iter<'a, T, I>(f: &mut core::fmt::Formatter<'_>, mut iter: I) -> core::fmt::Result
where
    T: Debug + 'a,
    I: Iterator<Item = &'a ConcurrentElement<T>>,
{
    write!(f, "[")?;
    if let Some(first) = iter.next() {
        first.map(|x| write!(f, "{:?}", x))?;
        for elem in iter {
            elem.map(|x| write!(f, ", {:?}", x))?;
        }
    }
    write!(f, "]")
}
