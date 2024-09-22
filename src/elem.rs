use orx_concurrent_option::ConcurrentOption;

/// An element of the `ConcurrentVec` that provides thread safe
/// read and write methods on the value of the element.
///
/// A concurrent element can be created by using the index operator `vec[i]`
/// or calling [`vec.get(i)`] or [`vec.iter()`] on a concurrent vec or slice.
///
/// [`vec.get(i)`]: crate::ConcurrentVec::get
/// [`vec.iter()`]: crate::ConcurrentVec::iter
pub struct ConcurrentElement<T>(pub(crate) ConcurrentOption<T>);

impl<T> From<ConcurrentOption<T>> for ConcurrentElement<T> {
    fn from(value: ConcurrentOption<T>) -> Self {
        Self(value)
    }
}

impl<T> ConcurrentElement<T> {
    /// Returns a clone of value of the element.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    /// vec.extend(["foo", "bar"].map(|x| x.to_string()));
    ///
    /// assert_eq!(vec[0].cloned(), "foo".to_string());
    /// assert_eq!(vec[1].cloned(), "bar".to_string());
    ///
    /// vec[1].set("baz".to_string());
    /// assert_eq!(vec[1].cloned(), "baz".to_string());
    /// ```
    #[inline(always)]
    #[allow(clippy::missing_panics_doc)]
    pub fn cloned(&self) -> T
    where
        T: Clone,
    {
        self.0.clone_into_option().expect(HAS_VALUE)
    }

    /// Returns a copy of value of the element.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    /// vec.extend([42, 7]);
    ///
    /// assert_eq!(vec[0].copied(), 42);
    /// assert_eq!(vec[1].copied(), 7);
    ///
    /// vec[1].set(0);
    /// assert_eq!(vec[1].copied(), 0);
    /// ```
    #[inline(always)]
    #[allow(clippy::missing_panics_doc)]
    pub fn copied(&self) -> T
    where
        T: Copy,
    {
        self.0.clone_into_option().expect(HAS_VALUE)
    }

    /// Maps the value and returns the result of `f(&element)`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec: ConcurrentVec<_> = [0, 1, 2, 3].into_iter().collect();
    ///
    /// let one = vec[1].map(|x| x.to_string());
    /// assert_eq!(one, 1.to_string());
    ///
    /// let doubles: Vec<_> = vec.iter().map(|elem| elem.map(|x| x * 2)).collect();
    /// assert_eq!(doubles, [0, 2, 4, 6]);
    ///
    /// let mut sum = 0;
    /// for i in 0..vec.len() {
    ///     vec[i].map(|x| {
    ///         sum += x;
    ///     });
    /// }
    /// assert_eq!(sum, 6);
    /// ```
    #[inline(always)]
    #[allow(clippy::missing_panics_doc)]
    pub fn map<F, U>(&self, f: F) -> U
    where
        F: FnOnce(&T) -> U,
    {
        self.0.map(f).expect(HAS_VALUE)
    }

    // mut

    /// Replaces the current value of the element
    /// with the given `value`, and returns the old value.
    ///
    /// See also [`set`] if the old value is to be omitted.
    ///
    /// [`set`]: crate::ConcurrentElement::set
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter(['a', 'b', 'c', 'd']);
    ///
    /// let c = vec[2].replace('x');
    ///
    /// assert_eq!(c, 'c');
    /// assert_eq!(&vec, &['a', 'b', 'x', 'd']);
    /// ```
    #[inline(always)]
    #[allow(clippy::missing_panics_doc)]
    pub fn replace(&self, value: T) -> T {
        self.0.replace(value).expect(HAS_VALUE)
    }

    /// Sets (overwrites) value of the element with the given `value`.
    ///
    /// See also [`replace`] if the old value is required.
    ///
    /// [`replace`]: crate::ConcurrentElement::replace
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::new();
    /// vec.extend(['a', 'b', 'c', 'd']);
    ///
    /// vec[2].set('x');
    /// assert_eq!(&vec, &['a', 'b', 'x', 'd']);
    /// ```
    #[inline(always)]
    #[allow(clippy::missing_panics_doc)]
    pub fn set(&self, value: T) {
        assert!(self.0.set_some(value), "Failed to set the element");
    }

    /// Updates the current value of the element by calling the mutating
    /// function `f(&mut element)`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orx_concurrent_vec::*;
    ///
    /// let vec = ConcurrentVec::from_iter([0, 1, 2, 3]);
    ///
    /// vec[1].update(|x| *x *= 2);
    /// vec[2].update(|x| *x += 10);
    /// vec[3].update(|x| *x = 7);
    ///
    /// assert_eq!(&vec, &[0, 2, 12, 7]);
    /// ```
    #[inline(always)]
    #[allow(clippy::missing_panics_doc)]
    pub fn update<F>(&self, f: F)
    where
        F: FnMut(&mut T),
    {
        assert!(self.0.update_if_some(f), "Failed to update the element");
    }
}

// constants

const HAS_VALUE: &str = "ConcurrentElement must always have a value";
