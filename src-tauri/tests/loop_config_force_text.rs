//! Verifies that StreamTaskInputs carries an AgenticLoopConfig field and the
//! type re-export is reachable from the public crate path.
//!
//! Behavioural force_text testing for the actual stream loop is exercised in
//! turn_service_stream_turn_e2e once StreamDelegate lands in S5. For S2-S1b
//! we assert (1) the type is on TurnServiceDeps and StreamTaskInputs, and
//! (2) the default config has force_text_after_truncations == 2.

use if2ai_backend::modules::application::turn_service::AgenticLoopConfig;

#[test]
fn config_default_force_text_threshold_is_two() {
    let cfg = AgenticLoopConfig::default();
    assert_eq!(cfg.force_text_after_truncations, 2);
    assert_eq!(cfg.max_iterations, 50);
}

// Compile-time witness: the field must exist on StreamTaskInputs. If the field
// is removed or renamed, this test fails to compile.
#[allow(dead_code)]
fn _stream_task_inputs_has_loop_config(
    inputs: &if2ai_backend::modules::application::turn_service::stream_task::StreamTaskInputs,
) -> &AgenticLoopConfig {
    &inputs.loop_config
}

// Compile-time witness for TurnServiceDeps too.
#[allow(dead_code)]
fn _turn_service_deps_has_loop_config(
    deps: &if2ai_backend::modules::application::turn_service::TurnServiceDeps,
) -> &AgenticLoopConfig {
    &deps.loop_config
}
