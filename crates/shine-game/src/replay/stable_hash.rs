use std::collections::BTreeMap;
use uuid::Uuid;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// 64-bit FNV-1a; identical output on native and wasm.
#[derive(Clone, Debug)]
pub struct StableHasher(u64);

impl Default for StableHasher {
    fn default() -> Self {
        Self(FNV_OFFSET)
    }
}

impl StableHasher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Continues a chain from a previously finished hash, rather than from the default seed.
    pub fn chain(base_hash: u64) -> Self {
        Self(base_hash)
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= *byte as u64;
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }

    pub fn finish(&self) -> u64 {
        self.0
    }
}

/// Platform independent hashing of replayed state. Not implemented for `usize`, floats or
/// `HashMap`: their encoding or iteration order differs between platforms.
pub trait StableHash {
    fn stable_hash(&self, hasher: &mut StableHasher);
}

/// Hash of a single value.
pub fn stable_hash_of<T: StableHash + ?Sized>(value: &T) -> u64 {
    let mut hasher = StableHasher::new();
    value.stable_hash(&mut hasher);
    hasher.finish()
}

macro_rules! impl_stable_hash_int {
    ($($ty:ty),*) => {
        $(impl StableHash for $ty {
            fn stable_hash(&self, hasher: &mut StableHasher) {
                hasher.write(&self.to_le_bytes());
            }
        })*
    };
}

impl_stable_hash_int!(u8, u16, u32, u64, i8, i16, i32, i64);

impl StableHash for bool {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        hasher.write(&[*self as u8]);
    }
}

/// Length (u64, little endian), then the UTF-8 bytes.
impl StableHash for str {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        (self.len() as u64).stable_hash(hasher);
        hasher.write(self.as_bytes());
    }
}

/// The 16 bytes.
impl StableHash for Uuid {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        hasher.write(self.as_bytes());
    }
}

impl<T: StableHash> StableHash for [T] {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        (self.len() as u64).stable_hash(hasher);
        for item in self {
            item.stable_hash(hasher);
        }
    }
}

impl<T: StableHash, const N: usize> StableHash for [T; N] {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        self.as_slice().stable_hash(hasher);
    }
}

impl<T: StableHash> StableHash for Vec<T> {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        self.as_slice().stable_hash(hasher);
    }
}

impl<T: StableHash> StableHash for Option<T> {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        match self {
            None => 0u8.stable_hash(hasher),
            Some(value) => {
                1u8.stable_hash(hasher);
                value.stable_hash(hasher);
            }
        }
    }
}

impl<K: StableHash, V: StableHash> StableHash for BTreeMap<K, V> {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        (self.len() as u64).stable_hash(hasher);
        for (key, value) in self {
            key.stable_hash(hasher);
            value.stable_hash(hasher);
        }
    }
}
