pub use shine_test_macros::test;

#[cfg(not(target_arch = "wasm32"))]
pub use serial_test;
#[cfg(not(target_arch = "wasm32"))]
pub mod db;
#[cfg(all(feature = "toxiproxy", not(target_arch = "wasm32")))]
pub mod toxiproxy;

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

    #[cfg(target_arch = "wasm32")]
    {
        // logger it should be initialized only once otherwise some warning it thrown
        INIT.call_once(|| wasm_logger::init(::wasm_logger::Config::new(log::Level::Trace)));
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        PANIC_HOOK.call_once(install_panic_hook);
        // Called from the test body, so this is the thread the harness runs the test on.
        test_threads().insert(std::thread::current().id());
    }
}

#[cfg(not(target_arch = "wasm32"))]
static PANIC_HOOK: Once = Once::new();

#[cfg(not(target_arch = "wasm32"))]
fn test_threads() -> std::sync::MutexGuard<'static, std::collections::HashSet<std::thread::ThreadId>> {
    use std::sync::{LazyLock, Mutex};
    static TEST_THREADS: LazyLock<Mutex<std::collections::HashSet<std::thread::ThreadId>>> =
        LazyLock::new(Default::default);
    TEST_THREADS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A panic on the test's own thread fails that test and the harness reports it. A panic on a background
/// thread (e.g. a spawned task nobody awaits) would be swallowed, so it stops the whole run instead.
#[cfg(not(target_arch = "wasm32"))]
fn install_panic_hook() {
    use std::io::Write;

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        if test_threads().contains(&thread.id()) {
            default_hook(info);
            return;
        }
        let name = thread.name();

        // Written to stderr directly: the harness captures the panic output and would drop it on exit.
        let _ = writeln!(
            std::io::stderr(),
            "\npanic in background thread '{}', aborting the test run: {info}\n{}",
            name.unwrap_or("<unnamed>"),
            std::backtrace::Backtrace::capture()
        );
        std::process::exit(-1);
    }));
}
