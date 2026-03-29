# Expression

Construct valid formal expression from semantic intent through guided composition.

---

## Core Idea

The user should not have to author the final form directly.
The system should help assemble meaning first, then render form.

---

## Scope

It asks:

- How can intent become valid structured output without hand-writing the entire final expression?

Best-fit domains are high-structure tasks such as commands, configs, and rule definitions.

---

## Model

1. Seed
2. Expansion
3. Composition
4. Closure
5. Rendering

Closure requires required fields, conflict resolution, and dependency satisfaction.

---

## Output Forms

- intermediate representation
- executable command
- structured configuration
- natural-language restatement

---

## Boundary

Expression does not:

- decide whether action is allowed
- provide safety gating
- manage trust structures
- preserve operation under failure

---

> Expression becomes cheaper when meaning is assembled before form is rendered.
