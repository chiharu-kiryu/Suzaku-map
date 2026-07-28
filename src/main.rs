use suzaku_map::ime::{
    CommitOptions, CommitResult, EngineConfig, InputSource, SignalState, Snapshot, XRTabletImeEngine,
};

#[derive(Debug, Clone)]
struct DemoRun {
    initial: Snapshot,
    after_signal: Snapshot,
    after_seed: Snapshot,
    after_selection: Snapshot,
    default_commit: CommitResult,
    forced_commit: CommitResult,
    undo: Option<Snapshot>,
}

fn main() {
    let DemoRun {
        initial,
        after_signal,
        after_seed,
        after_selection,
        default_commit,
        forced_commit,
        undo,
    } = run_demo();

    let mut output = String::new();
    print_demo_run(
        &mut output,
        &DemoRun {
            initial,
            after_signal,
            after_seed,
            after_selection,
            default_commit,
            forced_commit,
            undo,
        },
    );
    print!("{output}");
}

fn run_demo() -> DemoRun {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let initial = engine.snapshot();

    engine.set_source(InputSource::GazeDwell);
    engine.update_signal(SignalState {
        pointer_precision: 0.42,
        gaze_stability: 0.50,
        host_intent_weight: 0.80,
        source_confidence: 0.70,
    });
    let after_signal = engine.snapshot();

    let after_seed = engine.seed("ni hao xr");
    let after_selection = engine.move_selection(1);
    let default_commit = engine.commit(CommitOptions::default());
    let forced_commit = engine.commit(CommitOptions { force: true });
    let undo = engine.undo();

    DemoRun {
        initial,
        after_signal,
        after_seed,
        after_selection,
        default_commit,
        forced_commit,
        undo,
    }
}

fn print_demo_run<W: std::fmt::Write>(
    out: &mut W,
    DemoRun {
        initial,
        after_signal,
        after_seed,
        after_selection,
        default_commit,
        forced_commit,
        undo,
    }: &DemoRun,
) {
    writeln!(out, "initial: {:#?}", initial).unwrap();
    writeln!(out, "after signal: {:#?}", after_signal).unwrap();
    writeln!(out, "after seed: {:#?}", after_seed).unwrap();
    writeln!(out, "after select: {:#?}", after_selection).unwrap();
    writeln!(out, "commit without force: {:#?}", default_commit).unwrap();
    writeln!(out, "commit with force: {:#?}", forced_commit).unwrap();
    writeln!(out, "undo: {:#?}", undo).unwrap();
}

#[cfg(test)]
mod tests {
    use suzaku_map::ime::CommitReason;

    #[test]
    fn run_demo_behaves_like_ime_workflow() {
        let demo = super::run_demo();

        assert!(matches!(demo.initial.mode, suzaku_map::ime::Mode::Idle));
        assert!(matches!(demo.after_signal.mode, suzaku_map::ime::Mode::Idle));
        assert!(matches!(demo.after_seed.mode, suzaku_map::ime::Mode::Composing));
        assert!(demo.after_seed.seed_text.starts_with("ni hao"));
        assert_eq!(demo.after_selection.selected_index, 1);
        assert_eq!(demo.default_commit.reason, CommitReason::ConfirmationRequired);
        assert!(demo.forced_commit.ok);
        assert!(demo.forced_commit.text.is_some());
    }

    #[test]
    fn print_demo_run_includes_all_sections() {
        let demo = super::run_demo();
        let mut output = String::new();

        super::print_demo_run(&mut output, &demo);

        assert!(output.contains("initial:"));
        assert!(output.contains("after signal:"));
        assert!(output.contains("after seed:"));
        assert!(output.contains("after select:"));
        assert!(output.contains("commit without force:"));
        assert!(output.contains("commit with force:"));
        assert!(output.contains("undo:"));
    }
}
