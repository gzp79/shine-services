use crate::indexed::TypedIndex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    marker::PhantomData,
    ops::{Index, IndexMut},
};

/// A `Vec<T>` that can only be indexed by a specific `TypedIndex` type.
/// Prevents accidentally indexing a vertex array with a quad index (and vice versa).
pub struct IdxVec<I: TypedIndex, T> {
    data: Vec<T>,
    _phantom: PhantomData<I>,
}

impl<I: TypedIndex, T> IdxVec<I, T> {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            _phantom: PhantomData,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            _phantom: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn push(&mut self, value: T) -> I {
        let idx = I::new(self.data.len());
        self.data.push(value);
        idx
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn resize(&mut self, new_len: usize, value: T)
    where
        T: Clone,
    {
        self.data.resize(new_len, value);
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

    pub fn into_inner(self) -> Vec<T> {
        self.data
    }

    pub fn swap(&mut self, a: I, b: I) {
        self.data.swap(a.into_index(), b.into_index());
    }
}

impl<I: TypedIndex, T: Clone> IdxVec<I, T> {
    pub fn from_elem(value: T, count: usize) -> Self {
        Self {
            data: vec![value; count],
            _phantom: PhantomData,
        }
    }
}

impl<I: TypedIndex, T> From<Vec<T>> for IdxVec<I, T> {
    fn from(data: Vec<T>) -> Self {
        Self { data, _phantom: PhantomData }
    }
}

impl<I: TypedIndex, T: Clone> Clone for IdxVec<I, T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<I: TypedIndex, T> Index<I> for IdxVec<I, T> {
    type Output = T;

    #[inline]
    fn index(&self, idx: I) -> &T {
        &self.data[idx.into_index()]
    }
}

impl<I: TypedIndex, T> IndexMut<I> for IdxVec<I, T> {
    #[inline]
    fn index_mut(&mut self, idx: I) -> &mut T {
        &mut self.data[idx.into_index()]
    }
}

impl<I: TypedIndex, T: std::fmt::Debug> std::fmt::Debug for IdxVec<I, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdxVec")
            .field("len", &self.data.len())
            .field("data", &self.data)
            .finish()
    }
}

impl<I: TypedIndex, T> Default for IdxVec<I, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: TypedIndex, T: Serialize> Serialize for IdxVec<I, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.data.serialize(serializer)
    }
}

impl<'de, I: TypedIndex, T: Deserialize<'de>> Deserialize<'de> for IdxVec<I, T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let data = Vec::<T>::deserialize(deserializer)?;
        Ok(Self::from(data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shine_test::test;

    crate::define_typed_index!(TestIdx, "Test index for IdxVec tests.");

    #[test]
    fn basic() {
        let mut v: IdxVec<TestIdx, f32> = IdxVec::new();
        let i0 = v.push(1.0);
        let i1 = v.push(2.0);
        let i2 = v.push(3.0);

        assert_eq!(v.len(), 3);
        assert_eq!(v[i0], 1.0);
        assert_eq!(v[i1], 2.0);
        assert_eq!(v[i2], 3.0);

        v[i1] = 20.0;
        assert_eq!(v[i1], 20.0);
    }

    #[test]
    fn from_elem() {
        let v: IdxVec<TestIdx, bool> = IdxVec::from_elem(false, 5);
        assert_eq!(v.len(), 5);
        assert!(!v[TestIdx::new(0)]);
        assert!(!v[TestIdx::new(4)]);
    }

    #[test]
    fn iter_indexed() {
        let v: IdxVec<TestIdx, &str> = IdxVec::from(vec!["a", "b", "c"]);
        let collected: Vec<_> = v.iter_indexed().collect();
        assert_eq!(collected[0], (TestIdx::new(0), &"a"));
        assert_eq!(collected[1], (TestIdx::new(1), &"b"));
        assert_eq!(collected[2], (TestIdx::new(2), &"c"));
    }

    #[test]
    fn serde_round_trip() {
        let v: IdxVec<TestIdx, u32> = IdxVec::from(vec![10, 20, 30]);
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "[10,20,30]");
        let v2: IdxVec<TestIdx, u32> = serde_json::from_str(&json).unwrap();
        assert_eq!(v2.len(), 3);
        assert_eq!(v2[TestIdx::new(0)], 10);
        assert_eq!(v2[TestIdx::new(2)], 30);
    }
}
