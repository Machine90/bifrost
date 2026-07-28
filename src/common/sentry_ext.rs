#![allow(unused)]

pub trait SentryError {
    fn report_sentry(self) -> Self;
    fn report_sentry_with_detail(self, with_detail: bool) -> Self;
}

#[cfg(feature = "sentry-tracing")]
fn capture_sentry_events(
    error_type: String,
    mut events: sentry::protocol::Event<'static>,
    with_detail: bool,
) {
    events.exception.iter_mut().last().map(|exp| {
        exp.ty = error_type;
    });
    if !with_detail {
        events.exception.iter_mut().for_each(|exp| {
            let _ = exp.raw_stacktrace.take();
            let _ = exp.stacktrace.take();
        });
    }
    sentry::capture_event(events);
}

/// Extension for anyhow Result
impl SentryError for anyhow::Error {
    fn report_sentry(self) -> Self {
        (&self).report_sentry();
        self
    }

    fn report_sentry_with_detail(self, with_detail: bool) -> Self {
        (&self).report_sentry_with_detail(with_detail);
        self
    }
}

impl SentryError for &anyhow::Error {
    fn report_sentry(self) -> Self {
        self.report_sentry_with_detail(false)
    }

    fn report_sentry_with_detail(self, with_detail: bool) -> Self {
        #[cfg(feature = "sentry-tracing")]
        {
            let error_type = self.to_string();
            let mut events = sentry_anyhow::event_from_error(self);
            capture_sentry_events(error_type, events, with_detail);
        }
        #[cfg(not(feature = "sentry-tracing"))]
        {
            tracing::debug!("Sentry feature is disable");
        }
        self
    }
}

impl<T> SentryError for anyhow::Result<T> {
    fn report_sentry(self) -> Self {
        match &self {
            Err(err) => {
                err.report_sentry_with_detail(false);
            }
            _ => (),
        };
        self
    }

    fn report_sentry_with_detail(self, with_detail: bool) -> Self {
        match &self {
            Err(err) => {
                err.report_sentry_with_detail(with_detail);
            }
            _ => (),
        };
        self
    }
}

/// Extension for std io error
impl SentryError for std::io::Error {
    fn report_sentry(self) -> Self {
        (&self).report_sentry_with_detail(false);
        self
    }

    fn report_sentry_with_detail(self, with_detail: bool) -> Self {
        (&self).report_sentry_with_detail(with_detail);
        self
    }
}

impl SentryError for &std::io::Error {
    fn report_sentry(self) -> Self {
        #[cfg(feature = "sentry-tracing")]
        {
            sentry::capture_error(self);
        }
        #[cfg(not(feature = "sentry-tracing"))]
        {
            tracing::debug!("Sentry feature is disable");
        }
        self
    }

    fn report_sentry_with_detail(self, with_detail: bool) -> Self {
        #[cfg(feature = "sentry-tracing")]
        {
            let error_type = self.to_string();
            let mut events = sentry::event_from_error(self);
            capture_sentry_events(error_type, events, with_detail);
        }
        #[cfg(not(feature = "sentry-tracing"))]
        {
            tracing::debug!("Sentry feature is disable");
        }
        self
    }
}

impl<T> SentryError for std::io::Result<T> {
    fn report_sentry(self) -> Self {
        match &self {
            Err(err) => {
                err.report_sentry_with_detail(false);
            }
            _ => (),
        };
        self
    }

    fn report_sentry_with_detail(self, with_detail: bool) -> Self {
        match &self {
            Err(err) => {
                err.report_sentry_with_detail(with_detail);
            }
            _ => (),
        };
        self
    }
}
