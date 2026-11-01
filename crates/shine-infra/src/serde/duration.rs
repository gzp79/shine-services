use std::time::Duration;

pub const fn seconds<const VALUE: u64>() -> Duration {
    Duration::from_secs(VALUE)
}

pub const fn milliseconds<const VALUE: u64>() -> Duration {
    Duration::from_millis(VALUE)
}

pub mod seconds {
    use serde::{de::Error, Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seconds = u64::deserialize(deserializer)?;
        if seconds == 0 {
            return Err(D::Error::custom("must be greater than zero"));
        }
        Ok(Duration::from_secs(seconds))
    }
}

pub mod milliseconds {
    use serde::{de::Error, ser::Error as _, Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis().try_into().map_err(S::Error::custom)?)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let milliseconds = u64::deserialize(deserializer)?;
        if milliseconds == 0 {
            return Err(D::Error::custom("must be greater than zero"));
        }
        Ok(Duration::from_millis(milliseconds))
    }
}
