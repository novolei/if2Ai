//! Skills module — skill discovery, management, and security scanning.
//!
//! This module provides the skill system including:
//! - SkillsGuard: threat scanner with 60+ patterns across 15 categories
//! - Skill management: CRUD operations for skill directories
//! - Skill hub: multi-source skill marketplace adapters
//! - Skill sync: manifest-based bundled skill synchronization
//! - Skill commands: slash command integration

pub mod guard;
pub mod manager;
