//! Phase TTS-B.4: TTS provider idle eviction。
//!
//! ONNX sessions 加载后常驻 ~1.5GB RAM。在桌面应用里，TTS 不是热路径——用户
//! 用完就空置。本模块提供后台 ticker 定期检查"上次合成时间"，超过阈值则
//! drop provider 并触发 unload，下次请求再 lazy reload。
//!
//! ## 用法
//!
//! ```ignore
//! let provider_slot: Arc<RwLock<Option<Arc<dyn TtsProvider>>>> = Arc::new(RwLock::new(None));
//! let last_use: Arc<AtomicI64> = Arc::new(AtomicI64::new(0));
//! IdleEvictor::new(IdleEvictionConfig::default())
//!     .spawn(provider_slot.clone(), last_use.clone());
//! ```
//!
//! 当请求到达时：(1) `last_use.store(Instant::now().elapsed_as_secs())`；
//! (2) 如果 provider_slot 是 None → 重新构造 provider 写入 slot。

#![allow(dead_code)]

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use crate::modules::tts::TtsProvider;

/// Idle eviction 配置。
#[derive(Debug, Clone)]
pub struct IdleEvictionConfig {
    /// 多久无请求触发 unload；默认 5 分钟。
    pub idle_threshold: Duration,
    /// ticker 检查间隔；默认 30 秒。
    pub check_interval: Duration,
    /// 是否启用（false 时 spawn 立即返回不起 ticker）。
    pub enabled: bool,
}

impl Default for IdleEvictionConfig {
    fn default() -> Self {
        Self {
            idle_threshold: Duration::from_secs(300),
            check_interval: Duration::from_secs(30),
            enabled: true,
        }
    }
}

/// Idle evictor 句柄；drop 时 ticker 自动停止。
pub struct IdleEvictor {
    cancel: Arc<tokio::sync::Notify>,
    pub config: IdleEvictionConfig,
}

impl IdleEvictor {
    /// 启动后台 ticker。`last_use_millis` 是单调时钟下次活跃的毫秒数（由调用方
    /// 在每次合成时刷新为 `boot_instant.elapsed().as_millis() as i64`）。
    ///
    /// `on_evict` 是 evict 触发时的同步回调（在 evictor 私有 runtime 内调用），
    /// 调用方可以借此把 ProviderState 翻成 Evicted。
    pub fn spawn(
        config: IdleEvictionConfig,
        provider_slot: Arc<RwLock<Option<Arc<dyn TtsProvider>>>>,
        last_use_millis: Arc<AtomicI64>,
        boot: Instant,
        on_evict: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
    ) -> Self {
        let cancel = Arc::new(tokio::sync::Notify::new());
        let evictor = Self {
            cancel: cancel.clone(),
            config: config.clone(),
        };
        if !config.enabled {
            tracing::info!("TTS idle eviction disabled by config");
            return evictor;
        }
        // 不能直接用 `tokio::spawn`：main.rs 在 Tauri builder setup 阶段就会
        // 调到这里，此时还没有进入 Tokio runtime 上下文，会 panic:
        // "there is no reactor running".
        //
        // 解决：IdleEvictor 自己起一个 OS 线程，并在该线程内创建一个独立的
        // current-thread Tokio runtime，只跑这个 ticker。
        std::thread::Builder::new()
            .name("tts-idle-evictor".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_time()
                    .build();
                let Ok(rt) = runtime else {
                    tracing::error!("failed to build tokio runtime for TTS idle evictor");
                    return;
                };
                rt.block_on(async move {
                    tracing::info!(
                        idle_threshold_ms = config.idle_threshold.as_millis() as u64,
                        check_interval_ms = config.check_interval.as_millis() as u64,
                        "TTS idle eviction ticker started"
                    );
                    loop {
                        tokio::select! {
                            _ = tokio::time::sleep(config.check_interval) => {}
                            _ = cancel.notified() => {
                                tracing::info!("TTS idle eviction ticker stopped");
                                return;
                            }
                        }
                        let now_ms = boot.elapsed().as_millis() as i64;
                        let last = last_use_millis.load(Ordering::Relaxed);
                        if last <= 0 {
                            continue; // 未被使用过 / 未初始化
                        }
                        let idle_ms = now_ms.saturating_sub(last);
                        if idle_ms < config.idle_threshold.as_millis() as i64 {
                            continue;
                        }
                        let mut guard = provider_slot.write().await;
                        if guard.is_some() {
                            tracing::info!(
                                idle_ms,
                                "TTS idle threshold reached; evicting provider (release RAM)"
                            );
                            *guard = None;
                            if let Some(cb) = on_evict.as_ref() {
                                cb();
                            }
                        }
                    }
                });
            })
            .expect("failed to spawn tts-idle-evictor thread");
        evictor
    }

    /// 主动停止 ticker（drop 时也会触发，但显式更可控）。
    pub fn stop(&self) {
        self.cancel.notify_waiters();
    }
}

impl Drop for IdleEvictor {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn evictor_drops_provider_after_threshold() {
        // 启用 enabled=true，但 threshold/check 都很短，便于 unit test。
        let config = IdleEvictionConfig {
            idle_threshold: Duration::from_millis(100),
            check_interval: Duration::from_millis(50),
            enabled: true,
        };
        // 用 mock provider 占位
        let mock: Arc<dyn TtsProvider> =
            Arc::new(crate::modules::tts::provider::mock::MockTtsProvider::default());
        let slot = Arc::new(RwLock::new(Some(mock)));
        let boot = Instant::now();
        let last_use = Arc::new(AtomicI64::new(0));
        // 让 boot.elapsed() 累积一点，避免 last_use=0 被当作"未使用"
        tokio::time::sleep(Duration::from_millis(20)).await;
        last_use.store(
            (boot.elapsed().as_millis() as i64).max(1),
            Ordering::Relaxed,
        );

        let _evictor = IdleEvictor::spawn(config, slot.clone(), last_use.clone(), boot, None);

        tokio::time::sleep(Duration::from_millis(800)).await;
        let guard = slot.read().await;
        assert!(guard.is_none(), "provider should have been evicted");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn evictor_keeps_provider_when_active() {
        let config = IdleEvictionConfig {
            idle_threshold: Duration::from_millis(500),
            check_interval: Duration::from_millis(50),
            enabled: true,
        };
        let mock: Arc<dyn TtsProvider> =
            Arc::new(crate::modules::tts::provider::mock::MockTtsProvider::default());
        let slot = Arc::new(RwLock::new(Some(mock)));
        let boot = Instant::now();
        let last_use = Arc::new(AtomicI64::new(0));
        let _evictor = IdleEvictor::spawn(config, slot.clone(), last_use.clone(), boot, None);

        // 持续 600ms，每 100ms 打一次卡
        for _ in 0..6 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            last_use.store(boot.elapsed().as_millis() as i64, Ordering::Relaxed);
        }

        let guard = slot.read().await;
        assert!(
            guard.is_some(),
            "provider should NOT be evicted while active"
        );
    }
}
