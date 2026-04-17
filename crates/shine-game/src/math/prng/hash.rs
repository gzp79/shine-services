/// 64-bit FNV-1a hash function for strings.
pub fn fnv1a64(s: &str) -> u64 {
    let mut h: u64 = 14695981039346656037;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

/// A 32-bit hash function for two 32-bit values.
pub fn hash_u32_2(a: u32, b: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9e3779b9).wrapping_add(b);
    h ^= h >> 16;
    h = h.wrapping_mul(0x85ebca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2ae35);
    h ^= h >> 16;
    h
}
