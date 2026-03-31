use suzaku_map::ime::{CommitOptions, EngineConfig, InputSource, SignalState, XRTabletImeEngine};

fn main() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());

    println!("initial: {:#?}", engine.snapshot());

    engine.set_source(InputSource::GazeDwell);
    engine.update_signal(SignalState {
        pointer_precision: 0.42,
        gaze_stability: 0.50,
        host_intent_weight: 0.80,
        source_confidence: 0.70,
    });

    println!("after signal: {:#?}", engine.snapshot());
    println!("after seed: {:#?}", engine.seed("ni hao xr"));
    println!("after select: {:#?}", engine.move_selection(1));
    println!(
        "commit without force: {:#?}",
        engine.commit(CommitOptions::default())
    );
    println!(
        "commit with force: {:#?}",
        engine.commit(CommitOptions { force: true })
    );
    println!("undo: {:#?}", engine.undo());
}
