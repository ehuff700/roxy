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
        {
            *LOGGING_STREAM_SINK.write() = Some(s);
        }
        log::set_max_level(level.into());
        log::set_logger(&LOGGER).expect("failed to initialize logger!");
    });
    trace!("logger initialized");
    Ok(())
}

/// A log entry that represents a single log message with metadata.
///
/// This struct is used to pass log messages from Rust to Dart through a stream.
/// It contains all the necessary information to reconstruct the log message
/// on the Dart side with proper formatting and context.
pub struct LogEntry {
    /// The number of microseconds since the Unix epoch (1970-01-01 00:00:00 UTC)
    /// when this log entry was created.
    pub micros_since_epoch: u128,
    /// The severity level of this log entry (error, warn, info, debug, or trace).
    pub level: LoggingLevel,
    /// The source file information in the format "filename:line_number" or just
    /// the target name if file information is not available.
    pub file_info: String,
    /// The actual log message.
    pub msg: String,
}

#[frb(ignore)]
struct FlutterLogger {}

impl Log for FlutterLogger {
    fn enabled(&self, meta: &log::Metadata) -> bool {
        meta.level() <= log::max_level() && meta.target().starts_with("rust_lib_roxy")
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let record = record.into(); // Do this before the lock occurrs.
        {
            let guard = LOGGING_STREAM_SINK.read();
            guard.as_ref().map(|s| {
                s.add(record)
                    .inspect_err(|e| debug!("failed to add log entry: {}", e))
            });
        }
    }

    fn flush(&self) {}
}

impl From<&Record<'_>> for LogEntry {
    fn from(record: &Record) -> Self {
        let micros_since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_micros();

        let level = record.level().into();

        let file_info = record
            .file()
            .map(|s| format!("{s}:{}", record.line().unwrap_or(0)))
            .unwrap_or_else(|| record.target().to_string());

        let msg = format!("{}", record.args());

        LogEntry {
            micros_since_epoch,
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

impl From<Level> for LoggingLevel {
    #[frb(ignore)]
    fn from(value: Level) -> Self {
        match value {
            Level::Trace => LoggingLevel::Trace,
            Level::Debug => LoggingLevel::Debug,
            Level::Info => LoggingLevel::Info,
            Level::Warn => LoggingLevel::Warn,
            Level::Error => LoggingLevel::Error,
        }
    }
}
