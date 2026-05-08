//! Stub `TrajectorySource` — honest no-op until Wave A.3 ships a real
//! `evolution::Trajectory` producer in turn_service.

use async_trait::async_trait;

use crate::modules::memory::evolution::trajectory::Trajectory;

use super::engine::TrajectorySource;

pub struct EmptyTrajectorySource;

#[async_trait]
impl TrajectorySource for EmptyTrajectorySource {
    async fn recent_trajectories(&self, _max: usize) -> Result<Vec<Trajectory>, String> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_source_returns_empty_vec() {
        let s = EmptyTrajectorySource;
        let v = s.recent_trajectories(100).await.unwrap();
        assert!(v.is_empty());
    }
}
