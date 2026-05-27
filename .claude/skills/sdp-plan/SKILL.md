---
name: sdp-plan
description: >
  Use when the user wants to design, spec, plan, or document a feature —
  creating or updating its living HTML design doc under `docs/<feature>.html`.
  Trigger on phrases like "spec out", "design", "plan a feature", "write a doc
  for", "update the doc for", or any explicit `sdp-plan` invocation. Also use
  in **fix mode** when `sdp-sync-doc` or `sdp-sync-code` reports the target
  doc is non-conformant ("doc has change badges", "fix conventions", "doc has
  date/status line"), or to run a standalone conformance check on an
  existing doc ("check this doc", "is this doc conformant"). The output is a
  human-targeting HTML doc that reflects the desired state of the feature,
  not a change log. Pairs with `sdp-sync-doc` (code → doc) and `sdp-sync-code`
  (doc → code).
---

# SDP — Plan

This skill produces or updates a feature's living design doc as HTML at
`docs/<feature>.html`. The doc is the source of truth that the two sync skills
reconcile against. It is human-first: small flow diagrams, short prose, and
collapsible sections that carry the agent-only details.

This skill is the **plan stage** of a three-skill loop:

- **sdp-plan** — create/update the doc (this skill); also fixes
  non-conformant docs reported by the sync skills
- **sdp-sync-doc** — when code drifted, pull code state back into the doc
- **sdp-sync-code** — when doc moved ahead, generate a plan to update code

Read [references/conventions.md](references/conventions.md) before producing or
modifying any doc. It defines the file layout, HTML structure, flow-diagram
patterns, and the agent-tagging scheme — and the other two skills rely on the
exact same conventions, so deviations break them.

## Modes

This skill runs in one of four modes. Pick the right one before starting:

| Mode | When | What it does |
|------|------|--------------|
| **Create** | Doc doesn't exist | Brainstorm, then write fresh `docs/<feature>.html` |
| **Update** | Doc exists, design is changing | Brainstorm the change, then edit the existing doc |
| **Fix** | Sync skill reports non-conformant doc | No brainstorm; rewrite the doc to satisfy conventions, preserving content |
| **Check** | User asks "is this doc conformant?" | Read-only: run conformance check, report grouped findings, do nothing else |

Fix mode is described in *Fix mode* below. Check mode is described in *Check
mode* below. Create and Update share the checklist that follows.

## Checklist

Create one TodoWrite todo per item and complete in order:

1. **Identify the feature** — confirm a single feature name (kebab-case). The
   doc will live at `docs/<feature>.html`. No `-spec`, `-design`, or date
   suffixes. If the user is broadening or splitting an existing feature, decide
   the new file names with them before writing.
2. **Decide: brainstorm or update** — if the doc does not exist, or the user is
   making non-trivial changes to direction, run the brainstorming flow first
   (see *Brainstorming first* below). For small clarifications to an existing
   doc, skip straight to writing.
3. **Brainstorm** *(when needed)* — invoke `superpowers:brainstorming` to reach
   a validated design. The brainstorming skill drives the conversation and ends
   with an approved design. **Suppress its default `.md` spec write step** —
   tell brainstorming the artifact for this project is HTML at
   `docs/<feature>.html`, produced by this skill.
4. **Read conventions** — read [references/conventions.md](references/conventions.md)
   in full before writing. Verify `docs/doc.css` exists; if not, copy the
   reference template from conventions.
5. **Write the HTML** — produce `docs/<feature>.html` following the conventions
   exactly: link to `./doc.css`, use the standard sections, prefer flow
   diagrams over prose, place agent-targeted detail inside `<details
   data-agent="…">` blocks (collapsed by default).
6. **Self-review** — open the file mentally with fresh eyes against the
   conventions checklist (placeholders, change-log leakage, prose-where-a-diagram-belongs,
   missing agent tags, broken CSS link). Fix inline.
7. **Hand off** — tell the user the file path(s) written and what comes next:
   - "Doc written to `docs/<feature>.html` (and `docs/doc.css` if newly
     added). Review and commit when ready. If code already implements this,
     run `sdp-sync-doc` to reconcile any drift; otherwise run `sdp-sync-code`
     to generate an implementation plan."

This skill never runs `git` commands and never commits. The user owns
review and commit of every change.

## Fix mode

Triggered when `sdp-sync-doc` or `sdp-sync-code` reports the target doc
violates conventions (filename suffix, change badges, date/status line in
header, untagged `<details>`, status column in file map, etc.). The user
forwards the violation list to this skill.

Goal: make the doc conform without changing what it says. Fix mode is a
formatting/structural pass, not a redesign.

Steps:

1. **Read the violation list** the user (or sync skill) provided. The list is
   normally tier-grouped (Cosmetic vs Structural — see *Severity* in
   conventions); use that grouping to skip steps that have nothing to do
   (e.g., no rename step if the filename is fine). If the user just said
   "fix the doc" with no list, read the doc and run the conformance check
   from `references/conventions.md` yourself to produce the grouped list.
2. **Read conventions** in full.
3. **Confirm scope** with the user: show the list, note that fix mode is
   format-only, and ask if any apparent change-log content (e.g., a "Changes"
   table) should be deleted outright or moved into a `<details
   data-agent="migration">` block. Default: delete — git is the change log.
4. **Apply the fixes** in this order (each is a precise, mechanical edit, not
   a rewrite). Fix mode receives a tier-grouped list from the caller and
   addresses both tiers in a single pass — do not split into two:
   - Rename the file if its name has a `-spec` / `-design` / date / version
     suffix. Confirm the new name with the user before renaming. Use the
     Bash tool to run a non-git rename (e.g., `mv`); do not invoke
     `git mv` — staging is the user's decision.
   - Strip `new` / `refactor` / `keep` / `removed` badges everywhere they
     appear. If the file map has a status column, drop the column entirely
     and keep only File + Role.
   - Remove roadmap/placeholder content from the architecture diagram
     (e.g., boxes labelled "future" or for components not built and not
     being built now). If their absence breaks the visual flow, ask the
     user how to recompose the diagram.
   - Strip dates and status lines from the header.
   - Add `data-agent="…"` to every `<details>` that lacks it. Pick a tag from
     the conventions taxonomy that fits the contents; if unclear, ask the
     user once for that block.
   - Move inline `<style>` blocks into `doc.css` (or remove if redundant with
     existing rules). Confirm with user before adding to `doc.css`.
   - Remove change-log narrative (paragraphs comparing "today vs proposed",
     "previously did X", etc.).
5. **Self-review** against the full conformance checklist in conventions.
6. **Hand back** to the sync skill that triggered the fix: "Doc now
   conforms — review the changes and commit when ready, then re-run
   `sdp-sync-doc` / `sdp-sync-code` to continue."

Fix mode does **not** brainstorm, does **not** invoke
`superpowers:brainstorming`, and does **not** commit. The design content is
preserved as-is; only the shell around it changes.

## Check mode

Triggered when the user asks "is this doc conformant?", "check this doc", or
similar — they want to know whether a doc satisfies conventions without
changing anything. Check mode is read-only.

Goal: report the tier-grouped findings and stop. Do not edit the doc, do not
brainstorm, do not invoke any other skill.

Steps:

1. **Identify the doc** the user is asking about. If they didn't name one,
   list `docs/*.html` and ask which.
2. **Read conventions** in full.
3. **Run the conformance check** from `references/conventions.md` against the
   doc. Group findings by tier (Cosmetic vs Structural).
4. **Report back** in the same format the sync skills use:

   > "Conformance check found:
   >
   > Cosmetic (can fix inline): <list, or 'none'>
   > Structural (needs fix mode): <list, or 'none'>
   >
   > Run `sdp-plan` in fix mode to apply these, or leave the doc as-is."

   If the doc is fully conformant, say so plainly: "Doc conforms — no
   findings."
5. **Stop.** Do not offer to fix anything in this mode. The user re-invokes
   `sdp-plan` (which will land in fix mode) if they want changes.

Check mode does **not** edit, **not** rename, **not** brainstorm, and
**not** commit. It is purely a diagnostic.

## Brainstorming first

When the feature is new or the change is non-trivial, this skill delegates the
design conversation to `superpowers:brainstorming`. Brainstorming is good at
eliciting requirements, exploring approaches, and reaching agreement. Its
default end-state writes a markdown spec — **override that** for this project:

> Tell brainstorming up-front: "The artifact for this project is an HTML doc at
> `docs/<feature>.html`, produced by the `sdp-plan` skill. Run the brainstorm
> normally but skip your markdown spec write step — `sdp-plan` will produce the
> HTML once the design is approved."

Once brainstorming hands back an approved design, return here and write the
HTML.

## Doc shape — quick reference

(The full rules are in [references/conventions.md](references/conventions.md).
This is a reminder.)

```
docs/<feature>.html
  <link rel="stylesheet" href="./doc.css">
  <header>            feature name + one-line purpose
  Problem             2-3 sentences, what & why
  Architecture        flow diagram (boxes + arrows), minimal prose
  Key rules           tables for decision matrices, tags for status
  File map            table: file → role
  <details data-agent="test">          test plan
  <details data-agent="implementation"> implementation notes
  <details data-agent="security">      threat surface, etc.
```

The `<details>` blocks are the agent-aid surface. They are always collapsed by
default so humans see a clean overview, and they are tagged so future agents
(or the sync skills) can find what they need.

## Anti-patterns

- **Change-log doc.** The doc describes the desired state, not a diff from the
  current state. No "today vs proposed" tables. If you need to capture history,
  use git.
- **Wall of prose for architecture.** If a flow can be drawn, draw it. Prose
  belongs in `<details>` blocks or short captions.
- **Untagged collapsibles.** Every `<details>` should have `data-agent="…"` so
  the sync skills can route work. Pick a tag that fits; the taxonomy is open.
- **Multiple files for one feature.** One feature = one HTML file. If a feature
  is too big, split it into separate features with separate docs.
- **Hand-rolled CSS.** All styling comes from `docs/doc.css`. If you need a new
  visual primitive, add it to `doc.css` so other docs can reuse it.
- **Skipping brainstorming on a new feature.** "I already know what to write"
  is a yellow flag. Run brainstorming unless the user has explicitly given you
  a complete design.
