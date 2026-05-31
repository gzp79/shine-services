---
name: sdp-doc
description: >
  Create, update, sync, or conformance-check a feature's living HTML design doc
  under docs/<domain>/<feature>.html. Doc is the source of truth. Trigger on:
  "spec out", "design", "plan a feature", "write/update the doc for", "doc is
  stale", "sync the doc to the code", "update doc to match code", "check this
  doc", explicit sdp-doc. Pairs with sdp-code (doc → code).
---

# SDP — Doc

The doc is the desired state, human-first HTML. This skill only ever writes the
doc. **Never edits code, never runs git, never commits.** To realize a doc in
code, hand to `sdp-code`.

Read [references/conventions.md](references/conventions.md) once per session
before writing any doc. Skip if already read this session.

## Mode select (pick one, then jump to its section)

| Mode | Trigger | Brainstorm? |
|---|---|---|
| **Create** | doc missing | yes (new feature) |
| **Update** | doc exists, intent-driven change | yes if non-trivial |
| **Sync** | code moved ahead, doc stale | no |
| **Fix** | conformance violations to apply | no |
| **Check** | "is this doc conformant?" (read-only) | no |

Doc path = `docs/<domain>/<feature>.html`. Domains: `client/web`,
`services/identity`, `core/game`, `shared`. Feature is kebab-case, no
`-spec`/`-design`/date suffixes. CSS: relative path reaching `docs/doc.css`
(`../../doc.css` from a two-level domain folder); never inline, never duplicate.

## Shared preamble (all writing modes)

1. **Locate doc.** Glob `docs/**/<feature>*.html` to confirm exists/missing. If
   the feature name or domain is ambiguous, ask (see *Asking*).
2. **Conformance gate** (Sync/Update on existing docs): run Check.
   - Clean → continue.
   - Cosmetic only → fix inline, continue (mechanical; no need to ask).
   - Any structural → switch to **Fix** first, then resume.

## Create / Update

1. Preamble.
2. **Is the feature already built?** Grep/Glob for the code the doc would
   describe. This decides where the content comes from — do NOT brainstorm a
   design that contradicts shipped code:
   - **Code exists** (documenting reality, e.g. "write a doc for X"): skip
     brainstorming. **Survey the code first** using Sync's step 2 heuristic
     (parallel Explore agents if ≥6 files, ≥3 top-level dirs, or behavior isn't
     locatable; else read directly). Verify the real contract from source —
     don't bake an agent's paraphrase into the doc. Then write.
   - **Not yet built** (genuinely new feature, or a non-trivial intent-driven
     change): **brainstorm** via `superpowers:brainstorming`. Tell it the
     artifact is HTML at the doc path and to suppress its own markdown write
     step. Skip for small, well-understood edits.
3. **Write/edit HTML** per conventions: correct `doc.css` path, standard
   sections, and a diagram in the layout that fits the flow (layered stack /
   horizontal / fan-out — see conventions *Architecture*; pick by flow shape,
   reuse an existing layout before adding CSS).
   Keep agent `<details>` blocks minimal — prefer file-map and inline source
   references; add a block only where it carries durable insight (see
   *Collapsibles*). For a feature not yet built, a block that just hands intent
   to the implementer is fine — mark it `data-transient="true"` so a later Sync
   drops it once the code lands.
4. **Self-review** against the conventions checklist; fix inline.
5. **Hand off**: report path. For a not-yet-built feature, next step is
   `sdp-code` to realize it. For a doc of already-built code, it's already in
   sync — no handoff needed.

## Sync (code → doc)

Code is reality; refresh the doc to match — after which the doc is primary again.

1. Preamble.
2. **Survey code** from the doc's file map. If ≥6 entries, ≥3 top-level dirs, or
   behavior isn't locatable from the map: spawn parallel Explore agents. Else
   read directly.
3. **Diff** per section: In sync / Drifted / Removed / New-in-code / Suspicious.
   Also flag agent `<details>` blocks now satisfied by the code: any
   `data-transient="true"` block whose code now exists, and any block that — now
   that the code is readable — merely restates it. These are *Prune* candidates.
4. **Confirm scope**: show a short bullet diff, Prune candidates included. Ask
   only about *Suspicious* items (looks accidental, not designed) — see
   *Asking*. Don't ask about clear drift or routine prunes; just apply them.
5. **Rewrite** approved sections surgically (not from template). Replace prose
   that paraphrases code with a source reference; **remove** Prune-candidate
   blocks, keeping only those carrying durable insight (summary / complexity /
   gotcha / rationale). Desired state = current state after sync. No diff
   narrative, no "previously did X".
6. **Self-review**: "read fresh, does the doc accurately predict the code?"
7. **Hand off**: what changed, any pending decisions.

## Fix (conformance only)

Conform without changing meaning. Format/structure pass, no redesign.

1. Take the violation list (or run Check to produce one).
2. If change-log content must go somewhere, default to **delete**; ask only if
   it looks load-bearing (see *Asking*).
3. Apply in order: rename file (suffix violation, plain filesystem rename — the
   user stages it) → strip change badges → remove roadmap/future content → strip
   dates/status lines → add `data-agent` to untagged `<details>` → move inline
   `<style>`/styles into `doc.css`.
4. Self-review, then resume the mode that triggered Fix (or hand back).

## Check (read-only)

Run the conformance check from conventions. Report grouped by tier and stop —
do not modify:

> Cosmetic: `<list>` · Structural: `<list>`
> Run Fix mode to apply, or leave as-is.

If clean: "Doc conforms — no findings."

## Asking

Batch questions into one `AskUserQuestion` call, each item a separate question
so the user answers independently. Prefer yes/no options. Examples: suspicious
code item → {Document it / Leave out}; change-log content in Fix → {Delete /
Keep as `data-agent="migration"`}; ambiguous doc target → list candidate paths
as options. Never guess on a genuine design disagreement — if doc and code
fundamentally conflict, ask which is right and route accordingly (here if
code wins, or hand to `sdp-code` if doc wins).

## Anti-patterns

- Editing code or running git (this skill writes the doc, full stop).
- Silent rewrites in Sync — show the diff first.
- Change-log narrative, diff tables, "today vs proposed", change badges, dates,
  status lines (git owns history — see conventions).
- Prose where a diagram fits; untagged `<details>`; hand-rolled CSS.
- Out-of-scope nodes in a diagram — a box implies participation. A
  non-participant (works without subscribing, rejected alternative, related
  neighbour) goes in prose, never as a box, even dimmed (see conventions).
- Agent `<details>` blocks that paraphrase the code instead of pointing at it,
  or that survive after their code exists — prefer source refs; keep a block
  only for durable insight (see *Collapsibles*).
- Regenerating a whole doc from template when a surgical edit suffices.
- Skipping brainstorm on a genuinely new feature.
- Baking accidental code into the doc during Sync — flag as Suspicious, ask.
