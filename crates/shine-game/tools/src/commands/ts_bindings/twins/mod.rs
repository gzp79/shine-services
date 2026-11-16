//! Schema twins of the replica states, for the binding generator only; never built at runtime.
//!
//! `postcard-bindgen` needs every field type to implement its `GenBinding`, which `Uuid` and `VecDeque` do not, and
//! a foreign type cannot get a derive. A twin has the same name and the same postcard wire form as the real state,
//! with the unsupported types replaced by their wire form: `Uuid` is length-prefixed bytes, `VecDeque` is a sequence.
//! The tests keep the twins and the real states in agreement.

mod chat;
mod pixel_map;

pub use self::chat::{ChatCommand, ChatMessage, ChatState};
pub use self::pixel_map::{PixelMapCommand, PixelMapState};

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    /// Encodes the real state, decodes it as the twin and encodes it again: the bytes must survive unchanged.
    pub fn assert_twin<Real, Twin>(real: &Real)
    where
        Real: Serialize,
        Twin: Serialize + for<'de> Deserialize<'de>,
    {
        let bytes = postcard::to_stdvec(real).expect("real state encodes");
        let (twin, rest) = postcard::take_from_bytes::<Twin>(&bytes).expect("twin decodes the real state");
        assert!(rest.is_empty(), "twin left {} trailing bytes", rest.len());
        assert_eq!(bytes, postcard::to_stdvec(&twin).expect("twin encodes"));
    }
}
