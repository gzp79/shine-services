use serde::de::{
    value::{Error, MapDeserializer},
    DeserializeOwned,
};

/// Deserializes `T` from an empty map, so every field takes its own `#[serde(default = ...)]`.
///
/// # Panics
/// If a field of `T` has no serde default.
pub fn default_from_serde<T: DeserializeOwned>() -> T {
    let empty = MapDeserializer::<_, Error>::new(std::iter::empty::<(&str, &str)>());
    T::deserialize(empty).unwrap_or_else(|err| {
        panic!(
            "{} has a field without a serde default: {err}",
            std::any::type_name::<T>()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use shine_test::test;

    fn seven() -> u32 {
        7
    }

    #[derive(Debug, Deserialize)]
    struct Defaulted {
        #[serde(default = "seven")]
        value: u32,
        #[serde(default)]
        name: String,
    }

    #[derive(Debug, Deserialize)]    
    struct Required {
        value: u32,
    }

    #[test]
    fn every_field_takes_its_serde_default() {
        let defaulted: Defaulted = default_from_serde();
        assert_eq!(defaulted.value, 7);
        assert_eq!(defaulted.name, "");
    }

    #[test]
    #[should_panic(expected = "without a serde default")]
    fn a_field_without_a_default_panics() {
        let _: Required = default_from_serde();
    }
}
