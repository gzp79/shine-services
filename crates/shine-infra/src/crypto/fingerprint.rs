/// Short, non-reversible fingerprint (8 hex chars) of a secret or its hash (token, session key, ...) for log and
/// trace correlation. It is derived with a stable, cheap hash (FNV-1a, not a security primitive).
pub fn hash_fingerprint(hash: &str) -> String {
    let h = hash.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{:08x}", h >> 32)
}
