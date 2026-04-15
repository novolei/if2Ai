//! Embedding provider modules
//!
//! - [`FastEmbedProvider`] — Offline text embedding with FastEmbed (384d)

pub mod fastembed;

pub use fastembed::FastEmbedProvider;
