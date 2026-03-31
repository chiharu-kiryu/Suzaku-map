# IME for XR and Tablet

This document applies the Suzaku Map to an input method engine intended for XR and tablet environments.

---

## Design Target

The engine should remain usable when:

- pointer precision is low
- attention is fragmented
- posture and viewport change often
- text intent is clearer than final form

The system should prefer guided composition over dense key-by-key entry.

---

## Mapping

### Signal

Track what is trustworthy before the engine commits:

- pointer confidence
- gaze dwell stability
- source device identity
- viewport mode
- text-field intent hints

The engine should detect noisy input and reduce over-eager candidate mutation.

### Control

Gate expensive or destructive text actions:

- commit replacement across many characters
- auto-correction above confidence threshold
- mode switching that discards composition state
- command or shortcut emission

Critical edits should require a stronger confirmation path than ordinary character entry.

### Resilience

Remain usable under degraded interaction:

- fall back from free-form typing to phrase tiles
- reduce candidate count when confidence is weak
- preserve last stable composition snapshot
- allow one-step undo after commit

The engine should step down gracefully before it becomes frustrating.

### Coordination

Allow multiple actors and surfaces to cooperate:

- hand tracking
- hardware keyboard
- stylus
- on-screen panel
- host application semantic hints

No single input source should own the entire composition lifecycle.

### Expression

Use semantic composition first:

1. Seed: collect strokes, phonetics, gestures, or phrase intent
2. Expansion: generate candidate tokens
3. Composition: assemble tokens into a structured text plan
4. Closure: satisfy required context, resolve conflicts, and preserve caret invariants
5. Rendering: emit display text and commit payload

This is the primary engine loop.

---

## Practical Rules

1. Keep composition state small and inspectable.
2. Prefer large, stable candidate targets over dense keyboard grids.
3. Delay aggressive correction until confidence is earned.
4. Make recovery cheaper than perfect precision.
5. Separate visible draft text from committed text.

---

## Reference Implementation

See the Rust engine in [src/ime.rs](./ime.rs).

The optional GPU-facing scene builder is feature-gated behind `gpu`.
