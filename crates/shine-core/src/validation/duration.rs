//! Reusable `#[validate(custom(function = "..."))]` checks.

use std::time::Duration;
use validator::ValidationError;

pub fn non_zero_duration(value: &Duration) -> Result<(), ValidationError> {
    if value.is_zero() {
        return Err(ValidationError::new("non_zero").with_message("must be greater than zero".into()));
    }
    Ok(())
}
