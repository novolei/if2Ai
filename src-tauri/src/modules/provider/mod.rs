//! Provider module — provider registry, connection testing, and model listing.
//!
//! Supports the onboarding Step 4 (Provider Setup):
//! - `list_providers()`: 14 builtin providers
//! - `test_provider_connection()`: Validates API connectivity
//! - `list_models()`: Fetches available models per provider
//! - `configure_provider()` / `select_model()`: Saves configuration

// allow(dead_code): this module is a service layer; commands/ will use it in a later slice.
// allow(unused_imports): re-exports are for future Tauri command consumers.
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod capabilities;
pub mod client;
pub mod known_models;
pub mod known_providers;
pub mod llm_provider;
pub mod registry;
pub mod resilience;
pub mod service;
pub mod test;
pub mod types;

// Re-export commonly used types
pub use registry::{builtin_providers, find_provider};
pub use service::{configure_provider, list_models, list_providers, select_model};
pub use test::test_provider_connection;
pub use types::{Model, ModelModality, Provider, ProviderCategory, ProviderStatus, TestResult};
