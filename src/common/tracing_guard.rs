use std::any::Any;
use std::path::{Path, PathBuf};

use anyhow::{Ok, Result};
pub use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_error::ErrorLayer;
use tracing_subscriber::Layer as _;
use tracing_subscriber::fmt::time::LocalTime;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

#[derive(Debug, Clone, Copy)]
pub enum RollingStrategy {
    Never,
    Hourly,
    Daily,
    Weekly,
}

#[derive(Debug, Clone)]
pub struct LogFile {
    /// Log content level not lower than this level
    pub level_filter: LevelFilter,
    /// Log file rolling strategy, only worked for directory type path.
    pub strategy: RollingStrategy,
    /// This path can be a directory or a specified file path, if this is
    /// a directory, then we will create log files by using strategy, otherwise
    /// we only write log content into the given file.
    pub path: PathBuf,
}

impl Default for LogFile {
    fn default() -> Self {
        Self {
            level_filter: LevelFilter::INFO,
            strategy: RollingStrategy::Never,
            path: Path::new("./").into(),
        }
    }
}

impl LogFile {
    pub fn log_writer(self) -> Result<(NonBlocking, WorkerGuard)> {
        let Self {
            level_filter,
            path,
            strategy,
        } = self;
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }
        let level = level_filter
            .clone()
            .into_level()
            .map(|l| l.to_string())
            .unwrap_or("unknown".to_string());
        let log_file_name_prefix = format!("{strategy:?}-{level}");
        let writer = if path.is_file() {
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or(log_file_name_prefix);
            let parent_path = path
                .parent()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or("./".to_string());
            tracing_appender::rolling::never(parent_path, file_name)
        } else {
            match strategy {
                RollingStrategy::Never => {
                    tracing_appender::rolling::never(path, format!("{level}.log"))
                }
                RollingStrategy::Hourly => {
                    tracing_appender::rolling::hourly(path, log_file_name_prefix)
                }
                RollingStrategy::Daily => {
                    tracing_appender::rolling::daily(path, log_file_name_prefix)
                }
                RollingStrategy::Weekly => {
                    tracing_appender::rolling::weekly(path, log_file_name_prefix)
                }
            }
        };
        Ok(tracing_appender::non_blocking(writer))
    }
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub project_name: String,
    pub release: Option<String>,
    pub environment: String,
    pub sentry_dsn: Option<String>,
    pub sentry_sample_rate: f32,
    pub sentry_traces_sample_rate: f32,
    pub console_log_level: Option<LevelFilter>,
    /// Specific log file if attempt to write logs to local file,
    /// this path can be a directory or a file.
    pub log_file: Option<LogFile>,
}

impl Default for Settings {
    fn default() -> Self {
        #[allow(unused)]
        let release: Option<String> = None;
        #[cfg(feature = "sentry-tracing")]
        let release = sentry::release_name!().map(|c| c.to_string());
        let default_project_name = release
            .as_ref()
            .map(|n| n.split('@').next())
            .flatten()
            .map(ToString::to_string);
        Self {
            project_name: default_project_name.unwrap_or("unknown".to_string()),
            release,
            environment: "dev".to_string(),
            sentry_dsn: None,
            sentry_sample_rate: 0.01,
            sentry_traces_sample_rate: 0.001,
            console_log_level: Some(LevelFilter::INFO),
            log_file: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct LoggingGuard {
    guards: Vec<Box<dyn Any>>,
}

/// setup tracing for your project, please notes that, holding returns
/// `LoggingGuard` to your project if you specified sentry dsn.
///
/// ## Example
/// ```rust
/// let _guard = setup_tracing(Settings::default()).expected("Failed to setup tracing");
/// // do something under the logging guard
///
/// ```
#[allow(unused_variables)]
pub fn setup_tracing(settings: Settings) -> Result<LoggingGuard> {
    let Settings {
        project_name,
        release,
        environment,
        sentry_dsn,
        sentry_sample_rate,
        sentry_traces_sample_rate,
        console_log_level,
        log_file,
    } = settings;
    let mut logging_guard = LoggingGuard::default();
    let subscriber = tracing_subscriber::registry()
        .with(ErrorLayer::default())
        .with(if let Some(level) = console_log_level {
            let layer = tracing_subscriber::fmt::Layer::new()
                .pretty()
                .with_filter(level);
            Some(layer)
        } else {
            None
        });
    let subscriber = subscriber.with(if let Some(log_file) = log_file {
        let level = log_file.level_filter;
        let (non_blocking, guard) = log_file.log_writer()?;
        let layer = tracing_subscriber::fmt::Layer::new()
            .with_ansi(false)
            .with_line_number(true)
            .with_writer(non_blocking)
            .with_timer(LocalTime::rfc_3339())
            .with_filter(level);
        logging_guard.guards.push(Box::new(guard));
        Some(layer)
    } else {
        None
    });
    #[cfg(feature = "sentry-tracing")]
    {
        use anyhow::Context;
        use opentelemetry::global;
        use opentelemetry::trace::TracerProvider as _;
        use opentelemetry_sdk::trace::SdkTracerProvider;
        use sentry::integrations::tracing::EventFilter;
        use std::str::FromStr;

        let subscriber = subscriber
            .with({
                let tracer_provider = SdkTracerProvider::builder().build();
                let tracer = tracer_provider.tracer(project_name.to_string());
                global::set_tracer_provider(tracer_provider);
                let layer = tracing_opentelemetry::layer()
                    .with_location(true)
                    .with_tracked_inactivity(true)
                    .with_error_fields_to_exceptions(true)
                    .with_error_records_to_exceptions(true)
                    .with_threads(true)
                    .with_tracer(tracer);
                layer.boxed()
            })
            .with(if let Some(dsn) = sentry_dsn {
                let (layer, guard) = (|| {
                    let sentry_opts = sentry::ClientOptions {
                        dsn: Some(sentry::types::Dsn::from_str(&dsn)?),
                        debug: false,
                        release: release.map(Into::into).or(sentry::release_name!()),
                        environment: Some(environment.to_string().into()),
                        sample_rate: sentry_sample_rate,
                        traces_sample_rate: sentry_traces_sample_rate,
                        ..Default::default()
                    };
                    let sentry_guard = sentry::init(sentry_opts);
                    let layer = sentry::integrations::tracing::layer().event_filter(|_| {
                        return EventFilter::Breadcrumb;
                    });
                    anyhow::Ok((layer.boxed(), sentry_guard))
                })()
                .context("Failed to init sentry")?;
                logging_guard.guards.push(Box::new(guard));
                Some(layer)
            } else {
                None
            });
        subscriber.init();
    }
    #[cfg(not(feature = "sentry-tracing"))]
    {
        subscriber.init();
    }
    Ok(logging_guard)
}
