use crate::indexed::TypedIndex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt,
    marker::PhantomData,
    ops::{Index, IndexMut},
};

/// A `Vec<T>` that can only be indexed by a specific `TypedIndex` type.
/// Prevents accidentally indexing a vertex array with a quad index (and vice versa).
pub struct IdxArray<I: TypedIndex, T, const LEN: usize> {
    data: [T; LEN],
    _phantom: PhantomData<I>,
}

impl<I: TypedIndex, T, const LEN: usize> IdxArray<I, T, LEN> {
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.data.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.data.iter_mut()
    }

    /// Iterate with typed indices: `(I, &T)`.
    pub fn iter_indexed(&self) -> impl Iterator<Item = (I, &T)> {
        self.data.iter().enumerate().map(|(i, v)| (I::new(i), v))
    }

    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data
    }

    pub fn into_inner(self) -> [T; LEN] {
        self.data
    }

    pub fn swap(&mut self, a: I, b: I) {
        self.data.swap(a.into_index(), b.into_index());
    }
}

impl<I: TypedIndex, T: Default + Copy, const LEN: usize> IdxArray<I, T, LEN> {
    pub fn new() -> Self
    where
        Self: Default,
    {
        Self {
            data: [T::default(); LEN],
            _phantom: PhantomData,
        }
    }
}

impl<I: TypedIndex, T: Default + Copy, const LEN: usize> Default for IdxArray<I, T, LEN> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: TypedIndex, T: Copy, const LEN: usize> IdxArray<I, T, LEN> {
    pub fn from_elem(value: T) -> Self {
        Self {
            data: [value; LEN],
            _phantom: PhantomData,
        }
    }
}

impl<I: TypedIndex, T: Copy, const LEN: usize> From<[T; LEN]> for IdxArray<I, T, LEN> {
    fn from(value: [T; LEN]) -> Self {
        Self {
            data: value,
            _phantom: PhantomData,
        }
    }
}

impl<I: TypedIndex, T: Clone, const LEN: usize> Clone for IdxArray<I, T, LEN> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<I: TypedIndex, T, const LEN: usize> Index<I> for IdxArray<I, T, LEN> {
    type Output = T;

    #[inline]
    fn index(&self, idx: I) -> &T {
        &self.data[idx.into_index()]
    }
}

impl<I: TypedIndex, T, const LEN: usize> IndexMut<I> for IdxArray<I, T, LEN> {
    #[inline]
    fn index_mut(&mut self, idx: I) -> &mut T {
        &mut self.data[idx.into_index()]
    }
}

impl<I: TypedIndex, T: fmt::Debug, const LEN: usize> fmt::Debug for IdxArray<I, T, LEN> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdxArray")
            .field("len", &self.data.len())
            .field("data", &self.data)
            .finish()
    }
}

impl<I: TypedIndex, T: Serialize, const LEN: usize> Serialize for IdxArray<I, T, LEN> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.data.serialize(serializer)
    }
}

impl<'de, I: TypedIndex, T: Deserialize<'de>, const LEN: usize> Deserialize<'de> for IdxArray<I, T, LEN> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let vec = Vec::<T>::deserialize(deserializer)?;
        let data: [T; LEN] = vec
            .try_into()
            .map_err(|v: Vec<T>| serde::de::Error::invalid_length(v.len(), &LEN.to_string().as_str()))?;
        Ok(Self { data, _phantom: PhantomData })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shine_test::test;

    crate::define_typed_index!(TestIdx, "Test index for IdxArray tests.");

    #[test]
    fn default_and_new() {
        let arr: IdxArray<TestIdx, i32, 3> = IdxArray::new();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[TestIdx::new(0)], 0);
        assert_eq!(arr[TestIdx::new(1)], 0);
        assert_eq!(arr[TestIdx::new(2)], 0);
    }

    #[test]
    fn from_elem() {
        let arr: IdxArray<TestIdx, f32, 4> = IdxArray::from_elem(1.5);
        assert_eq!(arr.len(), 4);
        for i in 0..4 {
            assert_eq!(arr[TestIdx::new(i)], 1.5);
        }
    }

    #[test]
    fn index_and_index_mut() {
        let mut arr: IdxArray<TestIdx, i32, 3> = IdxArray::from_elem(0);
        arr[TestIdx::new(0)] = 10;
        arr[TestIdx::new(1)] = 20;
        arr[TestIdx::new(2)] = 30;
        assert_eq!(arr[TestIdx::new(0)], 10);
        assert_eq!(arr[TestIdx::new(1)], 20);
        assert_eq!(arr[TestIdx::new(2)], 30);
    }

    #[test]
    fn iter_indexed() {
        let mut arr: IdxArray<TestIdx, &str, 3> = IdxArray::from_elem("");
        arr[TestIdx::new(0)] = "a";
        arr[TestIdx::new(1)] = "b";
        arr[TestIdx::new(2)] = "c";
        let collected: Vec<_> = arr.iter_indexed().collect();
        assert_eq!(collected[0], (TestIdx::new(0), &"a"));
        assert_eq!(collected[1], (TestIdx::new(1), &"b"));
        assert_eq!(collected[2], (TestIdx::new(2), &"c"));
    }

    #[test]
    fn into_inner() {
        let mut arr: IdxArray<TestIdx, u32, 2> = IdxArray::from_elem(0);
        arr[TestIdx::new(0)] = 5;
        arr[TestIdx::new(1)] = 10;
        assert_eq!(arr.into_inner(), [5, 10]);
    }

    #[test]
    fn clone() {
        let mut arr: IdxArray<TestIdx, i32, 2> = IdxArray::from_elem(0);
        arr[TestIdx::new(0)] = 42;
        let arr2 = arr.clone();
        assert_eq!(arr2[TestIdx::new(0)], 42);
    }

    #[test]
    fn serde_round_trip() {
        let mut arr: IdxArray<TestIdx, u32, 3> = IdxArray::from_elem(0);
        arr[TestIdx::new(0)] = 10;
        arr[TestIdx::new(1)] = 20;
        arr[TestIdx::new(2)] = 30;
        let json = serde_json::to_string(&arr).unwrap();
        assert_eq!(json, "[10,20,30]");
        let arr2: IdxArray<TestIdx, u32, 3> = serde_json::from_str(&json).unwrap();
        assert_eq!(arr2[TestIdx::new(0)], 10);
        assert_eq!(arr2[TestIdx::new(1)], 20);
        assert_eq!(arr2[TestIdx::new(2)], 30);
    }
}
