use godot::global::godot_print;
use log::{Level, Metadata, Record};

static LOGGER: GodotLogger = GodotLogger;

pub struct GodotLogger;

impl GodotLogger {
    pub fn init() {
        log::set_logger(&LOGGER).expect("Failed to set logger");
        log::set_max_level(log::LevelFilter::Debug);
    }
}

impl log::Log for GodotLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            godot_print!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}
