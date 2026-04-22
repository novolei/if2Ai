//! Identity domain primitives for Soul / Persona resolution.
//!
//! This module establishes the durable identity layer used by later
//! prompt/session/memory packs:
//! - [`definition`] owns the serializable domain types
//! - [`registry`] exposes the built-in identity catalog
//! - [`resolver`] computes the effective identity for one turn
//! - [`settings`] stores global/session override settings

pub mod custom_pack;
pub mod definition;
pub mod prompt;
pub mod registry;
pub mod resolver;
pub mod settings;

#[allow(unused_imports)]
pub use custom_pack::{
    apply_identity_customization_pack, default_identity_customization_pack_path,
    normalize_identity_customization_pack, persona_has_customization,
    read_identity_customization_pack, soul_has_customization, write_identity_customization_pack,
    IdentityCustomizationPack, PersonaCustomization, SoulCustomization,
};
#[allow(unused_imports)]
pub use definition::{IdentitySource, PersonaDefinition, ResolvedIdentity, SoulDefinition};
#[allow(unused_imports)]
pub use prompt::{render_identity_naming_block, render_persona_block, render_soul_block};
#[allow(unused_imports)]
pub use registry::IdentityRegistry;
#[allow(unused_imports)]
pub use resolver::{resolve_identity, IdentityResolution};
#[allow(unused_imports)]
pub use settings::{
    default_prompt_control_settings_path, read_identity_naming_settings, IdentityNamingSettings,
    IdentitySettings, SessionIdentityOverride,
};
