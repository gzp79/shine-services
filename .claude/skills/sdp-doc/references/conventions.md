# SDP Doc Conventions

These rules govern every `docs/<feature>.html` produced or modified by `sdp-doc`
and `sdp-code`. Follow them exactly. Deviations break the skills' ability
to find and reason about the doc.

This file lives in `.claude/skills/sdp-doc/references/` and is referenced from
`.claude/skills/sdp-code/` via a relative path — keep it canonical here.

---

## File layout

```
docs/
  doc.css                      shared stylesheet (committed)
  client/web/<feature>.html    web game client docs
  services/identity/<feature>.html
  core/game/<feature>.html
  shared/<feature>.html        cross-cutting docs
  superpowers/specs/...        legacy markdown — leave alone
```

Rules:

- **Domain-scoped paths.** Feature docs live under a domain subfolder matching
  the code they document. Examples: `docs/client/web/input-system.html`,
  `docs/services/identity/auth-flow.html`.
- **One feature, one file.** Filename kebab-case. No `-spec`/`-design`/date/version suffixes.
- **Stylesheet is shared.** Every doc links `doc.css` via a relative path that
  reaches `docs/doc.css`. Adjust depth: `../../doc.css` from a two-level subfolder,
  `../doc.css` from one level. Never inline CSS, never duplicate `doc.css`.
- **GitHub raw friendly.** Doc must render correctly opened from GitHub raw —
  that's why CSS is a relative path to the committed sibling and there is no build step.
- `docs/doc.css` should already be present — it is the source of truth for the
  visual language. Do not regenerate it.

---

## Document skeleton

```html
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title><Feature Name></title>
<!-- depth-adjusted path to docs/doc.css: ../../doc.css from a two-level
     domain folder (the normal case), ../doc.css from one level. -->
<link rel="stylesheet" href="../../doc.css">
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
<!-- diagram in the layout that fits the flow — see Architecture below -->

<h2>Key rules</h2>
<!-- decision matrices as <table>; status as <span class="tag …"> -->

<h2>File map</h2>
<!-- table: file → role -->

<!-- Optional <details data-agent="…"> blocks — include ONLY where they carry
     durable insight the code can't show (see Collapsibles). A finished feature
     may have none. Example of one that earns its place: -->
<details data-agent="implementation">
  <summary>Why conflict resolution is centralized</summary>
  <div class="details-body"> … the non-obvious reasoning / gotcha … </div>
</details>

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

- **Always a diagram, not prose**, unless the architecture truly is a single
  component with no internal flow.
- All diagrams are built from the same `.box` primitive (below). What changes
  is the **layout** wrapping them. Pick the layout that matches the *shape of
  the flow* — don't force every feature into the layered stack.
- These class names are the contract — `doc.css` only styles these, so don't
  invent your own.

**Pick a layout:**

| Shape of the flow | Layout | Use when |
|---|---|---|
| Pipeline through stages | **Layered stack** (`.canvas`) | Data passes top→bottom through named layers (e.g. input → schema → state → system). The default. |
| Short linear chain | **Horizontal flow** (`.hflow`) | A → B → C cause/effect or request/response that reads naturally left-to-right and has no layer grouping. |
| One source, many reactors | **Fan-out** (`.fanout`) | An emitter (event/signal) feeds N peers that each react independently — observer/event-bus flows. Makes "each subscribes on its own" visible. |
| State chain that loops back | **Lifecycle** (`.lifecycle`) | States advance A→B→C then *recycle* to the start — load/unload, allocate→use→reclaim, pooled resources. The recycle edge is the point; a plain chain can't draw it without duplicating a state. |
| Intrinsically geometric | **Spatial SVG** (`.svg-diagram`) | The relationship *is* geometry — grid-cell ownership, adjacency, coordinate frames. No arrangement of boxes can show it; only a true drawing can. The rarest layout — reach for it only when a box diagram would lie. |

A doc can use more than one (e.g. a fan-out for the event, a small table for
the rules). Choose per-flow, not per-doc.

**The layout set is extensible, but reuse first.** This list is not closed — a
new flow shape can earn a new layout. But strongly prefer an existing layout:
most flows are a stack, a chain, or a fan-out in disguise, and a forced fit of
an existing layout still renders correctly, whereas an invented class name
renders as unstyled text. Only add a layout when an existing one would genuinely
distort the flow. When you do:
1. Add the layout's classes to `docs/doc.css` (follow the existing naming:
   `kebab-case`, layout-structural only — reuse `.box` and the `box-*` /
   `arr-*` colour variants, don't add cosmetics).
2. Add a row to the layout table above and a short `#### Layout N` example here,
   so the next run can find and reuse it.
3. Mention the addition in your hand-off so the user knows `doc.css` changed.

**Shared box anatomy.** Every layout uses this box; label/detail live in fixed
inner classes — `<b>`/`<p>`/free text will NOT be styled:

```html
<div class="box box-blue" style="flex:1">
  <div class="bname">BoxName</div>
  <div class="bdetail">one or two lines<br>use &lt;br&gt; to wrap</div>
</div>
```

**Semantic colours** (defined in `doc.css`) — pick consistently:
- `box-indigo` — raw input / external sources
- `box-teal` — adapters / schemas / translators (input→intent layer)
- `box-blue` — orchestrators / managers
- `box-green` — state holders / pollable data
- `box-orange` — events / signals
- `box-violet` — domain entities
- `box-pink` — output / consumers
- `box-dim` — a node that participates but is de-emphasised (e.g. an upstream
  box already detailed elsewhere). **Not** a license to draw non-participants —
  see the scope rule below. Use rarely.

**A diagram shows only what participates in that flow.** Every box is a
participant; drawing a node implies it takes part. Something that is *not* part
of the flow — a component that happens to work without subscribing, an
alternative that was rejected, a neighbour that's merely related — does **not**
get a box. That belongs in the orientation `<p>` under the diagram, or a
`<details>` block, as prose. Adding out-of-scope nodes (even dimmed) is the most
common way a diagram misleads: it makes a non-actor look like an actor.

Arrow/branch colour matches the box it leads into. **Layout hints are inline,
cosmetics are not:** `style="flex:N"`, `style="min-height:Npx"`,
`style="height:Npx"` on stems are required by the primitives and belong inline;
colours, borders, fonts, backgrounds never do.

#### Layout 1 — Layered stack (`.canvas`)

`layer-label` is the row's right-aligned caption; `layer-body` holds the boxes
(flex-weighted). Between rows, an **`.arrow-row`** carries one or more labeled
arrows. Each arrow is its own `.arrow-item arr-<colour>` column, so a row can
show **several parallel transitions** side-by-side — use that instead of
cramming multiple conditions into one caption.

```html
<div class="canvas">
  <div class="layer-row">
    <div class="layer-label">Stage name</div>
    <div class="layer-body">
      <div class="box box-indigo" style="flex:1"><div class="bname">A</div></div>
      <div class="box box-indigo" style="flex:2"><div class="bname">B</div></div>
    </div>
  </div>
  <div class="arrow-row" style="min-height:32px">
    <div class="arrow-spacer"></div>          <!-- aligns under layer-label -->
    <div class="arrow-body">
      <div class="arrow-item arr-blue" style="flex:1">
        <div class="arrow-stem" style="height:8px"></div>
        <div class="arrow-label">what flows down this path</div>
        <div class="arrow-stem" style="height:8px"></div>
        <div class="arrow-head"></div>
      </div>
      <!-- add more .arrow-item for parallel paths -->
    </div>
  </div>
  <!-- next layer-row … -->
</div>
```

Compound boxes (one header over split children) use `.box-compound-header` +
`.box-compound-body`. If the stack passes ~6 layer rows the feature is too big
for one doc — split it.

#### Layout 2 — Horizontal flow (`.hflow`)

Boxes in a row joined by `.hconn` connectors (a `→` with an optional
`.hconn-label`). For short chains that read across:

```html
<div class="hflow">
  <div class="box box-indigo"><div class="bname">Request</div></div>
  <div class="hconn"><div class="hconn-label">validate</div></div>
  <div class="box box-blue"><div class="bname">Handler</div></div>
  <div class="hconn"><div class="hconn-label">persist</div></div>
  <div class="box box-green"><div class="bname">Store</div></div>
</div>
```

#### Layout 3 — Fan-out (`.fanout`)

One source box, a trunk, then N branches that each react independently. Ideal
for an event consumed by several entities (the branch labels say what each one
does, making the independence explicit):

```html
<div class="fanout">
  <div class="fanout-source">
    <div class="box box-orange"><div class="bname">EVENT_NAME</div>
      <div class="bdetail">payload shape</div></div>
  </div>
  <div class="fanout-trunk"></div>
  <div class="fanout-branches">
    <div class="fanout-branch">
      <div class="fanout-branch-label">shifts its position</div>
      <div class="box box-green"><div class="bname">ConsumerA</div></div>
    </div>
    <div class="fanout-branch">
      <div class="fanout-branch-label">rebuilds its queue</div>
      <div class="box box-green"><div class="bname">ConsumerB</div></div>
    </div>
  </div>
</div>
```

#### Layout 4 — Lifecycle (`.lifecycle`)

A forward state chain (reusing `.hflow`) plus a `.cycle-return` back-edge that
states how the last state returns to the first. Use when a resource cycles
rather than terminates:

```html
<div class="lifecycle">
  <div class="hflow">
    <div class="box box-dim"><div class="bname">Unloaded</div></div>
    <div class="hconn"><div class="hconn-label">enters range</div></div>
    <div class="box box-orange"><div class="bname">Queued</div></div>
    <div class="hconn"><div class="hconn-label">idle-time slice</div></div>
    <div class="box box-green"><div class="bname">Loaded</div></div>
  </div>
  <div class="cycle-return">leaves range / over budget → torn down, back to Unloaded</div>
</div>
```

#### Layout 5 — Spatial SVG (`.svg-diagram`)

For a relationship that *is* geometry, an inline `<svg class="svg-diagram">` is
the diagram. This is the one place hand-authored SVG is allowed — and only
because boxes genuinely cannot express position/adjacency. Rules that keep it
conformant with the rest of the visual language:

- **No hard-coded colours on SVG elements.** `fill`/`stroke` come from the
  `.svg-diagram` classes in `doc.css` (`.face`, `.edge-owned`, `.edge-shared`,
  `.corner-owned`, `.corner-shared`, `.label-strong`, `.label-mono`), exactly as
  box colours come from `box-*`. Geometry attributes (`points`, `cx`, `d`, `x`,
  `width`, `viewBox`) are structural and belong inline — they're the SVG
  equivalent of `flex:N`, not cosmetics.
- If the existing classes don't cover the shape, extend the `.svg-diagram`
  palette in `doc.css` (reuse-first, same as any layout addition) — don't inline
  a one-off colour.
- Keep it diagrammatic, not decorative: it explains a rule the prose then states
  in words. Pair it with a one-line caption.

```html
<svg class="svg-diagram" viewBox="0 0 320 200" width="320">
  <polygon class="face" points="…"/>
  <line class="edge-owned" x1="…" y1="…" x2="…" y2="…"/>
  <circle class="corner-owned" cx="…" cy="…" r="5"/>
  <text class="label-mono" x="…" y="…">q,r</text>
</svg>
```

A `<p>` *under* any diagram for a sentence or two of orientation is fine.

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

This is the anchor both skills use to find the code. Keep it accurate
and complete: every file that implements (or is intended to implement) the
feature should appear, and only those files. Generated files, tests, and
build artifacts go in collapsibles, not the file map.

---

## Collapsibles — the agent surface

`<details data-agent="…">` blocks carry detail aimed at an agent rather than a
human reader. **Always collapsed by default** (no `open` attribute). They are
the exception, not the default — reach for them only when the alternatives below
genuinely fall short.

### Prefer a source reference over inline detail

Once code exists, the code *is* the detail. Before writing a block, ask whether
a pointer would serve better:

- The **file map** already names where each part lives — lean on it.
- For a specific behaviour, cite the file (and symbol) inline:
  `see <code>src/input/manager.rs · resolve_conflict</code>`.
- A reader who needs the concrete logic should be sent to the code, not handed
  a paraphrase of it that will drift the moment the code changes.

A block earns its place only when it says something the code **cannot show at a
glance**.

### Keep a block only if it carries durable insight

Keep it when it does one of these:

- **Summarizes** what would otherwise take reading many files — e.g. a test
  overview as one-liners, a map of allowed/blocked cases.
- Explains **non-trivial complexity** or the reasoning behind a design choice
  the code can't state for itself (a "why", not a "what").
- Flags a **surprise / gotcha** — an ordering constraint, a sharp edge, a
  non-obvious invariant someone will trip over.
- Requires **deeper understanding of a topic** to act safely — threat surface,
  allocation budget, an a11y requirement with rationale.

If the block is just routine detail the code states plainly (obvious signatures,
a mechanical step list, a restatement of what a function obviously does), it is
**transient scaffolding**: acceptable as a last resort *while the code does not
exist yet*, to carry intent to whoever implements it — but remove it once the
code lands. Optionally add `data-transient="true"` **alongside** its
`data-agent` tag (additive, not a replacement) so a later Sync knows to drop it
once the corresponding code is in.

Litmus test before keeping a block on a built feature: *would a competent agent
who has read the code and file map still learn something from this?* If no,
delete it.

### Tags

Tag each block with `data-agent="<type>"`. The taxonomy is open — pick the tag
that fits. Common tags:

| Tag | Use for |
|-----|---------|
| `data-agent="implementation"` | non-obvious complexity, key invariants, the "why" behind a choice |
| `data-agent="test"` | test overview / case map — the shape of coverage, not a restatement of each test |
| `data-agent="security"` | threat surface, abuse cases, validation rationale |
| `data-agent="performance"` | hot paths, allocation budgets, profiling targets |
| `data-agent="accessibility"` | a11y requirements and their rationale |
| `data-agent="migration"` | one-shot migration notes (transient; remove once done) |

Other tags are fine if they fit. Both skills filter by tag, so consistency
within a feature matters more than the exact label.

Inside a collapsible:

- Lead with a one-line summary in `<summary>`.
- Body wrapped in `<div class="details-body">`.
- Test plans use `.test-section` / `.test-list` / `.tag.t-block|t-allow|t-event|t-state`
  as defined in `doc.css`.
- Implementation notes can use `<pre><code>` for a signature that anchors the
  discussion, but keep it short and point at the source for the full thing —
  full code samples live in the codebase, not the doc.

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
- **Inline *cosmetic* CSS or `<style>` blocks.** Colours, borders, fonts,
  backgrounds, spacing — all live in `doc.css`. The only inline styles allowed
  are the diagram layout hints the primitives require (`flex:N` on a box,
  `min-height:Npx` on an arrow row); see *Architecture*.
- **Walls of prose.** If a paragraph is more than ~4 sentences, ask whether a
  flow diagram, table, or collapsible would carry it better.

---

## Self-review checklist

Run this against every doc before handing back to the user (the user
reviews and commits — the skills never do):

- [ ] Filename is `docs/<feature>.html`, kebab-case, no `-spec` / `-design` /
      date / version suffixes.
- [ ] `<link rel="stylesheet">` to `doc.css` present via a depth-adjusted
      relative path that reaches `docs/doc.css`; no inline *cosmetic* CSS
      (diagram `flex`/`min-height` layout hints are fine).
- [ ] Header has feature name and one-line purpose; no date, no status line.
- [ ] Architecture has a diagram in a layout that fits the flow (layered stack /
      horizontal / fan-out), or a strong justification for prose.
- [ ] Box colours and arrow colours follow the semantic palette.
- [ ] Every box in a diagram participates in that flow — no out-of-scope nodes
      (non-participants belong in prose, not a box, even dimmed).
- [ ] File map is two columns (File, Role) — no status column, no change
      badges.
- [ ] No `new` / `refactor` / `keep` / `removed` badges anywhere in the doc.
- [ ] Every `<details>` is collapsed (no `open`) and tagged with
      `data-agent="…"`.
- [ ] No change-log, no dates, no status lines, no TBDs, no inline cosmetic CSS.
- [ ] Read fresh, the doc accurately predicts the code (or, for a not-yet-built
      feature: predicts what the code *will* be).

---

## Conformance check

`sdp-doc` (Sync/Update modes) and `sdp-code` run this check against the
target doc **before** doing their own work. Only `sdp-doc` Fix mode patches the
violations.

The check is the self-review checklist above plus these extra items the
skills depend on:

- [ ] File map exists if the feature has code attached.
- [ ] File map paths actually exist in the repo (or are explicitly marked
      target-state in the architecture for files yet to be created).
- [ ] No agent block restates what the code states plainly. A built feature may
      have zero blocks — that's healthy, not a gap. Blocks that remain earn it
      by carrying durable insight (summary / non-trivial complexity / gotcha /
      rationale), per *Collapsibles*.
- [ ] Any `data-transient="true"` block still corresponds to unbuilt code. If
      the code now exists, the block should have been removed.

### Severity

Violations are sorted into two tiers, because the cost of fixing them
differs:

**Cosmetic** — one-line, mechanical, can be applied inline with user approval:

- Filename suffix (`-spec`, `-design`, dates) — plain rename via the
  filesystem; the user stages the rename when committing.
- Untagged `<details>` block — needs one `data-agent="…"` attribute.
- Date or status line in the header — delete a line.

**Structural** — touches diagram, file map, or content; needs `sdp-doc` Fix
mode:

- Change badges in the diagram, file map, or anywhere in the body.
- Status column in the file map.
- Inline `<style>` blocks, or inline *cosmetic* styles (colour/border/font/
  background) not sourced from `doc.css`. Diagram layout hints (`flex`,
  `min-height`) are allowed and are not a violation.
- Change-log narrative paragraphs / "today vs proposed" tables.
- Roadmap / future placeholder content in the architecture diagram.
- Missing file map when one is required.

### How to report

Group findings by tier, then offer:

> "Conformance check found:
>
> Cosmetic (can fix inline): <list>
> Structural (needs sdp-doc Fix mode): <list>
>
> Fix the cosmetic items inline now? Hand off the structural items to
> `sdp-doc` Fix mode?"

Handling depends on which skill is running:

- **`sdp-doc`** owns doc edits. If only cosmetic items exist, switch to Fix mode
  (with user approval), apply them, and resume. If any structural items exist,
  Fix mode is required before Sync/Update can proceed.
- **`sdp-code`** never edits the doc. Cosmetic items → note and proceed (they
  don't impede reading the doc). Any structural item → hand off to `sdp-doc`
  Fix mode and stop, then re-invoke once the doc is conformant.

When reporting an untagged `<details>` block, the checking skill **may** propose
a `data-agent` tag if the content makes it obvious (e.g., a block full of
test lists → `data-agent="test"`; a block of type signatures and edge cases
→ `data-agent="implementation"`). The proposed tag must come from the
taxonomy listed above — never invent new tags. If the content is mixed or
unclear, just flag the block as untagged and let Fix mode ask.

When Fix mode will rename the file (filename-suffix violations), call this out
so the user knows to re-invoke against the new path:

> "Note: Fix mode will rename `<old>.html` → `<new>.html`. After Fix mode,
> re-invoke against the new path."

The doc on disk is the only state — there is no resume checkpoint. For
`sdp-code`, a Fix-mode handoff means a hard restart: re-invoke from scratch
once the doc is conformant. For `sdp-doc`, Fix is just an internal mode switch,
then it resumes the original mode.
