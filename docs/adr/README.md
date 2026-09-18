# Architecture Decision Records

An ADR is a permanent, version-controlled record of one significant
architectural choice: the context that forced it, the options considered,
the decision, and its consequences. Write one whenever a change alters how
major parts fit together (rendering architecture, persistence format,
lighting model, worldgen contract). Routine feature work does not need one.

Rules:

1. ADRs are append-only. To reverse a decision, add a new ADR that marks the
   old one **Superseded by NNNN** — never rewrite history.
2. One decision per file, numbered `NNNN-kebab-title.md`.
3. Keep it to one page; link code and docs by path and symbol, never by line
   number.
4. The ADR explains *why*. The *what* belongs in code and `docs/*.md`.

## Template

```markdown
# NNNN. Title

Status: Accepted | Superseded by NNNN

## Context
Forces at play: the problem, constraints, and what happens if nothing changes.

## Decision
The choice made, stated in one sentence, then the mechanism in a paragraph.

## Alternatives considered
- Option — why rejected.

## Consequences
What is now easier or harder; what invariants and tests pin the decision;
what future work it enables or forecloses.
```

## Index

| ADR | Decision | Status |
| --- | --- | --- |
| 0001 | Decoupled shadow architecture (DS01) | Accepted |
| 0002 | Water variance filtering (WA01); `--water-mottle` pinned to 0 | Accepted |
| 0003 | Fog height derives from `worldgen::SEA_LEVEL` | Accepted |
| 0004 | Static linear-albedo table for bounce light | Accepted |
