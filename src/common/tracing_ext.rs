#![allow(unused)]
use tracing::Level;

pub(crate) trait TracingResultExt<R> {
    fn log_if_ok(self, level: Level, msg: fn(&R) -> Option<String>) -> Self;

    fn log_if_error(self, level: Level) -> Self;
}

impl<T> TracingResultExt<T> for anyhow::Result<T> {
    fn log_if_ok(self, level: Level, msg: fn(&T) -> Option<String>) -> Self {
        match &self {
            Ok(result) => {
                let message = msg(result);
                let message = match message {
                    Some(message) => message,
                    None => return self,
                };
                match level.as_str() {
                    "TRACE" => {
                        tracing::trace!("{}", message);
                    }
                    "DEBUG" => {
                        tracing::debug!("{}", message);
                    }
                    "INFO" => {
                        tracing::info!("{}", message);
                    }
                    "WARN" => {
                        tracing::warn!("{}", message);
                    }
                    "ERROR" => {
                        tracing::error!("{}", message);
                    }
                    _ => (),
                }
            }
            Err(_) => (),
        };
        self
    }

    fn log_if_error(self, level: Level) -> Self {
        match &self {
            Ok(_) => (),
            Err(err) => match level.as_str() {
                "TRACE" => {
                    err.log_as_trace();
                }
                "DEBUG" => {
                    err.log_as_debug();
                }
                "INFO" => {
                    err.log_as_info();
                }
                "WARN" => {
                    err.log_as_warn();
                }
                "ERROR" => {
                    err.log_as_error();
                }
                _ => (),
            },
        };
        self
    }
}

pub(crate) trait TracingErrorExt {
    fn log_as_trace(self) -> Self;

    fn log_as_debug(self) -> Self;

    fn log_as_info(self) -> Self;

    fn log_as_warn(self) -> Self;

    fn log_as_error(self) -> Self;
}

impl TracingErrorExt for &anyhow::Error {
    fn log_as_trace(self) -> Self {
        tracing::trace!("{self:?}");
        self
    }

    fn log_as_debug(self) -> Self {
        tracing::debug!("{self:?}");
        self
    }

    fn log_as_info(self) -> Self {
        tracing::info!("{self:?}");
        self
    }

    fn log_as_warn(self) -> Self {
        tracing::warn!("{self:?}");
        self
    }

    fn log_as_error(self) -> Self {
        tracing::error!("{self:?}");
        self
    }
}

impl TracingErrorExt for anyhow::Error {
    fn log_as_trace(self) -> Self {
        (&self).log_as_trace();
        self
    }

    fn log_as_debug(self) -> Self {
        (&self).log_as_debug();
        self
    }

    fn log_as_info(self) -> Self {
        (&self).log_as_info();
        self
    }

    fn log_as_warn(self) -> Self {
        (&self).log_as_warn();
        self
    }

    fn log_as_error(self) -> Self {
        (&self).log_as_error();
        self
    }
}
