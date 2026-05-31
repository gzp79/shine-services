# SDP trigger evals

These are **trigger evals** for the `sdp-doc` / `sdp-code` skill pair. They don't
check output quality — they check *routing*: given a user prompt, does the right
skill fire and the wrong one stay quiet?

The two skills split by **artifact**:

- `sdp-doc` writes the **doc** (from a prompt, or to re-sync a doc that drifted
  behind the code).
- `sdp-code` writes the **code** (to realize a stable, untouchable doc).

The boundary is sharpest on drift prompts — and that's deliberately tested:

| Prompt | Right skill | Why |
|---|---|---|
| "doc is stale, sync it to the code" | `sdp-doc` | the **doc** must change |
| "doc says X but the code doesn't, fix the code" | `sdp-code` | the **code** must change |

Each file is the mirror of the other: `sdp-doc.eval.json`'s `should_trigger:false`
cases are `sdp-code.eval.json`'s `true` cases, and vice versa. That overlap is the
point — it measures *separation*, not just recall.

## Files

| File | Purpose |
|---|---|
| `sdp-doc.eval.json` | prompts that should / shouldn't trigger **sdp-doc** |
| `sdp-code.eval.json` | prompts that should / shouldn't trigger **sdp-code** |
| `run_trigger_eval.py` | the runner (see below) |

Eval format (one array per file):

```json
{ "query": "<user prompt>", "should_trigger": true, "note": "<why>" }
```

`note` is for humans; the runner ignores it.

## Running

Use `run_trigger_eval.py`. It drives the `claude -p` CLI — **no API key or
`anthropic` SDK needed** (OAuth login is enough) — runs each prompt to
completion, and scores by reading **which installed skill the model actually
invoked**. Run from the repo root.

```bash
M=us.anthropic.claude-sonnet-4-5-20250929-v1:0   # see "Model id" below

# sdp-doc — fires on doc work, stays quiet on code work; flags sdp-code cross-fires
python .claude/skills/sdp-doc/evals/run_trigger_eval.py \
  --eval-set .claude/skills/sdp-doc/evals/sdp-doc.eval.json \
  --skill sdp-doc --sibling sdp-code \
  --runs-per-query 2 --model "$M"

# sdp-code — the mirror
python .claude/skills/sdp-doc/evals/run_trigger_eval.py \
  --eval-set .claude/skills/sdp-doc/evals/sdp-code.eval.json \
  --skill sdp-code --sibling sdp-doc \
  --runs-per-query 2 --model "$M"
```

A case **passes** when the target skill's fire-rate lands on the right side of
`--trigger-threshold` (≥ for `should_trigger:true`, < for `false`). Whenever the
`--sibling` skill fires, the line is tagged `<-- CROSS-FIRE` and counted in the
summary — that's the signal the two descriptions have started to overlap.

Drop `--model` to test against your default (Opus) model.

### Model id

This machine talks to **Bedrock**, which wants the full model id. Plain aliases
like `claude-sonnet-4-6` return `400 invalid model identifier`. Use:

- everyday lower-cost: `us.anthropic.claude-sonnet-4-5-20250929-v1:0`
- (the CLI's 400 error helpfully prints the exact id to use if this drifts)

## When to re-run

Re-run after editing either skill's `description:` frontmatter — the description
is the entire trigger surface. A regression shows up as a **cross-fire** (the
sibling firing on a prompt that's the other skill's job) or a `should_trigger`
case flipping. Tighten the wording on the boundary and re-run.

## Why not skill-creator's `run_eval.py`

The bundled `scripts/run_eval.py` is unusable for this repo, which is why this
runner exists:

- **`select()` on a pipe** — it polls `claude -p` stdout with `select.select()`,
  which on Windows only accepts sockets, so every query dies with
  `WinError 10038` and is silently scored "did not trigger".
- **Probe-command indirection** — it registers the description as a throwaway
  slash command and watches for *that*, assuming the real skill isn't installed.
  Here both skills *are* installed, so the model invokes them by their real
  names and the probe is never called → false negatives even where `select()`
  works.

`run_trigger_eval.py` keys on the **real skill invocation** instead, which is
both the truthful production signal and portable across platforms.

## Caveat: it's a headless single-turn proxy

Each case is one fresh `claude -p` turn with no conversation history, so it
measures the description's pull in isolation — not how the skill behaves
mid-conversation. Treat a surprising score as *"investigate"*, not *"broken"*:
re-read the run, and remember that real sessions carry context these prompts
don't.
