# Suzaku Map

A small set of orthogonal system concepts.

Start here:

- [Map](./src/00-Map.md)

## Set

1. [Signal](./src/01-Signal.md)
2. [Control](./src/02-Control.md)
3. [Resilience](./src/03-Resilience.md)
4. [Coordination](./src/04-Coordination.md)
5. [Expression](./src/05-Expression.md)
6. [IME for XR and Tablet](./src/06-IME-XR-Tablet.md)

## Rule

Each document should answer one question only:

- `Signal`: what is true or no longer trustworthy
- `Control`: what action must be gated or separated
- `Resilience`: how continuity is preserved under constraint
- `Coordination`: how actors cooperate without strong institutions
- `Expression`: how intent becomes valid structured output

## 中文说明

这是一个压缩后的概念集合。

每篇文档只负责一个问题，尽量保持正交。

## Prototype

This repository now contains a TDD-driven Rust IME prototype for XR and tablet environments:

- Engine: [src/ime.rs](./src/ime.rs)
- Demo: `cargo run`
- Tests: `cargo test`

## TDD Loop

The active workflow is:

1. Write or extend a behavior test in [tests/ime_engine.rs](./tests/ime_engine.rs)
2. Implement the smallest engine change in [src/ime.rs](./src/ime.rs)
3. Run `cargo test`

## GPU Rendering

Yes, the project can use GPU rendering.

- The engine remains UI-agnostic.
- A feature-gated `wgpu` scene builder lives in [src/ime.rs](./src/ime.rs).
- Enable it with `cargo test --features gpu` or integrate it into a windowed host renderer.
