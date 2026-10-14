/// Short, non-reversible prefix of a hex hash (token, session key, ...) for log and trace correlation.
/// It is enough to identify the same secret across spans (and to match it against the stored hash)
/// without making the stored hash usable for lookups.
pub fn hash_fingerprint(hash: &str) -> &str {
    &hash[..hash.len().min(8)]
}
