# Documentation index

The rules file is `AGENTS.md` at the repo root (`CLAUDE.md` links to it).
Everything here is reference material it points to.

| File | Contents |
| --- | --- |
| `architecture.md` | Module map, the frame's data flow, the WGSL shader inventory |
| `rendering.md` | Atmosphere (fog/clouds), shadow architecture, water filtering, lighting, TAA/grade |
| `world.md` | Worldgen and sea level, `BlockDef` fields, the albedo table, edit journals |
| `testing.md` | Suite layout, enforced invariants, commands, the validation scripts, test protocol |
| `audit-triage.md` | Four external audits adjudicated against this tree — verified findings and dispositions |
| `lessons.md` | Living log of pitfalls and agent fumbles — append freely |
| `adr/` | Architecture Decision Records; template and rules in `adr/README.md` |

## Maintenance rules

1. A behavior change updates the doc it contradicts in the same commit.
2. If doc and code disagree, the code is right — fix the doc, not the
   wording of reality. Mention the discrepancy class in `lessons.md`.
3. Cite paths and symbols, never line numbers.
4. Keep prose tight. These files are read by agents with finite context;
   tables over paragraphs, facts over narrative.
5. ADRs are append-only. A reversed decision is a new ADR that marks the old
   one superseded.
