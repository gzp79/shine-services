use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(RedisJsonValue)]
pub fn redis_json_value(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_type = input.ident;

    let expanded = quote! {
      impl redis::ToRedisArgs for #struct_type {
          fn write_redis_args<W>(&self, out: &mut W)
          where
            W: ?Sized + redis::RedisWrite,
          {
            out.write_arg(&serde_json::to_vec(self).expect("JSON encoding failed for RedisJsonValue"));
          }
        }

        impl redis::FromRedisValue for #struct_type {
          fn from_redis_value(v: redis::Value) -> Result<Self, redis::ParsingError> {
            match v {
              redis::Value::BulkString(ref bytes) => Ok(serde_json::from_slice(bytes).map_err(|err| err.to_string())?),
              _ => Err("Invalid redis type for RedisJsonValue".into()),
            }
          }
        }

        impl redis::ToSingleRedisArg for #struct_type {}
    };

    TokenStream::from(expanded)
}
