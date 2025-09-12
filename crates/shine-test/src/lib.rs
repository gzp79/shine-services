pub use shine_test_macros::test;
use std::sync::Once;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

static INIT: Once = Once::new();

fn init_tracing_with_env_like() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt = fmt::layer()
        .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT) // show span enter/exit
        .with_target(false) // optional: hide module path
        .pretty();

    tracing_subscriber::registry().with(fmt).with(filter).init();
}

/// Test setup executed before each test.
pub fn setup_test() {
    #[cfg(not(any(target_arch = "wasm32", miri)))]
    {
        INIT.call_once(|| {
            init_tracing_with_env_like();
        });
    }

    #[cfg(target_arch = "wasm32")]
    {
        // logger it should be initialized only once otherwise some warning it thrown
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
