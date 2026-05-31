---
name: sdp-code
description: >
  Use when the design doc is ahead of the code and the CODE must change to match
  it — the doc is the stable source of truth and is never edited here. Trigger
  on: "implement the doc", "sync code to spec", "make the code match the doc",
  "build the feature from the doc", "doc says X but the code is wrong",
  explicit sdp-code. Counterpart: sdp-doc (writes the doc; use that instead when
  the doc is what should change). Operates from docs/<domain>/<feature>.html;
  writes an impl plan or implements directly.
---

# SDP — Code (doc → code)

Doc ahead of code. Diff doc vs impl, then either write an implementation plan
(large/risky diff) or implement directly (small/low-risk diff). Never edits the
doc, never runs git, never commits.

Shared conventions: [../sdp-doc/references/conventions.md](../sdp-doc/references/conventions.md) — read before working with any doc.

## Checklist

1. **Identify doc** — confirm `docs/<domain>/<feature>.html`. See *Ambiguity* below if unclear.
2. **Read conventions.**
3. **Conformance check** — run the conformance checklist from conventions. This skill never edits the doc, so don't patch anything here. Cosmetic issues → note them and proceed (they don't block reasoning about the doc). Any structural issue → hand off to sdp-doc Fix mode and stop, since you can't reliably read a doc that violates the structural conventions.
4. **Read doc carefully** — especially: architecture/flow diagram, file map, `data-agent="implementation"`, `data-agent="test"`, other relevant agent blocks. Treat `data-transient="true"` blocks as implementation intent to consume now; note them — once their code exists they become prune candidates for sdp-doc Sync (step 9).
5. **Survey code** — read files from file map. Spawn parallel Explore agents if ≥6 entries, ≥3 top-level dirs, or behavior not locatable from file map.
6. **Diff** — Matches / Missing / Wrong / Stale / Ambiguous. On ambiguous: stop, ask user (route to sdp-doc if it's a design/doc question).
7. **Confirm scope** — short diff list. Suggest splitting if large.
8. **Route by size:**
   - **Small diff** (a few clear, low-risk items — no architectural decisions,
     no cross-cutting changes, fits in one focused sitting): ask the user
     whether to skip planning. Offer {Implement directly / Write a plan first}.
     If they pick direct, implement the diff items now against the doc — no
     `writing-plans`, no plan file.
   - **Otherwise**: invoke `superpowers:writing-plans`:
     ```
     spec: docs/<domain>/<feature>.html
     diff: missing/wrong/stale items
     agent_blocks: implementation, test, relevant others
     constraints: match doc; no redesign; no doc edits; test items → test tasks
     ```
9. **Hand off** — planned: "Plan written at `<path>`. Review, then run
   executing-plans." Direct: report what changed. Either way: run sdp-doc Sync
   afterward if incidental drift appeared, or to prune now-redundant agent
   blocks (transient blocks whose code now exists; blocks that just restate the
   code you wrote) — name them so Sync knows what to drop.

## Ambiguity

| Situation | Action |
|---|---|
| No doc named | List `docs/**/*.html`; ask which |
| Doc fails conformance | Hand off to sdp-doc Fix mode; stop |
| Detail missing for plan | Ask; if design question → sdp-doc; else capture and note for sdp-doc Sync |
| Doc/code disagree, unclear which right | Ask; don't pick silently |
| Multiple docs same surface | Confirm which leads |

## Anti-patterns

- Skipping the plan on a large or risky diff (the direct path is for small,
  low-risk diffs only; non-trivial work must go through `writing-plans` so it's
  structured, persisted, and reviewable).
- Editing code on the planned path (that's executing-plans' job).
- Guessing ambiguity (one question < wrong impl).
- Scope creep into redesign (hand off to sdp-doc).
- Ignoring `data-agent` blocks (they're written for this skill; fold into plan
  or implementation).
- Editing the doc here (drift goes to sdp-doc Sync afterward).
