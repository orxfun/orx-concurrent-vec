use crate::{elem::ConcurrentElement, ConcurrentSlice};
use orx_pinned_vec::IntoConcurrentPinnedVec;

impl<'a, T, P> ConcurrentSlice<'a, T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    /// Returns an iterator to values obtained by mapping elements of the vec by `f`.
    ///
    /// Note that `vec.map(f)` is a shorthand for `vec.iter().map(move |elem| elem.map(|x: &T| f(x)))`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter(0..4);
    ///
    /// let doubles: Vec<_> = vec.map(|x| x * 2).collect();
    /// assert_eq!(doubles, [0, 2, 4, 6]);
    /// ```
    pub fn map<F, U>(&'a self, mut f: F) -> impl Iterator<Item = U> + 'a
    where
        F: FnMut(&T) -> U + 'a,
    {
        self.iter().map(move |elem| elem.map(|x: &T| f(x)))
    }

    /// Returns an iterator to elements filtered by using the predicate `f` on the values.
    ///
    /// Note that `vec.filter(f)` is a shorthand for `vec.iter().filter(move |elem| elem.map(|x: &T| f(x)))`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter(0..4);
    ///
    /// let mut evens = vec.filter(|x| x % 2 == 0);
    /// assert_eq!(evens.next().unwrap(), &0);
    /// assert_eq!(evens.next().unwrap(), &2);
    /// assert_eq!(evens.next(), None);
    /// ```
    pub fn filter<F>(&self, mut f: F) -> impl Iterator<Item = &ConcurrentElement<T>> + '_
    where
        F: FnMut(&T) -> bool + 'a,
    {
        self.iter().filter(move |elem| elem.map(|x: &T| f(x)))
    }

    /// Folds the values of the vec starting from the `init` using the fold function `f`.
    ///
    /// Note that `vec.fold(f)` is a shorthand for `vec.iter().fold(init, |agg, elem| elem.map(|x| f(agg, x)))`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter(0..4);
    ///
    /// let sum = vec.fold(0, |sum, x| sum + x);
    /// assert_eq!(sum, 6);
    /// ```
    pub fn fold<F, U>(&self, init: U, mut f: F) -> U
    where
        F: FnMut(U, &T) -> U,
    {
        self.iter().fold(init, |agg, elem| elem.map(|x| f(agg, x)))
    }

    /// Reduces the values of the slice using the reduction `f`; returns None if the vec is empty.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    /// let sum = vec.reduce(|a, b| a + b);
    /// assert_eq!(sum, None);
    ///
    /// vec.push(42);
    /// let sum = vec.reduce(|a, b| a + b);
    /// assert_eq!(sum, Some(42));
    ///
    /// vec.extend([6, 2]);
    /// let sum = vec.reduce(|a, b| a + b);
    /// assert_eq!(sum, Some(50));
    /// ```
    pub fn reduce<F>(&self, mut f: F) -> Option<T>
    where
        T: Clone,
        F: FnMut(&T, &T) -> T,
    {
        let mut iter = self.iter();
        match iter.next() {
            Some(first) => {
                let mut agg = first.cloned();
                for elem in iter {
                    agg = elem.map(|x| f(&agg, x));
                }
                Some(agg)
            }
            None => None,
        }
    }
}
