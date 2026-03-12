use std::env;

use tracing::info;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::writer::MakeWriterExt;

pub fn init() {
    // Configure logging
    if env::var("ENV") != Ok("testing".to_string()) {
        let stdout = std::io::stdout.with_filter(|meta| meta.target() == "app");
        let debug_file = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("info")
            .filename_suffix("log")
            .max_log_files(5)
            .build("./logs")
            .expect("initializing rolling info_file appender failed")
            .with_max_level(tracing::Level::INFO);
        let error_file = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("error")
            .filename_suffix("log")
            .max_log_files(5)
            .build("./logs")
            .expect("initializing rolling error_file appender failed")
            .with_filter(|meta| meta.target() == "app")
            .with_max_level(tracing::Level::ERROR);
        let writer = debug_file.and(error_file).and(stdout);
        tracing_subscriber::fmt()
            .compact()
            .with_writer(writer)
            .with_ansi(false)
            .with_max_level(tracing::Level::DEBUG)
            .init();
    }

    info!(target: "app", "Starting application...");
}
