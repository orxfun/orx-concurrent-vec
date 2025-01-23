use crate::{ConcurrentElement, ConcurrentVec};
use core::marker::PhantomData;
use orx_concurrent_option::ConcurrentOption;
use orx_fixed_vec::IntoConcurrentPinnedVec;
use serde::{de::Visitor, ser::SerializeSeq, Deserialize, Serialize};

impl<T, P> Serialize for ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
    T: Serialize,
{
    /// Serializes the concurrent vector elements as a sequence.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let empty_vec = ConcurrentVec::<String>::new();
    /// let json = serde_json::to_string(&empty_vec).unwrap();
    /// assert_eq!(json, "[]");
    ///
    /// let vec = ConcurrentVec::new();
    /// for i in 0..7 {
    ///     vec.push(i.to_string());
    /// }
    /// let json = serde_json::to_string(&vec).unwrap();
    /// assert_eq!(json, "[\"0\",\"1\",\"2\",\"3\",\"4\",\"5\",\"6\"]");
    /// ```
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for element in self.iter() {
            element.map(|x| seq.serialize_element(x))?;
        }
        seq.end()
    }
}

struct ConcurrentVecDeserializer<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>>,
{
    phantom: PhantomData<(T, P)>,
}

impl<'de, T, P> Visitor<'de> for ConcurrentVecDeserializer<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>> + Default,
    T: Deserialize<'de>,
{
    type Value = ConcurrentVec<T, P>;

    fn expecting(&self, formatter: &mut alloc::fmt::Formatter) -> alloc::fmt::Result {
        formatter.write_str("Expecting the sequence of elements.")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut pinned = P::default();
        while let Some(x) = seq.next_element::<T>()? {
            pinned.push(ConcurrentElement(ConcurrentOption::some(x)));
        }
        Ok(ConcurrentVec::new_from_pinned(pinned))
    }
}

impl<'de, T, P> Deserialize<'de> for ConcurrentVec<T, P>
where
    P: IntoConcurrentPinnedVec<ConcurrentElement<T>> + Default,
    T: Deserialize<'de>,
{
    /// Deserializes a sequence as a concurrent vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use orx_concurrent_vec::*;
    ///
    /// let json = "[]";
    /// let result: Result<ConcurrentVec<String>, _> = serde_json::from_str(json);
    /// assert!(result.is_ok());
    /// let empty_vec = result.unwrap();
    /// assert!(empty_vec.is_empty());
    ///
    /// let json = "[0, 1, 2, 3, 4, 5, 6]";
    /// let result: Result<ConcurrentVec<u64>, _> = serde_json::from_str(json);
    /// assert!(result.is_ok());
    /// let vec = result.unwrap();
    /// assert_eq!(vec, [0, 1, 2, 3, 4, 5, 6]);
    /// ```
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(ConcurrentVecDeserializer {
            phantom: PhantomData,
        })
    }
}
