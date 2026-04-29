use crate::indexed::EnumIndex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt,
    marker::PhantomData,
    ops::{Index, IndexMut},
    slice,
};

/// A `[T;N]` that can only be indexed by an eenm like type.
pub struct EnumArray<I: EnumIndex, T, const LEN: usize>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    data: [T; LEN],
    _phantom: PhantomData<I>,
}

impl<I: EnumIndex, T, const LEN: usize> EnumArray<I, T, LEN>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    pub fn len(&self) -> usize {
        self.data.len()
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

    pub fn into_inner(self) -> [T; LEN] {
        self.data
    }

    pub fn swap(&mut self, a: I, b: I) {
        self.data.swap(a.into(), b.into());
    }
}

impl<I: EnumIndex, T: Default + Copy, const LEN: usize> EnumArray<I, T, LEN>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
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

impl<I: EnumIndex, T: Default + Copy, const LEN: usize> Default for EnumArray<I, T, LEN>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<I: EnumIndex, T: Copy, const LEN: usize> EnumArray<I, T, LEN>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    pub fn from_elem(value: T) -> Self {
        Self {
            data: [value; LEN],
            _phantom: PhantomData,
        }
    }
}

impl<I: EnumIndex, T: Copy, const LEN: usize> From<[T; LEN]> for EnumArray<I, T, LEN>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    fn from(value: [T; LEN]) -> Self {
        Self {
            data: value,
            _phantom: PhantomData,
        }
    }
}

impl<I: EnumIndex, T: Clone, const LEN: usize> Clone for EnumArray<I, T, LEN>
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

impl<I: EnumIndex, T, const LEN: usize> Index<I> for EnumArray<I, T, LEN>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    type Output = T;

    #[inline]
    fn index(&self, idx: I) -> &T {
        &self.data[idx.into()]
    }
}

impl<I: EnumIndex, T, const LEN: usize> IndexMut<I> for EnumArray<I, T, LEN>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    #[inline]
    fn index_mut(&mut self, idx: I) -> &mut T {
        &mut self.data[idx.into()]
    }
}

impl<I: EnumIndex, T: fmt::Debug, const LEN: usize> fmt::Debug for EnumArray<I, T, LEN>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnumArray")
            .field("len", &self.data.len())
            .field("data", &self.data)
            .finish()
    }
}

impl<I: EnumIndex, T: Serialize, const LEN: usize> Serialize for EnumArray<I, T, LEN>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.data.serialize(serializer)
    }
}

impl<'de, I: EnumIndex, T: Deserialize<'de>, const LEN: usize> Deserialize<'de> for EnumArray<I, T, LEN>
where
    <I as TryFrom<usize>>::Error: fmt::Debug,
{
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

    crate::define_enum_index!(
        #[derive(Debug, PartialEq, Eq)]
        /// A simple three-variant enum used for testing EnumArray.
        TestEnum {
            0 => A,
            1 => B,
            2 => C
        }
    );

    #[test]
    fn default_and_new() {
        let arr: EnumArray<TestEnum, i32, 3> = EnumArray::new();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[TestEnum::A], 0);
        assert_eq!(arr[TestEnum::B], 0);
        assert_eq!(arr[TestEnum::C], 0);
    }

    #[test]
    fn from_elem() {
        let arr: EnumArray<TestEnum, f32, 4> = EnumArray::from_elem(1.5);
        assert_eq!(arr.len(), 4);
        for i in [TestEnum::A, TestEnum::B, TestEnum::C] {
            assert_eq!(arr[i], 1.5);
        }
    }

    #[test]
    fn index_and_index_mut() {
        let mut arr: EnumArray<TestEnum, i32, 3> = EnumArray::from_elem(0);
        arr[TestEnum::A] = 10;
        arr[TestEnum::B] = 20;
        arr[TestEnum::C] = 30;
        assert_eq!(arr[TestEnum::A], 10);
        assert_eq!(arr[TestEnum::B], 20);
        assert_eq!(arr[TestEnum::C], 30);
    }

    #[test]
    fn iter_indexed() {
        let mut arr: EnumArray<TestEnum, &str, 3> = EnumArray::from_elem("");
        arr[TestEnum::A] = "a";
        arr[TestEnum::B] = "b";
        arr[TestEnum::C] = "c";
        let collected: Vec<_> = arr.iter_indexed().collect();
        assert_eq!(collected[0], (TestEnum::A, &"a"));
        assert_eq!(collected[1], (TestEnum::B, &"b"));
        assert_eq!(collected[2], (TestEnum::C, &"c"));
    }

    #[test]
    fn into_inner() {
        let mut arr: EnumArray<TestEnum, u32, 2> = EnumArray::from_elem(0);
        arr[TestEnum::A] = 5;
        arr[TestEnum::B] = 10;
        assert_eq!(arr.into_inner(), [5, 10]);
    }

    #[test]
    fn clone() {
        let mut arr: EnumArray<TestEnum, i32, 2> = EnumArray::from_elem(0);
        arr[TestEnum::A] = 42;
        let arr2 = arr.clone();
        assert_eq!(arr2[TestEnum::A], 42);
    }

    #[test]
    fn serde_round_trip() {
        let mut arr: EnumArray<TestEnum, u32, 3> = EnumArray::from_elem(0);
        arr[TestEnum::A] = 10;
        arr[TestEnum::B] = 20;
        arr[TestEnum::C] = 30;
        let json = serde_json::to_string(&arr).unwrap();
        assert_eq!(json, "[10,20,30]");
        let arr2: EnumArray<TestEnum, u32, 3> = serde_json::from_str(&json).unwrap();
        assert_eq!(arr2[TestEnum::A], 10);
        assert_eq!(arr2[TestEnum::B], 20);
        assert_eq!(arr2[TestEnum::C], 30);
    }
}
