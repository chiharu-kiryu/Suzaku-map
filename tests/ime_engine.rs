use std::sync::Arc;

use suzaku_map::ime::{
    Candidate, CommitOptions, CommitReason, EngineConfig, InputSource, LanguagePlugin, Mode,
    SignalState, Warning, XRTabletImeEngine,
};
use suzaku_map::languages::llm::{
    LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmLanguagePlugin,
};

#[path = "ime_engine_core.rs"]
mod ime_engine_core;
#[path = "ime_engine_gpu_scene_a.rs"]
mod ime_engine_gpu_scene_a;
#[path = "ime_engine_gpu_scene_b.rs"]
mod ime_engine_gpu_scene_b;
#[path = "ime_engine_gpu_scene_c.rs"]
mod ime_engine_gpu_scene_c;
