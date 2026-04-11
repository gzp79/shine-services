/// Check if `pattern` is a rotation of `reference`.
///
/// A rotation means the pattern can be obtained by shifting the reference slice
/// circularly. For example, `[1, 2, 3]` is a rotation of `[2, 3, 1]`.
///
/// Uses modulo arithmetic to avoid duplicating the reference slice.
///
/// # Examples
///
/// ```
/// use shine_core::utils::is_rotation;
///
/// assert!(is_rotation(&[1, 2, 3, 4], &[3, 4, 1, 2]));
/// assert!(is_rotation(&[1, 2, 3, 4], &[1, 2, 3, 4]));
/// assert!(!is_rotation(&[1, 2, 3, 4], &[1, 3, 2, 4]));
/// assert!(!is_rotation(&[1, 2, 3], &[1, 2, 3, 4]));
/// ```
pub fn is_rotation<T: PartialEq>(reference: &[T], pattern: &[T]) -> bool {
    // Different lengths cannot be rotations
    if reference.len() != pattern.len() {
        return false;
    }

    // Empty slices are rotations of each other
    if reference.is_empty() {
        return true;
    }

    // Find all positions where pattern[0] occurs in reference
    for start_idx in 0..reference.len() {
        if reference[start_idx] == pattern[0] {
            // Check if pattern matches starting from this position using modulo
            let matches = (0..pattern.len()).all(|i| reference[(start_idx + i) % reference.len()] == pattern[i]);

            if matches {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_rotation_basic() {
        assert!(is_rotation(&[1, 2, 3, 4], &[1, 2, 3, 4]));
        assert!(is_rotation(&[1, 2, 3, 4], &[2, 3, 4, 1]));
        assert!(is_rotation(&[1, 2, 3, 4], &[3, 4, 1, 2]));
        assert!(is_rotation(&[1, 2, 3, 4], &[4, 1, 2, 3]));
    }

    #[test]
    fn test_is_rotation_not_rotation() {
        assert!(!is_rotation(&[1, 2, 3, 4], &[1, 3, 2, 4]));
        assert!(!is_rotation(&[1, 2, 3, 4], &[4, 3, 2, 1]));
    }

    #[test]
    fn test_is_rotation_different_lengths() {
        assert!(!is_rotation(&[1, 2, 3], &[1, 2, 3, 4]));
        assert!(!is_rotation(&[1, 2, 3, 4], &[1, 2, 3]));
    }

    #[test]
    fn test_is_rotation_empty() {
        assert!(is_rotation::<i32>(&[], &[]));
    }

    #[test]
    fn test_is_rotation_single_element() {
        assert!(is_rotation(&[1], &[1]));
        assert!(!is_rotation(&[1], &[2]));
    }

    #[test]
    fn test_is_rotation_strings() {
        assert!(is_rotation(&["a", "b", "c"], &["b", "c", "a"]));
        assert!(!is_rotation(&["a", "b", "c"], &["a", "c", "b"]));
    }
}
