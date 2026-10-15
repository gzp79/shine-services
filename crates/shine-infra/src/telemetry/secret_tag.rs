use std::fmt;
use tracing::field::DisplayValue;

/// Short, non-reversible tag (8 hex chars) of a secret or its hash (token, session key, ...) for log and
/// trace correlation. It is derived with a stable, cheap hash (FNV-1a, not a security primitive).
/// The hash is computed only when the tag is formatted.
#[derive(Clone, Copy)]
pub struct SecretTag<'a>(&'a str);

impl fmt::Display for SecretTag<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let h = self.0.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
        write!(f, "{:08x}", h >> 32)
    }
}

/// The tag as a tracing field value (also `Display`), usable in `Span::record`, `#[instrument]` fields and logs.
pub fn secret_tag(secret: &str) -> DisplayValue<SecretTag<'_>> {
    tracing::field::display(SecretTag(secret))
}
