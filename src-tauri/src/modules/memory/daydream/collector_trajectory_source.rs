//! Production `TrajectorySource` — delegates to the in-memory
//! `TrajectoryCollector` shipped in `evolution::trajectory`.
//!
//! Replaces `EmptyTrajectorySource` once turn_service starts feeding
//! the collector with real `start_trajectory` / `record_turn` /
//! `finish_trajectory` calls (A.3).

use std::sync::Arc;

use async_trait::async_trait;

use crate::modules::memory::evolution::trajectory::{Trajectory, TrajectoryCollector};

use super::engine::TrajectorySource;

pub struct CollectorTrajectorySource {
    collector: Arc<TrajectoryCollector>,
}

impl CollectorTrajectorySource {
    pub fn new(collector: Arc<TrajectoryCollector>) -> Self {
        Self { collector }
    }
}

#[async_trait]
impl TrajectorySource for CollectorTrajectorySource {
    async fn recent_trajectories(&self, max: usize) -> Result<Vec<Trajectory>, String> {
        Ok(self.collector.recent_trajectories(max).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::evolution::trajectory::TaskOutcome;

    #[tokio::test]
    async fn empty_collector_returns_empty() {
        let collector = Arc::new(TrajectoryCollector::new());
        let source = CollectorTrajectorySource::new(collector);
        let v = source.recent_trajectories(10).await.unwrap();
        assert!(v.is_empty());
    }

    #[tokio::test]
    async fn finished_trajectory_visible_to_source() {
        let collector = Arc::new(TrajectoryCollector::new());
        collector.start_trajectory("s1", "test task").await;
        collector
            .finish_trajectory("s1", TaskOutcome::Success { quality_score: 1.0 })
            .await;
        let source = CollectorTrajectorySource::new(collector);
        let v = source.recent_trajectories(10).await.unwrap();
        assert_eq!(v.len(), 1);
        assert!(matches!(v[0].outcome, Some(TaskOutcome::Success { .. })));
    }
}
