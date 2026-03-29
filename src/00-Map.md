# Map

This set is organized by system question, not by topic.

---

## Layers

### 1. Signal

Question:
What is true, changing, or unsafe to trust?

Document:

- [Signal](./01-Signal.md)

Boundary:
Observe and preserve evidence.
Do not decide or enforce.

### 2. Control

Question:
What actions must be gated, confirmed, or separated?

Document:

- [Control](./02-Control.md)

Boundary:
Constrain execution and responsibility.
Do not diagnose or recover.

### 3. Resilience

Question:
How does the system remain usable under stress, failure, or scarcity?

Document:

- [Resilience](./03-Resilience.md)

Boundary:
Preserve continuity.
Do not authorize or coordinate.

### 4. Coordination

Question:
How do actors maintain cooperation when institutions are weak or absent?

Document:

- [Coordination](./04-Coordination.md)

Boundary:
Organize continuity across actors.
Do not gate critical action.

### 5. Expression

Question:
How is semantic intent turned into valid structured output?

Document:

- [Expression](./05-Expression.md)

Boundary:
Define the expression interface.
Do not govern, recover, or coordinate.

---

## Separation Test

1. If it improves visibility, it is `Signal`.
2. If it limits action, it is `Control`.
3. If it preserves operation under constraint, it is `Resilience`.
4. If it organizes multi-actor continuity, it is `Coordination`.
5. If it builds valid output from intent, it is `Expression`.

If one note fits multiple tests, split it.
