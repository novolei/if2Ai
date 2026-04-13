//! Desktop control-plane primitives.
//!
//! This module centralizes session context resolution and tool execution
//! orchestration so command handlers remain thin.

pub mod audit;
pub mod boundary_resolver;
pub mod session_context;
pub mod tool_execution_broker;

#[allow(unused_imports)]
pub use audit::AuditEmitter;
#[allow(unused_imports)]
pub use boundary_resolver::BoundaryResolver;
#[allow(unused_imports)]
pub use session_context::{SessionContextResolver, SessionExecutionContext};
#[allow(unused_imports)]
pub use tool_execution_broker::ToolExecutionBroker;
