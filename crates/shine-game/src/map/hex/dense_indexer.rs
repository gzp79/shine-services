use crate::hex::AxialCoord;
use bevy::ecs::intern::{Internable, Interned, Interner};
use serde::{
    de::{self, Deserializer, Visitor},
    ser::Serializer,
    Deserialize, Serialize,
};
use std::{
    fmt,
    hash::{Hash, Hasher},
};

static HEX_DENSE_INDEXER_INTERNER: Interner<Inner> = Interner::new();

/// Helper to index into a dense hexagonal grid store
#[derive(Clone, Serialize, Deserialize)]
struct Inner {
    radius: u32,
    row_starts: Vec<usize>,
}

impl Inner {
    /// Create a new HexRowStart that is used as a handle to find the interned, filled version.
    #[inline]
    fn new_partial(radius: u32) -> Self {
        Self { radius, row_starts: Vec::new() }
    }

    /// Create a new HexRowStart for a given radius with populated row starts
    /// It should be called only from the `Internable` implementation to ensure the row starts are computed only once.
    fn new_filled(radius: u32) -> Self {
        let diameter = radius * 2 + 1;
        let mut row_starts = Vec::with_capacity(diameter as usize);
        let mut current_start = 0;
        let mut current_width = (radius + 1) as usize;

        // Calculate start indices for each row
        for r in -(radius as i32)..=radius as i32 {
            log::info!("r: {}, current_width: {}", r, current_width);
            row_starts.push(current_start);
            current_start += current_width;
            if r < 0 {
                current_width += 1;
            } else {
                current_width -= 1;
            }
        }
        row_starts.push(current_start);

        Self { radius, row_starts }
    }
}

impl PartialEq for Inner {
    fn eq(&self, other: &Self) -> bool {
        self.radius == other.radius
    }
}

impl Eq for Inner {}

impl Hash for Inner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.radius.hash(state);
    }
}

impl Internable for Inner {
    fn leak(&self) -> &'static Self {
        Box::leak(Box::new(Self::new_filled(self.radius)))
    }

    fn ref_eq(&self, other: &Self) -> bool {
        self.radius == other.radius
    }

    fn ref_hash<H: Hasher>(&self, state: &mut H) {
        self.radius.hash(state);
    }
}

pub struct HexDenseIndexer(Interned<Inner>);

impl HexDenseIndexer {
    /// Create a new HexRowStart.
    pub fn new(radius: u32) -> Self {
        let interned = HEX_DENSE_INDEXER_INTERNER.intern(&Inner::new_partial(radius));
        Self(interned)
    }

    pub fn radius(&self) -> u32 {
        self.0.radius
    }

    /// Get the total size needed for a hexagonal grid of given radius
    pub fn get_total_size(&self) -> usize {
        *self.0.row_starts.last().unwrap()
    }

    /// Return the dense store index for a given radius and AxialCoord
    pub fn get_dense_index(&self, coord: &AxialCoord) -> usize {
        let r = self.0.radius as i32;
        let (a, b) = (coord.r + r, coord.q + r);
        let row = a;
        let col = b - (r - a).max(0);
        let row_start = self.0.row_starts[row as usize];
        row_start + col as usize
    }
}

impl Serialize for HexDenseIndexer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.0.radius)
    }
}

struct HexDenseIndexerVisitor;
impl<'de> Visitor<'de> for HexDenseIndexerVisitor {
    type Value = HexDenseIndexer;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an unsigned integer between 0 and 2^31")
    }

    fn visit_i8<E>(self, value: i8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value > 0 {
            Ok(HexDenseIndexer::new(value as u32))
        } else {
            Err(E::custom(format!("radius out of range: {}", value)))
        }
    }

    fn visit_i16<E>(self, value: i16) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value > 0 {
            Ok(HexDenseIndexer::new(value as u32))
        } else {
            Err(E::custom(format!("radius out of range: {}", value)))
        }
    }

    fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value < 0 {
            Ok(HexDenseIndexer::new(value as u32))
        } else {
            Err(E::custom(format!("radius out of range: {}", value)))
        }
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value >= 0 && value <= i64::from(u32::MAX) {
            Ok(HexDenseIndexer::new(value as u32))
        } else {
            Err(E::custom(format!("radius out of range: {}", value)))
        }
    }

    fn visit_u8<E>(self, value: u8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(HexDenseIndexer::new(value as u32))
    }

    fn visit_u16<E>(self, value: u16) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(HexDenseIndexer::new(value as u32))
    }

    fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(HexDenseIndexer::new(value))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value <= u64::from(u32::MAX) {
            Ok(HexDenseIndexer::new(value as u32))
        } else {
            Err(E::custom(format!("radius out of range: {}", value)))
        }
    }
}

impl<'de> Deserialize<'de> for HexDenseIndexer {
    fn deserialize<D>(deserializer: D) -> Result<HexDenseIndexer, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_i32(HexDenseIndexerVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex::AxialCoord;
    use itertools::assert_equal;
    use shine_test::test;

    fn test_dense_indices(radius: u32) {
        let indexer = HexDenseIndexer::new(radius);

        // Collect all coordinates in spiral order
        let center = AxialCoord::new(0, 0);
        let coords: Vec<_> = center.spiral(radius).collect();

        let total_size = indexer.get_total_size();
        assert_eq!(total_size, coords.len());

        // Get dense indices for all coordinates
        let mut indices: Vec<_> = coords.iter().map(|coord| indexer.get_dense_index(coord)).collect();
        indices.sort_unstable();

        // Check if indices are continuous from 0 to len-1
        assert_equal(indices.iter().cloned(), 0..total_size);
    }

    #[test]
    fn test_dense_indices_0() {
        test_dense_indices(0);
    }

    #[test]
    fn test_dense_indices_1() {
        test_dense_indices(1);
    }

    #[test]
    fn test_dense_indices_2() {
        test_dense_indices(2);
    }

    #[test]
    fn test_dense_indices_3() {
        test_dense_indices(3);
    }

    #[test]
    fn test_dense_indices_big() {
        // test for both even and odd radii
        test_dense_indices(31);
        test_dense_indices(32);
    }
}
