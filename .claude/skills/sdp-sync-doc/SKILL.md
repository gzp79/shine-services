---
name: sdp-sync-doc
description: >
  Use when code has moved ahead of its design doc and the doc must be brought
  back into sync — i.e., the code is the source of truth. Trigger on phrases
  like "update the doc to match the code", "the doc is stale", "sync the doc",
  "the implementation changed, fix the doc", or any explicit `sdp-sync-doc`
  invocation. Counterpart to `sdp-sync-code` (which goes the other way) and
  `sdp-plan` (which authors the doc). Operates on `docs/<feature>.html` files.
---

# SDP — Sync Doc (code → doc)

The code has changed and the design doc is now lying. This skill reads the code,
compares it against the doc, and rewrites the doc to reflect what the code
actually does.

This is one half of the sync pair:

- **sdp-sync-doc** — code is right, doc is stale (this skill)
- **sdp-sync-code** — doc is right, code is stale

Both skills share the doc format defined in
[../sdp-plan/references/conventions.md](../sdp-plan/references/conventions.md).
Read it before editing any doc.

## Checklist

Create one TodoWrite todo per item and complete in order:

1. **Identify the doc** — confirm which `docs/<feature>.html` to sync. If the
   user did not name it, see *Resolving ambiguity* below.
2. **Read conventions** — read [../sdp-plan/references/conventions.md](../sdp-plan/references/conventions.md)
   so the rewritten doc keeps the project's HTML structure, flow-diagram style,
   and agent-tagged collapsibles.
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
4. **Survey the code** — find the files the doc references (its file map is the
   anchor). Read them. If the doc has no file map, ask the user for the entry
   points or use Explore to locate them. Spawn parallel Explore agents when:
   - the file map has ≥6 entries, **or**
   - the file map's paths span ≥3 distinct top-level directories, **or**
   - you need to detect *new* code that isn't in the file map (broad search).

   Otherwise read the files directly.
5. **Diff against the doc** — for each section of the doc, list what the code
   says vs what the doc says. Categorise each item:
   - **In sync** — leave it.
   - **Drifted** — code differs; doc must be updated.
   - **Removed** — doc describes something the code no longer has.
   - **New in code, missing from doc** — needs adding.
   - **Suspicious / out-of-scope** — code does something the doc does not
     mention but probably shouldn't either; flag to the user, do not silently
     document it.
6. **Confirm scope with the user** — show the categorised diff (short bullet
   list, not a wall of text). Ask: "I'll update the doc to match items 1–N.
   Items X are flagged as suspicious — keep, document, or ignore?" Wait for
   approval.
7. **Rewrite the doc** — apply the approved updates. Preserve the structure
   from conventions: same sections, same flow-diagram patterns, same `<details
   data-agent="…">` blocks (collapsed). The doc must still describe the
   *desired* state, which after this sync is the *current* state — no diff
   tables, no "previously did X" footnotes.
8. **Self-review** — re-check against the conventions checklist. Run a final
   pass: does the doc, read fresh, accurately predict what the code does?
9. **Hand off** — tell the user what changed in the doc and call out any
   flagged items that still need a decision. The user reviews and commits
   when ready. If the diff revealed a design problem (not just drift),
   suggest running `sdp-plan` to redesign rather than papering over it.

This skill never edits code, never runs `git`, and never commits.

## Resolving ambiguity

The user may say "sync the doc" without naming a feature, or the code change
may straddle multiple feature docs. Handle these cases:

| Situation | Action |
|-----------|--------|
| User did not name a doc | List existing `docs/*.html`; ask which one. |
| Doc fails conformance check | Hand off to `sdp-plan` in fix mode. Do not silently patch and proceed. |
| Recent code change touches files mapped in N docs | List the candidates with the touched files; ask which to sync (may be more than one). |
| Code change has no matching doc | Ask: "No existing doc covers this. Should I create a new spec via `sdp-plan`, or skip?" Do not invent a doc here — that is `sdp-plan`'s job. |
| Doc and code disagree fundamentally on direction | Stop. Ask the user whether the code or the doc is correct. If the doc is correct, hand off to `sdp-sync-code`. If the code is correct, continue. If neither is fully right, hand off to `sdp-plan`. |

When in doubt, ask. A wrong-direction sync is worse than a one-message delay.

## Anti-patterns

- **Silent rewrites.** Always show the diff and get approval before editing.
- **Documenting accidental code.** If the code does something nobody intended,
  flag it; do not bake it into the doc as a feature.
- **Format drift.** Do not "improve" the conventions while syncing. If the
  conventions feel wrong, raise that as a separate task.
- **Editing code in this skill.** This skill is one-directional: code → doc
  only. If code needs to change, that is `sdp-sync-code` or `sdp-plan`.
- **Bulk syncing without reading.** Do not regenerate the whole doc from a
  template; preserve human-written prose where it is still accurate. Edit
  surgically.
