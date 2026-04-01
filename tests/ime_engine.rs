use suzaku_map::ime::{
    CommitOptions, CommitReason, EngineConfig, InputSource, Mode, SignalState, Warning,
    XRTabletImeEngine,
};

#[test]
fn builds_draft_candidates_from_seed_input() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");

    assert_eq!(snapshot.mode, Mode::Composing);
    assert_eq!(snapshot.seed_text, "ni hao");
    assert!(!snapshot.candidate_labels.is_empty());
    assert_eq!(snapshot.draft_text, snapshot.candidate_labels[0]);
}

#[test]
fn enters_degraded_mode_under_weak_signal_and_limits_candidates() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.update_signal(SignalState {
        pointer_precision: 0.2,
        gaze_stability: 0.2,
        host_intent_weight: 0.4,
        source_confidence: 0.4,
    });

    let snapshot = engine.seed("ni hao xr");

    assert!(snapshot.degraded);
    assert!(snapshot.candidate_labels.len() <= 3);
    assert!(snapshot.candidate_labels[0].contains("[stable]"));
    assert!(snapshot.warnings.contains(&Warning::DegradedMode));
}

#[test]
fn requires_confirmation_when_confidence_is_weak() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.update_signal(SignalState {
        pointer_precision: 0.2,
        gaze_stability: 0.2,
        host_intent_weight: 0.2,
        source_confidence: 0.2,
    });
    engine.seed("tablet ime");

    let result = engine.commit(CommitOptions::default());

    assert!(!result.ok);
    assert_eq!(result.reason, CommitReason::ConfirmationRequired);
}

#[test]
fn supports_forced_commit_and_one_step_undo() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.seed("xr tablet");

    let commit = engine.commit(CommitOptions { force: true });
    assert!(commit.ok);
    assert!(commit.text.unwrap().to_lowercase().contains("xr"));

    let undo = engine.undo().expect("undo snapshot");
    assert_eq!(undo.committed_text, "");
}

#[test]
fn keeps_source_and_selection_flow_visible_for_xr_hosts() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.set_source(InputSource::HandTracking);
    engine.seed("ni hao xr");
    let snapshot = engine.select_candidate(1);

    assert_eq!(snapshot.active_source, InputSource::HandTracking);
    assert_eq!(snapshot.selected_index, 1);
}

#[cfg(feature = "gpu")]
#[test]
fn gpu_scene_builder_marks_selected_candidate() {
    use suzaku_map::ime::gpu::WgpuCandidateRenderer;

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.seed("ni hao xr");
    let snapshot = engine.select_candidate(1);

    let renderer = WgpuCandidateRenderer::new(1024.0, 768.0);
    let scene = renderer.build_scene(&snapshot);

    assert_eq!(scene.quads.len(), snapshot.candidate_labels.len());
    assert_eq!(scene.labels[1], snapshot.candidate_labels[1]);
    assert_ne!(scene.quads[1].color, scene.quads[0].color);
}

#[cfg(feature = "gpu")]
#[test]
fn gpu_scene_builder_keeps_vertical_candidate_stack() {
    use suzaku_map::ime::gpu::WgpuCandidateRenderer;

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao xr");

    let renderer = WgpuCandidateRenderer::new(1280.0, 800.0);
    let scene = renderer.build_scene(&snapshot);

    assert!(scene.quads.len() >= 2);
    assert!(scene.quads[1].rect[1] > scene.quads[0].rect[1]);
    assert_eq!(
        scene.selected_label.as_deref(),
        Some(snapshot.candidate_labels[0].as_str())
    );
    assert_eq!(scene.draft_text, snapshot.draft_text);
}
