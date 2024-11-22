import 'dart:io';
import 'package:roxy/backend/api/utils/logger.dart';
import 'package:roxy/main.dart';
import 'package:stack_trace/stack_trace.dart';

const kDefaultTag = 'Roxy-Dart';
const kRustTag = 'Roxy-Rust';

/// A simple logging utility for Roxy that supports different log levels and formatting.
///
/// The logger provides methods for logging at different severity levels (trace, debug, info, etc)
/// and includes helpful context like timestamps, tags, and file/line information.
///
/// Example usage:
/// ```dart
/// DLogger.d("Processing request"); // Debug log with auto file info
/// DLogger.log("Custom message", LoggingLevel.info,
///   tag: "MyTag",
///   time: DateTime.now(),
///   fileInfo: "my_file.dart:42"
/// ); // Fully customized log
/// ```
///
/// The maximum logging level can be configured:
/// ```dart
/// DLogger.maxLevel = LoggingLevel.debug; // Only show debug and above
/// ```
///
/// Log levels from highest to lowest severity:
/// - error
/// - warn
/// - info
/// - debug
/// - trace
///
/// By default, logs are written to stdout with formatting:
/// `[TAG] TIME LEVEL FILE - Message`

class DLogger {
  static bool _initialized = false;

  /// The maximum level of logging to output.
  static LoggingLevel _level = kMaxLoggingLevel;

  static set maxLevel(LoggingLevel level) => _level = level;

  /// Initializes the logging system by setting up a stream to receive logs from Rust.
  ///
  /// This function should be called once at app startup. It sets up a listener for
  /// log messages coming from the Rust backend and forwards them to the Dart logging
  /// system with appropriate formatting.
  ///
  /// The log messages from Rust will:
  /// - Use [kRustTag] as the tag to distinguish them from Dart logs
  /// - Include the original file info from Rust
  /// - Convert the timestamp from microseconds since epoch to DateTime
  /// - Forward any errors during setup to the error log
  ///
  /// If already initialized, this function will return early to prevent duplicate initialization.
  static void init() {
    if (_initialized) return;
    // Setup logging
    setupLogStream(level: kMaxLoggingLevel).listen(
      (entry) => DLogger.log(
        entry.msg,
        entry.level,
        tag: kRustTag,
        fileInfo: entry.fileInfo,
        time: DateTime.fromMicrosecondsSinceEpoch(
          entry.microsSinceEpoch.toInt(),
          isUtc: true,
        ),
      ),
      onError: (e) => DLogger.e("Error setting up log stream: $e"),
    );
    _initialized = true;
  }

  /// Logs a message with the given level, tag, time, and file info.
  static void log(String msg, LoggingLevel level,
      {String? tag, DateTime? time, String? fileInfo}) {
    final outputString =
        '${_formatTag(tag ?? kDefaultTag)} ${_formatTime(time)} ${_formatLevel(level)} ${_formatFileInfo(fileInfo)} - $msg';
    stdout.writeln(outputString);
    stdout.flush();
  }

  static String _getCallerInfo() {
    final frames = Trace.current().frames;

    // Skip frames related to the logging system itself
    for (var frame in frames.skip(1)) {
      if (!frame.library.contains('logging.dart')) {
        // Join all path segments after 'lib' to get the full relative path
        final pathSegments = frame.uri.pathSegments;
        final libIndex = pathSegments.indexOf('lib');
        if (libIndex >= 0) {
          final relativePath = pathSegments.sublist(libIndex + 1).join('/');
          return '$relativePath:${frame.line}';
        }
        return '${frame.uri.pathSegments.last}:${frame.line}';
      }
    }
    return '';
  }

  /// Logs a trace message.
  static void t(String msg) {
    if (_level.index == LoggingLevel.trace.index) {
      DLogger.log(msg, LoggingLevel.trace, fileInfo: _getCallerInfo());
    }
  }

  /// Logs a debug message.
  static void d(String msg) {
    if (_level.index >= LoggingLevel.debug.index) {
      DLogger.log(msg, LoggingLevel.debug, fileInfo: _getCallerInfo());
    }
  }

  /// Logs an info message.
  static void i(String msg) {
    if (_level.index >= LoggingLevel.info.index) {
      DLogger.log(msg, LoggingLevel.info, fileInfo: _getCallerInfo());
    }
  }

  /// Logs a warning message.
  static void w(String msg) {
    if (_level.index >= LoggingLevel.warn.index) {
      DLogger.log(msg, LoggingLevel.warn, fileInfo: _getCallerInfo());
    }
  }

  /// Logs an error message.
  static void e(String msg) {
    if (_level.index >= LoggingLevel.error.index) {
      DLogger.log(msg, LoggingLevel.error, fileInfo: _getCallerInfo());
    }
  }

  static String _formatLevel(LoggingLevel level) {
    final levelStr = level.name.toUpperCase();
    switch (level) {
      case LoggingLevel.trace:
        return '\x1B[35m$levelStr\x1B[0m'; // Purple (Magenta)
      case LoggingLevel.debug:
        return '\x1B[36m$levelStr\x1B[0m'; // Cyan
      case LoggingLevel.info:
        return '\x1B[35m$levelStr\x1B[0m'; // Purple (Magenta)
      case LoggingLevel.warn:
        return '\x1B[38;5;208m$levelStr\x1B[0m'; // Orange (using 8-bit color)
      case LoggingLevel.error:
        return '\x1B[31m$levelStr\x1B[0m'; // Red
    }
  }

  /// Formats the time to a ISO8601 string with a gray color.
  static String _formatTime(DateTime? time) {
    var adjustedTime = time ?? DateTime.now().toUtc();
    return '\x1B[90m[${adjustedTime.toIso8601String()}]\x1B[0m';
  }

  /// Formats the tag to a string with a orange color.
  static String _formatTag(String tag) => '\x1B[1m\x1B[38;5;208m[$tag]\x1B[0m';

  /// Formats the file info to a string with a gray color.
  static String _formatFileInfo(String? fileInfo) {
    if (fileInfo == null || fileInfo.isEmpty) return '';
    return '\x1B[90m($fileInfo)\x1B[0m';
  }
}
