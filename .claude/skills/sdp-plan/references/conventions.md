# SDP Doc Conventions

These rules govern every `docs/<feature>.html` produced or modified by `sdp-plan`,
`sdp-sync-doc`, and `sdp-sync-code`. Follow them exactly. Deviations break the
sync skills' ability to find and reason about the doc.

The same file is referenced from `.claude/skills/sdp-sync-doc/` and
`.claude/skills/sdp-sync-code/` via relative paths — keep it canonical here.

---

## File layout

```
docs/
  doc.css                shared stylesheet for all docs (committed)
  <feature>.html         one file per feature
  <other-feature>.html
  superpowers/specs/...  legacy markdown specs — leave alone, do not migrate proactively
```

Rules:

- **One feature, one file.** Filename is the feature in kebab-case. No
  `-spec`, `-design`, date prefixes, or version suffixes. Examples:
  `input-system.html`, `auth-flow.html`, `mesh-topology.html`.
- **Stylesheet is shared.** Every doc links `./doc.css`. Never inline CSS.
  Never duplicate `doc.css`.
- **GitHub raw friendly.** The doc must render correctly when opened directly
  from GitHub raw — that's why `./doc.css` is a sibling and there is no build
  step.
- If `docs/doc.css` does not exist when `sdp-plan` runs, copy the canonical
  version from this project (it should already be present at
  `docs/doc.css`). Do not regenerate it from scratch — it is the source of
  truth for the visual language.

---

## Document skeleton

```html
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title><Feature Name></title>
<link rel="stylesheet" href="./doc.css">
</head>
<body>

<header class="doc-header">
  <h1 class="doc-title"><Feature Name></h1>
  <div class="doc-meta">
    <span><one-line purpose></span>
  </div>
</header>

<h2>Problem</h2>
<p>2–4 sentences. What the feature does and why it exists.</p>

<h2>Architecture</h2>
<!-- flow diagram (see below) -->

<h2>Key rules</h2>
<!-- decision matrices as <table>; status as <span class="tag …"> -->

<h2>File map</h2>
<!-- table: file → role -->

<details data-agent="implementation">
  <summary>Implementation notes</summary>
  <div class="details-body"> … </div>
</details>

<details data-agent="test">
  <summary>Test plan</summary>
  <div class="details-body"> … </div>
</details>

<!-- additional <details data-agent="…"> blocks as needed -->

</body>
</html>
```

The skeleton is a guide, not a straitjacket. Sections may be reordered or
omitted if a feature genuinely doesn't have them — but **do not add sections
the skeleton omits**, especially not change-log or history sections.

---

## Section rules

### Problem

- 2–4 sentences. What the feature does and the user-visible reason it exists.
- No diff narrative ("we used to do X, now we do Y"). The doc is the desired
  state.

### Architecture

- **Always a flow diagram, not prose**, unless the architecture truly is a
  single component with no internal flow.
- Use the `.canvas` / `.layer-row` / `.box` / `.arrow-row` primitives from
  `doc.css`. The semantic colour names (`box-indigo`, `box-blue`, `box-green`,
  `box-orange`, `box-violet`, `box-pink`) carry meaning — pick consistently:
  - `box-indigo` — raw input / external sources
  - `box-blue` — orchestrators / managers
  - `box-green` — state holders / pollable data
  - `box-orange` — events / signals
  - `box-violet` — domain entities
  - `box-pink` — output / consumers
- Compound boxes (header + split children) use `.box-compound-header` /
  `.box-compound-body`.
- Arrow colour matches the box it leads into.
- One short caption per arrow row (label-text), not paragraphs.
- A `<p>` *under* the diagram for one or two sentences of orientation is fine.
- If the diagram balloons past ~6 layer rows, the feature is too big for one
  doc. Split it.

### Key rules

- Decision matrices, conflict tables, state transitions → `<table>`.
- Inline status indicators → `<span class="tag good|bad|warn|neutral">…</span>`.
- Each row should fit on one line where possible. If a cell needs more, the
  rule probably belongs in a `<details>` block instead.

### File map

Required for every doc with code attached. Format:

```html
<table>
  <tr><th>File</th><th>Role</th></tr>
  <tr><td><code>path/to/file.ts</code></td><td>One-line role.</td></tr>
  …
</table>
```

Two columns only: **File** and **Role**. No status column, no change badges
(`new` / `refactor` / `keep` / `removed`). The map is the target-state set of
files; if a file doesn't belong, it shouldn't be listed.

This is the anchor the sync skills use to find the code. Keep it accurate
and complete: every file that implements (or is intended to implement) the
feature should appear, and only those files. Generated files, tests, and
build artifacts go in collapsibles, not the file map.

---

## Collapsibles — the agent surface

Every detail that aids an agent (rather than orienting a human) goes inside a
`<details>` block. **Always collapsed by default** (no `open` attribute).

Tag each block with `data-agent="<type>"`. The taxonomy is open — pick the tag
that fits. Common tags:

| Tag | Use for |
|-----|---------|
| `data-agent="implementation"` | concrete impl notes, type signatures, edge cases |
| `data-agent="test"` | test plan, blocked/allowed cases, fixture notes |
| `data-agent="security"` | threat surface, validation rules, abuse cases |
| `data-agent="performance"` | hot paths, allocation budgets, profiling targets |
| `data-agent="accessibility"` | a11y requirements |
| `data-agent="migration"` | one-shot migration notes (rare; remove once done) |

Other tags are fine if they fit. The sync skills filter by tag, so consistency
within a feature matters more than the exact label.

Inside a collapsible:

- Lead with a one-line summary in `<summary>`.
- Body wrapped in `<div class="details-body">`.
- Test plans use `.test-section` / `.test-list` / `.tag.t-block|t-allow|t-event|t-state`
  as defined in `doc.css`.
- Implementation notes can use `<pre><code>` for type signatures, but keep
  them short — full code samples live in the codebase, not the doc.

---

## What never appears in a doc

- **Change-log narrative.** No "today vs proposed", no "previously did X". The
  doc reflects the desired state.

  The boundary between "orientation prose" and "change-log narrative" is
  often subtle. Test: does the sentence still make sense if a new reader
  encounters it without knowing the prior version?

  - **OK** — "The input system maps raw device events to gesture and
    locomotion outputs."
  - **OK** — "Conflict resolution lives in `InputManager` so the rules are
    testable in one place."
  - **Change-log** — "The existing input system has three issues that this
    design addresses: …" (defines the doc as a diff against prior state).
  - **Change-log** — "Velocity is no longer stored as a world-space
    snapshot." (only meaningful if the reader knows it used to be).
  - **Change-log** — "We replaced `InputController` with `InputManager`."
    (talks about the transition, not the system).
- **Change badges anywhere.** `new` / `refactor` / `keep` / `removed` are
  change-log indicators. They never appear in the doc body, file map, or
  diagram boxes. The doc describes the target; what changed since the last
  version is git's job.
- **Roadmap content.** "Future / not yet implemented" boxes or sections.
  If something isn't built and isn't being built now, it doesn't appear in
  a feature doc.
- **Dates.** No "as of YYYY-MM-DD", no version stamps. Git carries history.
- **Status lines.** No "status: approved", no "draft", no "in progress". A
  committed doc is the current desired state by definition.
- **Assignment / ownership lines.** No "@person owns this".
- **TBD, TODO, placeholder.** Resolve before writing, or move into a
  `<details>` block as an explicit open question.
- **Inline CSS or `<style>` blocks.** All styling is in `doc.css`.
- **Walls of prose.** If a paragraph is more than ~4 sentences, ask whether a
  flow diagram, table, or collapsible would carry it better.

---

## Self-review checklist

Run this against every doc before handing back to the user (the user
reviews and commits — the skills never do):

- [ ] Filename is `docs/<feature>.html`, kebab-case, no `-spec` / `-design` /
      date / version suffixes.
- [ ] `<link rel="stylesheet" href="./doc.css">` present; no inline CSS.
- [ ] Header has feature name and one-line purpose; no date, no status line.
- [ ] Architecture has a flow diagram (or strong justification for prose).
- [ ] Box colours and arrow colours follow the semantic palette.
- [ ] File map is two columns (File, Role) — no status column, no change
      badges.
- [ ] No `new` / `refactor` / `keep` / `removed` badges anywhere in the doc.
- [ ] Every `<details>` is collapsed (no `open`) and tagged with
      `data-agent="…"`.
- [ ] No change-log, no dates, no status lines, no TBDs, no inline CSS.
- [ ] Read fresh, the doc accurately predicts the code (or, for `sdp-plan` on
      a not-yet-built feature: predicts what the code *will* be).

---

## Conformance check (for sync skills)

`sdp-sync-doc` and `sdp-sync-code` run this check against the target doc
**before** doing their own work. They do not patch conformance issues
themselves — that's `sdp-plan`'s job.

The check is the self-review checklist above plus these extra items the sync
skills depend on:

- [ ] File map exists if the feature has code attached.
- [ ] File map paths actually exist in the repo (or are explicitly marked
      target-state in the architecture for files yet to be created).
- [ ] At least one `<details data-agent="implementation">` or
      `<details data-agent="test">` block, if the doc covers non-trivial
      behaviour.

### Severity

Violations are sorted into two tiers, because the cost of fixing them
differs:

**Cosmetic** — one-line, mechanical, can be applied inline with user approval:

- Filename suffix (`-spec`, `-design`, dates) — plain rename via the
  filesystem; the user stages the rename when committing.
- Untagged `<details>` block — needs one `data-agent="…"` attribute.
- Date or status line in the header — delete a line.

**Structural** — touches diagram, file map, or content; needs `sdp-plan` fix
mode:

- Change badges in the diagram, file map, or anywhere in the body.
- Status column in the file map.
- Inline `<style>` blocks or non-`./doc.css` styles.
- Change-log narrative paragraphs / "today vs proposed" tables.
- Roadmap / future placeholder content in the architecture diagram.
- Missing file map when one is required.

### How sync skills should report

Group findings by tier, then offer:

> "Conformance check found:
>
> Cosmetic (can fix inline): <list>
> Structural (needs sdp-plan fix mode): <list>
>
> Fix the cosmetic items inline now? Hand off the structural items to
> `sdp-plan`?"

If only cosmetic items exist, the sync skill may fix them inline (with user
approval) and continue. If any structural items exist, hand off to `sdp-plan`
fix mode and stop — do not proceed with sync until the doc is conformant.

When reporting an untagged `<details>` block, the sync skill **may** propose
a `data-agent` tag if the content makes it obvious (e.g., a block full of
test lists → `data-agent="test"`; a block of type signatures and edge cases
→ `data-agent="implementation"`). The proposed tag must come from the
taxonomy listed above — never invent new tags. If the content is mixed or
unclear, just flag the block as untagged and let fix mode ask.

When fix mode will rename the file (filename-suffix violations), the sync
skill should call this out in its handoff message so the user knows to
re-invoke against the new path:

> "Note: fix mode will rename `<old>.html` → `<new>.html`. After fix mode,
> re-invoke me against the new path."

After fix mode finishes, the sync is a hard restart: the user re-invokes the
sync skill from scratch. There is no resume point — the doc on disk is the
state.
