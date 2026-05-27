---
name: sdp-sync-code
description: >
  Use when the design doc has moved ahead of the code and the code must be
  brought back into sync — i.e., the doc is the source of truth. Trigger on
  phrases like "implement the doc", "sync code to spec", "the doc says X but
  the code does Y", "make the code match the doc", or any explicit
  `sdp-sync-code` invocation. Counterpart to `sdp-sync-doc` (other direction)
  and `sdp-plan` (which authors the doc). Operates from `docs/<feature>.html`
  and produces an implementation plan that another loop will execute.
---

# SDP — Sync Code (doc → code)

The doc has moved ahead of the code. This skill diffs the doc against the
implementation, then produces an implementation plan that brings the code in
line. It does **not** edit code itself — that is the job of the executing-plans
flow.

This is one half of the sync pair:

- **sdp-sync-doc** — code is right, doc is stale
- **sdp-sync-code** — doc is right, code is stale (this skill)

All three SDP skills share the doc format defined in
[../sdp-plan/references/conventions.md](../sdp-plan/references/conventions.md).
Read it before working with any doc.

## Checklist

Create one TodoWrite todo per item and complete in order:

1. **Identify the doc** — confirm which `docs/<feature>.html` is the source of
   truth. If the user did not name it, see *Resolving ambiguity* below.
2. **Read conventions** — read [../sdp-plan/references/conventions.md](../sdp-plan/references/conventions.md).
   The doc's structure (especially its file map and `<details
   data-agent="…">` blocks) drives the implementation plan.
3. **Conformance check** — run the conformance checklist from conventions.md
   against the target doc. Group findings by tier (Cosmetic vs Structural —
   see *Severity* in conventions). Then:

   - **All clear** — continue to step 4.
   - **Cosmetic only** — list them and ask the user to approve fixing
     inline. Apply the fixes (no commit — the user reviews and commits when
     ready), then continue to step 4.
   - **Any structural** — stop. Hand off all findings (cosmetic + structural)
     to `sdp-plan` fix mode in one pass; do not fix the cosmetic items
     inline first when structural work is also needed. Show the grouped list
     and offer:

     > "Structural issues need `sdp-plan` fix mode: <structural list>.
     > Cosmetic items will be folded into the same pass: <cosmetic list>.
     > Run fix mode, then re-invoke me."

     If the cosmetic list includes a filename-suffix violation (`-spec`,
     `-design`, date suffix, etc.), append the rename callout so the user
     knows the path will change:

     > "Note: fix mode will rename `<old>.html` → `<new>.html`. After fix
     > mode, re-invoke me against the new path."

     Do not patch structural issues here — that is `sdp-plan`'s job. After
     fix mode, the user re-invokes this skill from scratch (there is no
     resume point — the doc on disk is the state).
4. **Read the doc carefully** — especially:
   - the architecture / flow diagram (the contract)
   - the file map (the targets)
   - the `<details data-agent="implementation">` block (concrete notes)
   - the `<details data-agent="test">` block (test plan, fed into the plan)
   - any other agent-tagged collapsibles relevant to the change
5. **Survey the current code** — read the files in the file map. Build a
   mental model of where the code stands today. Spawn parallel Explore agents
   when:
   - the file map has ≥6 entries, **or**
   - the file map's paths span ≥3 distinct top-level directories (e.g., a
     doc that touches `engine/`, `avatar/`, and `events/` simultaneously),
     **or**
   - the doc references behaviour you can't locate from the file map alone.

   Otherwise read the files directly. The goal is parallelism when the
   surface is genuinely wide, not always.
6. **Diff doc against code** — produce a categorised list:
   - **Matches** — already correct.
   - **Missing** — doc describes it, code lacks it.
   - **Wrong** — code has it but does not match doc.
   - **Stale** — code has something the doc says is removed.
   - **Ambiguous** — doc is unclear on this point. **Stop and ask the user**;
     if the doc is wrong, hand off to `sdp-sync-doc` or `sdp-plan`. Do not
     resolve ambiguity by guessing.
7. **Confirm scope with the user** — show the diff (short list, not prose).
   Confirm what is in-scope for this sync. If the diff is large, suggest
   splitting into multiple plans.
8. **Generate the implementation plan** — invoke
   `superpowers:writing-plans` with the doc as the spec and the diff as the
   delta. Use this handoff payload:

   ```
   spec: docs/<feature>.html  (read in full; this is the contract)
   diff:
     missing: <items>
     wrong:   <items>
     stale:   <items>
   agent_blocks_to_consult:
     - data-agent="implementation"  (target-state notes)
     - data-agent="test"            (test plan; map 1:1 to test tasks)
     - <other tags relevant to the change>
   constraints:
     - implement to match the doc; do not redesign
     - do not edit the doc
     - test plan items become test tasks
   ```

   This skill will not execute the plan — leave it for the user/next loop.
9. **Hand off** — present the plan path and tell the user the next step:
   - "Plan written. Review it, then run `superpowers:executing-plans` to apply
     it. After execution, run `sdp-sync-doc` if any incidental drift appeared
     during implementation."

This skill stops at the plan. It does not run `executing-plans` itself, never
edits code, never runs `git`, and never commits. The user reviews and commits
when ready.

## Resolving ambiguity

| Situation | Action |
|-----------|--------|
| User did not name a doc | List existing `docs/*.html`; ask which one. |
| Doc fails conformance check | Hand off to `sdp-plan` in fix mode. Do not silently patch and proceed. |
| Doc is missing detail needed to write the plan | Ask user. If the answer changes the design, hand off to `sdp-plan`. If it just clarifies, capture the answer and either update the doc inline (small) or note it for `sdp-sync-doc` afterward. |
| Doc and code disagree but it's not obvious which is right | Stop and ask. Don't pick a direction silently. |
| Multiple docs touch the same code surface | Confirm which doc is leading; sync against that one. The others may need follow-up. |

## Anti-patterns

- **Editing code in this skill.** This skill produces a plan only. Code edits
  happen via `superpowers:executing-plans` (or manual review).
- **Bypassing writing-plans.** Do not write the plan inline in chat. Use
  `superpowers:writing-plans` so the plan is structured, persisted, and
  executable. The doc + diff feed into it.
- **Resolving ambiguity by guessing.** If the doc does not pin down a detail,
  ask. The cost of one question is low; the cost of a wrong implementation is
  high.
- **Scope creep into redesign.** This skill makes code match the doc. If the
  user wants to redesign, hand off to `sdp-plan`. If you find the doc itself
  is wrong, hand off to `sdp-sync-doc` or `sdp-plan` rather than "fixing" it
  on the way through.
- **Ignoring agent-tagged blocks.** The `<details data-agent="implementation">`
  and `<details data-agent="test">` blocks are written *for this skill*. Read
  them, fold them into the plan, and propagate test-plan items into the plan's
  test tasks.
