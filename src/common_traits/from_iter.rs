use crate::ConcurrentVec;

impl<T> FromIterator<T> for ConcurrentVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let vec = ConcurrentVec::new();

        let iter = iter.into_iter();
        match iter.size_hint() {
            (a, Some(b)) if a == b => {
                vec.extend_n_items(iter, a);
            }
            _ => {
                for x in iter {
                    vec.push(x);
                }
            }
        }

        vec
    }
}
