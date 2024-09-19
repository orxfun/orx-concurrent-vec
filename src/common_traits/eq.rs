use crate::{ConcurrentElem, ConcurrentSlice, ConcurrentVec};

// elem
impl<T: PartialEq> PartialEq for ConcurrentElem<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: PartialEq> PartialEq<T> for ConcurrentElem<T> {
    fn eq(&self, other: &T) -> bool {
        self.map(|x| x == other)
    }
}

// vec

impl<T: PartialEq> PartialEq for ConcurrentVec<T> {
    fn eq(&self, other: &Self) -> bool {
        eq_elem_iters(self.iter(), other.iter())
    }
}

impl<'a, T: PartialEq> PartialEq<ConcurrentSlice<'a, T>> for ConcurrentVec<T> {
    fn eq(&self, other: &ConcurrentSlice<'a, T>) -> bool {
        eq_elem_iters(self.iter(), other.iter())
    }
}

impl<T: PartialEq> PartialEq<[T]> for ConcurrentVec<T> {
    fn eq(&self, other: &[T]) -> bool {
        eq_elem_iter_to_iter(self.iter(), other.iter())
    }
}

impl<const N: usize, T: PartialEq> PartialEq<[T; N]> for ConcurrentVec<T> {
    fn eq(&self, other: &[T; N]) -> bool {
        eq_elem_iter_to_iter(self.iter(), other.iter())
    }
}

// slice

impl<'a, T: PartialEq> PartialEq for ConcurrentSlice<'a, T> {
    fn eq(&self, other: &Self) -> bool {
        eq_elem_iters(self.iter(), other.iter())
    }
}

impl<'a, T: PartialEq> PartialEq<ConcurrentVec<T>> for ConcurrentSlice<'a, T> {
    fn eq(&self, other: &ConcurrentVec<T>) -> bool {
        eq_elem_iters(self.iter(), other.iter())
    }
}

impl<'a, T: PartialEq> PartialEq<[T]> for ConcurrentSlice<'a, T> {
    fn eq(&self, other: &[T]) -> bool {
        eq_elem_iter_to_iter(self.iter(), other.iter())
    }
}

impl<'a, const N: usize, T: PartialEq> PartialEq<[T; N]> for ConcurrentSlice<'a, T> {
    fn eq(&self, other: &[T; N]) -> bool {
        eq_elem_iter_to_iter(self.iter(), other.iter())
    }
}

// helpers

fn eq_elem_iters<'a, T, I, J>(mut a: I, mut b: J) -> bool
where
    I: Iterator<Item = &'a ConcurrentElem<T>>,
    J: Iterator<Item = &'a ConcurrentElem<T>>,
    T: PartialEq + 'a,
{
    loop {
        match (a.next(), b.next()) {
            (Some(a), Some(b)) if a == b => continue,
            (None, None) => return true,
            _ => return false,
        }
    }
}

fn eq_elem_iter_to_iter<'a, T, I, J>(mut a: I, mut b: J) -> bool
where
    I: Iterator<Item = &'a ConcurrentElem<T>>,
    J: Iterator<Item = &'a T>,
    T: PartialEq + 'a,
{
    loop {
        match (a.next(), b.next()) {
            (Some(a), Some(b)) if a.0.is_some_and(|a| a == b) => continue,
            (None, None) => return true,
            _ => return false,
        }
    }
}
