use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use chrono::Local;
use log::{LevelFilter, Log, Metadata, Record};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

struct CombinedLogger {
    file: Mutex<File>,
    level: LevelFilter,
}

impl Log for CombinedLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            
            // Write to file
            let mut file = self.file.lock().unwrap();
            writeln!(
                file,
                "[{} {} {}:{}] {}",
                timestamp,
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args()
            ).unwrap();
            file.flush().unwrap();

            // Write to console with colors
            let mut stdout = StandardStream::stdout(ColorChoice::Always);
            let color = match record.level() {
                log::Level::Error => Color::Red,
                log::Level::Warn => Color::Yellow,
                log::Level::Info => Color::Green,
                log::Level::Debug => Color::Blue,
                log::Level::Trace => Color::Cyan,
            };
            
            let console_timestamp = Local::now().format("%H:%M:%S");
            stdout.set_color(ColorSpec::new().set_fg(Some(color))).unwrap();
            writeln!(
                stdout,
                "[{} {} {}:{}] {}",
                console_timestamp,
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args()
            ).unwrap();
            stdout.reset().unwrap();
        }
    }

    fn flush(&self) {
        let mut file = self.file.lock().unwrap();
        file.flush().unwrap();
    }
}

use crate::config::{ConfigManager, WhisprConfig};

pub fn setup_logging() -> anyhow::Result<()> {
    // Load config to check if logging is enabled
    let config_manager = ConfigManager::<WhisprConfig>::new("settings")?;
    let config = if config_manager.config_exists("settings") {
        config_manager.load_config("settings")?
    } else {
        WhisprConfig::default()
    };

    let log_level = if config.developer.logging {
        LevelFilter::Debug
    } else {
        LevelFilter::Error
    };

    // Set up file logging
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let log_dir = home_dir.join(".whispr").join("logs");
    fs::create_dir_all(&log_dir)?;

    let log_file_path = log_dir.join(format!("whispr_{}.log", Local::now().format("%Y%m%d")));
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path)?;

    let logger = Box::new(CombinedLogger {
        file: Mutex::new(file),
        level: log_level,
    });

    log::set_boxed_logger(logger)?;
    log::set_max_level(log_level);

    Ok(())
}

pub fn get_log_dir() -> anyhow::Result<PathBuf> {
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    Ok(home_dir.join(".whispr").join("logs"))
}
