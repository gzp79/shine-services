use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::quote;
use syn::{
    meta,
    parse::{Error as ParseError, Parser},
    parse_macro_input, Ident, ItemFn, LitStr, ReturnType,
};

#[derive(Default, Debug)]
struct TestAttributes {
    pub serial: Option<LitStr>,
    pub skip: Option<LitStr>,
}

impl TestAttributes {
    fn parse(input: TokenStream) -> Result<Self, ParseError> {
        let mut attrs = Self::default();

        let parser = meta::parser(|meta| {
            if meta.path.is_ident("serial") {
                attrs.serial = Some(meta.value()?.parse()?);
                Ok(())
            } else if meta.path.is_ident("skip") {
                attrs.skip = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("unsupported tea property"))
            }
        });

        parser.parse(input)?;
        Ok(attrs)
    }
}

/// An improved test macro. Supported properties
/// - serial - ensure the test is not running in parallel with other tests.
/// - skip - skip the test
#[proc_macro_attribute]
pub fn test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = TestAttributes::parse(attr).unwrap();
    let input = parse_macro_input!(item as ItemFn);

    let mut test_decors = Vec::new();
    if input.sig.asyncness.is_some() {
        test_decors.push(quote! { #[cfg_attr(not(target_arch = "wasm32"), ::tokio::test(flavor = "multi_thread"))] });
        test_decors.push(quote! { #[cfg_attr(target_arch = "wasm32", ::wasm_bindgen_test::wasm_bindgen_test)] });
    } else {
        test_decors.push(quote! { #[cfg_attr(not(target_arch = "wasm32"), ::core::prelude::v1::test)] });
        test_decors.push(quote! { #[cfg_attr(target_arch = "wasm32", ::wasm_bindgen_test::wasm_bindgen_test)] });
    };

    if let Some(serial) = attrs.serial {
        test_decors.push(quote! { #[::shine_test::serial_test::serial(#serial)] });
    }
    if let Some(skip) = attrs.skip {
        test_decors.push(quote! { #[ignore = #skip] });
    }

    expand_wrapper(&test_decors, &input)
}

/// Expand the wasm bindgen configuration, By default all tests are running in (headless) browser.
fn expand_wasm_bindgen_test_configure(test_name: &Ident) -> Tokens {
    quote! {
      #[cfg(target_family = "wasm")]
      mod #test_name {
        ::wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
      }
    }
}

/// Emit code for a wrapper function around a test function.
fn expand_wrapper(test_decors: &[Tokens], input: &ItemFn) -> TokenStream {
    let async_token = &input.sig.asyncness;
    let await_token = async_token.map(|_| quote! {.await});

    let body = &input.block;
    let test_name = &input.sig.ident;

    // Note: Rust does not allow us to have a test function with #[should_panic] that has a non-unit return value.
    let ret = match &input.sig.output {
        ReturnType::Default => quote! {},
        ReturnType::Type(_, ty) => quote! {-> #ty},
    };

    let wasm_bindgen_test_configure = expand_wasm_bindgen_test_configure(test_name);

    let result = quote! {
      #wasm_bindgen_test_configure

      #(#test_decors)*
      #async_token fn #test_name() #ret {
        #async_token fn test_impl() #ret {
          #body
        }

        ::shine_test::setup_test();

        test_impl()#await_token
      }
    };
    result.into()
}
