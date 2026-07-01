use crate::indexed::EnumIndex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt,
    marker::PhantomData,
    ops::{Index, IndexMut},
    slice,
};

/// A `Vec<T>` that can only be indexed by a specific `EnumIndex` type.
pub struct EnumVec<I: EnumIndex, T>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    data: Vec<T>,
    _phantom: PhantomData<I>,
}

impl<I: EnumIndex, T> EnumVec<I, T>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
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
        let idx = I::try_from(self.data.len()).unwrap();
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

    pub fn iter(&self) -> slice::Iter<'_, T> {
        self.data.iter()
    }

    pub fn iter_mut(&mut self) -> slice::IterMut<'_, T> {
        self.data.iter_mut()
    }

    /// Iterate with typed indices: `(I, &T)`.
    pub fn iter_indexed(&self) -> impl Iterator<Item = (I, &T)> {
        self.data.iter().enumerate().map(|(i, v)| (I::try_from(i).unwrap(), v))
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
        self.data.swap(a.into(), b.into());
    }
}

impl<I: EnumIndex, T: Clone> EnumVec<I, T>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    pub fn from_elem(value: T, count: usize) -> Self {
        Self {
            data: vec![value; count],
            _phantom: PhantomData,
        }
    }
}

impl<I: EnumIndex, T> From<Vec<T>> for EnumVec<I, T>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    fn from(data: Vec<T>) -> Self {
        Self { data, _phantom: PhantomData }
    }
}

impl<I: EnumIndex, T: Clone> Clone for EnumVec<I, T>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<I: EnumIndex, T> Index<I> for EnumVec<I, T>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    type Output = T;

    #[inline]
    fn index(&self, idx: I) -> &T {
        &self.data[idx.into()]
    }
}

impl<I: EnumIndex, T> IndexMut<I> for EnumVec<I, T>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    #[inline]
    fn index_mut(&mut self, idx: I) -> &mut T {
        &mut self.data[idx.into()]
    }
}

impl<I: EnumIndex, T: std::fmt::Debug> std::fmt::Debug for EnumVec<I, T>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnumVec")
            .field("len", &self.data.len())
            .field("data", &self.data)
            .finish()
    }
}

impl<I: EnumIndex, T> Default for EnumVec<I, T>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<I: EnumIndex, T: Serialize> Serialize for EnumVec<I, T>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.data.serialize(serializer)
    }
}

impl<'de, I: EnumIndex, T: Deserialize<'de>> Deserialize<'de> for EnumVec<I, T>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let data = Vec::<T>::deserialize(deserializer)?;
        Ok(Self::from(data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shine_test::test;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct TestEnum(usize);

    impl TestEnum {
        fn new(value: usize) -> Self {
            TestEnum(value)
        }
    }

    impl TryFrom<usize> for TestEnum {
        type Error = ();

        fn try_from(value: usize) -> Result<Self, Self::Error> {
            Ok(TestEnum(value))
        }
    }

    impl From<TestEnum> for usize {
        fn from(value: TestEnum) -> usize {
            value.0
        }
    }

    #[test]
    fn basic() {
        let mut v: EnumVec<TestEnum, f32> = EnumVec::new();
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
        let v: EnumVec<TestEnum, bool> = EnumVec::from_elem(false, 5);
        assert_eq!(v.len(), 5);
        assert!(!v[TestEnum::new(0)]);
        assert!(!v[TestEnum::new(4)]);
    }

    #[test]
    fn iter_indexed() {
        let v: EnumVec<TestEnum, &str> = EnumVec::from(vec!["a", "b", "c"]);
        let collected: Vec<_> = v.iter_indexed().collect();
        assert_eq!(collected[0], (TestEnum::new(0), &"a"));
        assert_eq!(collected[1], (TestEnum::new(1), &"b"));
        assert_eq!(collected[2], (TestEnum::new(2), &"c"));
    }

    #[test]
    fn serde_round_trip() {
        let v: EnumVec<TestEnum, u32> = EnumVec::from(vec![10, 20, 30]);
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "[10,20,30]");
        let v2: EnumVec<TestEnum, u32> = serde_json::from_str(&json).unwrap();
        assert_eq!(v2.len(), 3);
        assert_eq!(v2[TestEnum::new(0)], 10);
        assert_eq!(v2[TestEnum::new(2)], 30);
    }
}
