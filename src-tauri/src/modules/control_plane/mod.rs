//! Desktop control-plane primitives.
//!
//! This module centralizes session context resolution and tool execution
//! orchestration so command handlers remain thin.

pub mod audit;
pub mod boundary_resolver;
pub mod ingress_classifier;
pub mod prepare_step_execution;
pub mod session_context;
pub mod tool_execution_broker;

#[allow(unused_imports)]
pub use audit::AuditEmitter;
#[allow(unused_imports)]
pub use boundary_resolver::BoundaryResolver;
#[allow(unused_imports)]
pub use ingress_classifier::{
    classify_request as classify_ingress, IngressClassifierInput, IngressClassifierOutput,
    CLASSIFIER_POLICY_VERSION,
};
#[allow(unused_imports)]
pub use prepare_step_execution::{
    prepare_step_execution, BoundaryDecision, PermissionDecision, PrepareStepExecutionInput,
    PrepareStepExecutionOutput, PrepareStepOutcome, SandboxPolicy, PREPARE_STEP_POLICY_VERSION,
};
#[allow(unused_imports)]
pub use session_context::{SessionContextResolver, SessionExecutionContext};
#[allow(unused_imports)]
pub use tool_execution_broker::ToolExecutionBroker;
