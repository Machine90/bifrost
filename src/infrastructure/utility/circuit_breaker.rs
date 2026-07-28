use moka2::future::Cache;
use std::{
    fmt::Debug,
    hash::Hash,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

#[derive(Debug)]
pub struct CircuitBreaker<K: Hash + PartialEq + Eq + Send + Sync + 'static + Debug> {
    failure_counts: Cache<K, Arc<AtomicU32>>,
    allow_failure_counts: u32,
}

impl<K> CircuitBreaker<K>
where
    K: Hash + PartialEq + Eq + Send + Sync + 'static + Debug,
{
    pub fn new(cool_down_time: Duration, allow_failure_counts: u32) -> Self {
        Self {
            failure_counts: Cache::builder().time_to_live(cool_down_time).build(),
            allow_failure_counts,
        }
    }

    pub async fn report_failure(&self, target: K) {
        let target_info = format!("{target:?}");
        let failure_count = self
            .failure_counts
            .entry(target)
            .or_insert(Arc::new(AtomicU32::new(0)))
            .await
            .value()
            .fetch_add(1, Ordering::SeqCst);
        if failure_count == self.allow_failure_counts {
            tracing::warn!(
                "Circuit breaker tripped for {target_info}. Entering cool-down period (threshold: {}).",
                self.allow_failure_counts
            );
        }
    }

    pub async fn report_success(&self, target: K) {
        self.failure_counts.invalidate(&target).await;
    }

    pub async fn is_available(&self, target: &K) -> bool {
        let failure_count = self
            .failure_counts
            .get(target)
            .await
            .map(|v| v.load(Ordering::SeqCst))
            .unwrap_or_default();
        failure_count < self.allow_failure_counts
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use anyhow::Result;
    use tokio::time::sleep;

    use crate::infrastructure::utility::circuit_breaker::CircuitBreaker;

    #[tokio::test]
    async fn test_circuit_breaker() -> Result<()> {
        let banned_dur = Duration::from_secs(1);
        let allowed_failures = 3;
        let cb = Arc::new(CircuitBreaker::<&'static str>::new(
            banned_dur.clone(),
            allowed_failures,
        ));

        let mut tasks = vec![];
        for _ in 0..10 {
            let cb_for_failure = cb.clone();
            let t = tokio::spawn(async move {
                let called_cnt = if cb_for_failure.is_available(&"target").await {
                    1
                } else {
                    0
                };
                cb_for_failure.report_failure("target").await;
                called_cnt
            });
            tasks.push(t);
        }
        let mut approved = 0;
        for t in tasks {
            approved += t.await?;
        }
        assert_eq!(approved, allowed_failures);
        sleep(banned_dur).await;
        let recovery = cb.is_available(&"target").await;
        assert!(recovery);
        Ok(())
    }
}
