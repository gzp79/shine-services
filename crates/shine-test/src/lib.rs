pub use shine_test_macros::test;
use std::sync::Once;

static INIT: Once = Once::new();

/// Test setup executed before each test.
pub fn setup_test() {
    #[cfg(not(any(target_arch = "wasm32", miri)))]
    {
        INIT.call_once(|| {
            use tracing_subscriber::{
                fmt::{self, format::FmtSpan},
                layer::SubscriberExt,
                util::SubscriberInitExt,
                EnvFilter,
            };

            let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
            let fmt = fmt::layer()
                .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT) // show span enter/exit
                .with_target(false) // optional: hide module path
                .pretty();

            tracing_subscriber::registry().with(fmt).with(filter).init();
        });
    }

    #[cfg(target_family = "wasm")]
    {
        // logger it should be initialized only once otherwise some warning it thrown
        INIT.call_once(|| wasm_logger::init(::wasm_logger::Config::new(log::Level::Trace)));
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    {
        let orig_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            // invoke the default handler and exit the process
            orig_hook(panic_info);
            std::process::exit(-1);
        }));
    }
}
