use crate::{ConcurrentElem, ConcurrentSlice, ConcurrentVec};
use core::fmt::Debug;
use orx_fixed_vec::IntoConcurrentPinnedVec;

impl<T: Debug> Debug for ConcurrentElem<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.map(|x| write!(f, "{:?}", x))
    }
}

impl<T, P> Debug for ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElem<T>>,
    T: Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        fmt_elems_iter(f, self.iter())
    }
}

impl<'a, T, P> Debug for ConcurrentSlice<'a, T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElem<T>>,
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt_elems_iter(f, self.iter())
    }
}

// helper

fn fmt_elems_iter<'a, T, I>(f: &mut core::fmt::Formatter<'_>, mut iter: I) -> core::fmt::Result
where
    T: Debug + 'a,
    I: Iterator<Item = &'a ConcurrentElem<T>>,
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
