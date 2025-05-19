pub use shine_test_macros::test;

#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen_test::wasm_bindgen_test;
#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen_test::wasm_bindgen_test_configure;

/// Test setup executed before each test.
pub fn setup_test() {
    #[cfg(not(any(target_arch = "wasm32", miri)))]
    {
        let _ = env_logger::builder().is_test(true).try_init();
        color_backtrace::install();
    }

    #[cfg(target_arch = "wasm32")]
    {
        // logger it should be initialized only once otherwise some warning it thrown
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| wasm_logger::init(::wasm_logger::Config::new(log::Level::Trace)));
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let orig_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            // invoke the default handler and exit the process
            orig_hook(panic_info);
            std::process::exit(-1);
        }));
    }
}
