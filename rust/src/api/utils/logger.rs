use std::{
    sync::LazyLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use flutter_rust_bridge::frb;
use log::{Level, LevelFilter, Log, Record};
use parking_lot::{Once, RwLock};

use crate::{frb_generated::StreamSink, BackendError};

static LOGGING_STREAM_SINK: LazyLock<RwLock<Option<StreamSink<LogEntry>>>> =
    LazyLock::new(|| RwLock::new(None));
static LOGGER: FlutterLogger = FlutterLogger {};

/// Sets up the log stream for Rust --> Dart logs.
pub fn setup_log_stream(s: StreamSink<LogEntry>, level: LoggingLevel) -> Result<(), BackendError> {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Set the stream sink
        {
            let mut guard = LOGGING_STREAM_SINK.write();
            *guard = Some(s);
        }
        log::set_max_level(level.into());
        log::set_logger(&LOGGER).expect("failed to initialize logger!");
    });
    trace!("logger initialized");
    Ok(())
}

pub struct LogEntry {
    pub time_millis: i64,
    pub level: LoggingLevel,
    pub file_info: String,
    pub msg: String,
}

#[frb(ignore)]
struct FlutterLogger {}

impl Log for FlutterLogger {
    fn enabled(&self, meta: &log::Metadata) -> bool {
        meta.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let record = record.into(); // Do this before the lock occurrs.
        let mut guard = LOGGING_STREAM_SINK.write();
        if let Some(guard) = guard.as_mut() {
            if let Err(why) = guard.add(record) {
                debug!("failed to add log entry: {}", why);
            }
        }
    }

    fn flush(&self) {}
}

impl From<&Record<'_>> for LogEntry {
    fn from(record: &Record) -> Self {
        let time_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_millis() as i64;

        let level = match record.level() {
            Level::Trace => LoggingLevel::Trace,
            Level::Debug => LoggingLevel::Debug,
            Level::Info => LoggingLevel::Info,
            Level::Warn => LoggingLevel::Warn,
            Level::Error => LoggingLevel::Error,
        };

        let file_info = record
            .file()
            .map(|s| format!("{s}:{}", record.line().unwrap_or(0)))
            .unwrap_or_else(|| record.target().to_string());

        let msg = format!("{}", record.args());

        LogEntry {
            time_millis,
            level,
            file_info,
            msg,
        }
    }
}

pub enum LoggingLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<LoggingLevel> for LevelFilter {
    #[frb(ignore)]
    fn from(value: LoggingLevel) -> Self {
        match value {
            LoggingLevel::Trace => LevelFilter::Trace,
            LoggingLevel::Debug => LevelFilter::Debug,
            LoggingLevel::Info => LevelFilter::Info,
            LoggingLevel::Warn => LevelFilter::Warn,
            LoggingLevel::Error => LevelFilter::Error,
        }
    }
}
