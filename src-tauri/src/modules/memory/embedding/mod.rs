//! Embedding provider modules
//!
//! - [`FastEmbedProvider`] — Offline text embedding with FastEmbed (384d)

pub mod fastembed;
pub mod mock;

pub use fastembed::FastEmbedProvider;
#[allow(unused_imports)] // Re-exported for tests in hrr/integration.rs and others.
pub use mock::MockEmbedder;
