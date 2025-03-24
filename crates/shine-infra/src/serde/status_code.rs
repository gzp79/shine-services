pub mod serde_status_code {
    use axum::http::StatusCode;
    use serde::Serializer;

    pub fn serialize<S>(value: &StatusCode, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i32(value.as_u16() as i32)
    }
}
