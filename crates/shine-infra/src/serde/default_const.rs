/// Functions with const generics to use as `serde(default = "...")` paths, so a numeric/bool/char
/// default can be written inline (e.g. `#[serde(default = "shine_infra::serde::u64::<300>")]`)
/// instead of a dedicated named function per field.
/// See [issue](https://github.com/serde-rs/serde/issues/368) for more details; this can be removed once the issue is resolved.
macro_rules! ty {
    ($ty:ty, $fn_name:tt) => {
        pub const fn $fn_name<const T: $ty>() -> $ty {
            T
        }
    };
}

ty!(usize, usize);

ty!(u128, u128);
ty!(u64, u64);
ty!(u32, u32);
ty!(u16, u16);
ty!(u8, u8);

ty!(i128, i128);
ty!(i64, i64);
ty!(i32, i32);
ty!(i16, i16);
ty!(i8, i8);

ty!(bool, bool);

ty!(char, char);

#[cfg(test)]
mod test {
    use serde::Deserialize;
    use shine_test::test;

    #[derive(Deserialize)]
    struct Defaulted {
        #[serde(default = "crate::serde::u64::<300>")]
        value: u64,
    }

    #[test]
    fn a_missing_field_uses_the_const_default() {
        let parsed: Defaulted = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed.value, 300);
    }

    #[test]
    fn a_present_field_overrides_the_default() {
        let parsed: Defaulted = serde_json::from_str(r#"{"value":7}"#).unwrap();
        assert_eq!(parsed.value, 7);
    }
}
