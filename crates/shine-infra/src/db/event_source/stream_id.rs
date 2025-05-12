use std::fmt::Debug;
use uuid::Uuid;

pub trait StreamId: 'static + Clone + PartialEq + Debug + Send + Sync {
    fn to_string(&self) -> String;
    fn from_string(value: String) -> Self;
}

impl StreamId for String {
    fn to_string(&self) -> String {
        self.clone()
    }

    fn from_string(value: String) -> Self {
        value
    }
}

impl StreamId for Uuid {
    fn to_string(&self) -> String {
        self.as_hyphenated().to_string()
    }

    fn from_string(value: String) -> Self {
        Uuid::parse_str(&value).expect("Invalid UUID format")
    }
}
