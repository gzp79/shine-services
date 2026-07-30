---
name: sdp
description: >
  Spec-Driven Practice — keep a feature's living HTML design doc
  (docs/<domain>/<feature>.html) and its code in agreement. One skill for the
  whole doc↔code loop: write/update a doc, implement code from a doc, or refresh
  a stale doc from the code. It figures out which side is ahead and moves the
  other to match; when both moved or they genuinely conflict, it asks which is
  the source of truth. Trigger on: "spec out", "design", "plan a feature",
  "write/update the doc for", "implement the doc", "build the feature from the
  doc", "make the code match the doc" / "make the doc match the code", "sync the
  doc", "doc is stale", "doc says X but the code does Y", "check this doc",
  explicit sdp. Supersedes the old sdp-doc / sdp-code pair.
---

# SDP — Spec-Driven Practice (doc ↔ code)

A feature lives as a human-first HTML design doc and as code. This skill keeps
the two in agreement. It never runs git or commits — the user reviews and
stages. Where docs live, the domain layout, and the doc-format contract are
project facts: see CLAUDE.md → *Design docs* and
[references/conventions.md](references/conventions.md).

The old split (sdp-doc writes docs, sdp-code writes code) forced you to pick the
skill before you knew which artifact should change. Here you don't pick: the
skill's first job is to **detect which side is ahead**, then move the other to
match. You only stop to ask when that's genuinely unclear.

Read [references/conventions.md](references/conventions.md) once per session
before writing or reasoning about any doc — the doc format is a strict contract
and everything below assumes it. Skip if already read this session.

## Preamble (every run, in order)

1. **Locate the doc.** Resolve the doc path from the project's doc layout
   (CLAUDE.md → *Design docs*) and the filename rules in
   [references/conventions.md](references/conventions.md) → *File layout*. Glob
   the docs tree to confirm the doc exists or is missing. If the feature name,
   domain, or target doc is ambiguous, ask (see *Asking*).
2. **Conformance check** (only if the doc exists). Run the conformance checklist
   from conventions. Because this is one skill that both reads and writes the
   doc, fixes are applied inline — there is no cross-skill handoff and no hard
   restart:
   - Clean → continue.
   - Cosmetic (rename suffix, untagged `<details>`, stray date/status line) →
     fix inline, continue. Mechanical; no need to ask.
   - Structural (change badges, status column, inline cosmetic CSS, change-log
     prose, roadmap boxes, missing file map) → fix inline so the doc is
     readable, then continue. If a structural fix would change *meaning* rather
     than form, that's a design question — ask (see *Asking*).

## Detect direction (the front door)

After the preamble, decide which side is authoritative and route. Use the code
survey (below) and the request wording together — don't guess when they
disagree.

| Situation | Direction | Section |
|---|---|---|
| No doc, feature is genuinely new | doc is the new work | **Write doc → (new)** |
| Doc exists, code missing/behind it | doc leads | **Write code** |
| Code exists, doc missing or stale | code leads | **Sync doc** |
| Both moved since they agreed, **or** doc and code genuinely contradict, **or** unclear which is right | — | **Ask which is the source of truth**, then route to the losing side |

There is no standing "doc always wins" default. The request verb is a strong
hint ("spec out / design" → doc leads; "implement / build from the doc" → code
follows; "sync the doc / doc is stale" → code leads), but confirm it against
reality: if the wording says "implement the doc" yet the code already matches
and has moved *past* the doc, that's the ambiguous row — ask.

### Surveying the code

Both "write code" and "sync doc" start by reading the code the doc describes.
From the doc's file map: if ≥6 entries, ≥3 top-level dirs, or the behavior isn't
locatable from the map, spawn parallel Explore agents; otherwise read directly.
Verify the real contract from source — never bake an agent's paraphrase into a
doc or a plan.

## Write doc (new feature)

The doc is the desired state and doesn't exist yet.

1. **Brainstorm** via `superpowers:brainstorming`. Tell it the artifact is HTML
   at the doc path and to suppress its own markdown write step. Skip only for a
   small, well-understood addition to an existing doc.
2. **Write the HTML** per conventions: correct depth-adjusted `doc.css` path,
   standard sections, and a diagram in the layout that fits the flow (layered
   stack / horizontal / fan-out / lifecycle / spatial — pick by flow shape, reuse
   an existing layout before adding CSS).
3. Keep agent `<details>` blocks minimal — prefer the file map and inline source
   references. For a not-yet-built feature, a block that just hands intent to the
   implementer is fine; mark it `data-transient="true"` so a later sync drops it
   once the code lands.
4. **Self-review** against the conventions checklist; fix inline.
5. **Hand off**: report the path. Next step is usually **Write code** to realize
   it.

*Updating an existing doc for an intent-driven change* is the same path: survey
the code first so you don't design against shipped behavior, brainstorm if the
change is non-trivial, then edit surgically (don't regenerate from template).

## Write code (doc → code)

Doc is ahead; the code must change to match it. Never edit the doc here — if the
doc is what should change, you're in the wrong direction (re-check *Detect
direction*).

1. **Read the doc carefully** — especially the architecture/flow diagram, file
   map, and `data-agent="implementation"` / `data-agent="test"` / other relevant
   blocks. Treat `data-transient="true"` blocks as implementation intent to
   consume now; note them as prune candidates for a later sync.
2. **Survey code** (see above) and **diff** doc vs. impl: Matches / Missing /
   Wrong / Stale / Ambiguous. On a genuine ambiguity, stop and ask — if it's a
   design question, that's a doc change (switch direction).
3. **Confirm scope** with a short diff list. Suggest splitting if large.
4. **Route by size:**
   - **Small diff** (a few clear, low-risk items — no architectural decisions,
     no cross-cutting change, fits one focused sitting): offer {Implement
     directly / Write a plan first}. If direct, implement the diff items now
     against the doc — no plan file.
   - **Otherwise**: invoke `superpowers:writing-plans`:
     ```
     spec: <doc path>
     diff: missing/wrong/stale items
     agent_blocks: implementation, test, relevant others
     constraints: match doc; no redesign; no doc edits; test items → test tasks
     ```
5. **Hand off** — planned: "Plan written at `<path>`. Review, then run
   executing-plans." Direct: report what changed. Either way, if incidental
   drift appeared or transient blocks now have code, run **Sync doc** afterward
   and name the blocks to prune.

## Sync doc (code → doc)

Code is reality; refresh the doc to match, after which the doc is primary again.

1. **Survey code** (see above) from the doc's file map.
2. **Diff** per section: In sync / Drifted / Removed / New-in-code / Suspicious.
   Also flag agent blocks now satisfied by the code — any `data-transient="true"`
   block whose code now exists, and any block that, now the code is readable,
   merely restates it. These are *Prune* candidates.
3. **Confirm scope**: show a short bullet diff, prune candidates included. Ask
   only about *Suspicious* items (looks accidental, not designed — see *Asking*).
   Don't ask about clear drift or routine prunes; apply them.
4. **Rewrite** approved sections surgically (not from template). Replace prose
   that paraphrases code with a source reference; remove prune-candidate blocks,
   keeping only those with durable insight (summary / complexity / gotcha /
   rationale). Desired state = current state after sync — no diff narrative, no
   "previously did X".
5. **Self-review**: read fresh — does the doc accurately predict the code?
6. **Hand off**: what changed, any pending decisions.

## Asking

Batch questions into one `AskUserQuestion` call, each item answered
independently. Prefer yes/no options. The decision this skill exists to surface
cleanly:

- **Which is the source of truth?** When doc and code genuinely conflict, or both
  moved: ask {Doc is right → change the code / Code is right → sync the doc},
  then route to the losing side. Never pick silently on a real design
  disagreement.

Other common asks: suspicious code item during sync → {Document it / Leave out};
ambiguous doc target → list candidate paths as options; a structural conformance
fix that would change meaning → confirm the intended meaning first.

## Anti-patterns

- Running git or committing (the user stages and commits, always).
- Silent rewrites in Sync — show the diff first.
- Change-log narrative, diff tables, "today vs proposed", change badges, dates,
  status lines (git owns history — see conventions).
- Prose where a diagram fits; untagged `<details>`; hand-rolled cosmetic CSS.
- Out-of-scope nodes in a diagram — a box implies participation. Non-participants
  (works without subscribing, rejected alternative, related neighbor) go in
  prose, never a box, even dimmed (see conventions).
- Agent `<details>` blocks that paraphrase the code instead of pointing at it, or
  that survive after their code exists — prefer source refs; keep a block only
  for durable insight.
- Regenerating a whole doc from template when a surgical edit suffices.
- Skipping brainstorm on a genuinely new feature.
- Editing the doc while in **Write code**, or editing code while in **Write
  doc** / **Sync doc** — if the other side needs to change, re-check *Detect
  direction* and route.
- Picking a direction silently when doc and code disagree — ask.
- Baking accidental code into the doc during Sync — flag as Suspicious, ask.
