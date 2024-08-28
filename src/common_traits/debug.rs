use crate::ConcurrentVec;
use orx_concurrent_option::ConcurrentOption;
use orx_fixed_vec::IntoConcurrentPinnedVec;
use std::fmt::Debug;

const ELEM_PER_LINE: usize = 8;

impl<T, P> Debug for ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentOption<T>>,
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = self.len();
        let capacity = self.capacity();

        write!(f, "ConcurrentVec {{")?;
        write!(f, "\n    len: {},", len)?;
        write!(f, "\n    capacity: {},", capacity)?;
        write!(f, "\n    data: [,")?;
        for i in 0..len {
            if i % ELEM_PER_LINE == 0 {
                write!(f, "\n        ")?;
            }
            match self.get(i) {
                Some(x) => write!(f, "{:?}, ", x)?,
                None => write!(f, "*, ")?,
            }
        }
        write!(f, "\n    ],")?;
        write!(f, "\n}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug() {
        let vec = ConcurrentVec::new();
        vec.extend([0, 4, 1, 2, 5, 6, 32, 5, 1, 121, 2, 42]);
        let dbg_str = format!("{:?}", &vec);
        assert_eq!(dbg_str, "ConcurrentVec {\n    len: 12,\n    capacity: 12,\n    data: [,\n        0, 4, 1, 2, 5, 6, 32, 5, \n        1, 121, 2, 42, \n    ],\n}");
    }
}
