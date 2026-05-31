#!/usr/bin/env python3
"""Trigger eval for the sdp-doc / sdp-code skill pair.

Measures the real production signal: given a user prompt, which installed
skill does `claude -p` actually invoke? A query passes when the target skill
fires (for should_trigger=true) or stays quiet (for false), and any time the
SIBLING skill fires it's reported as a CROSS-FIRE -- the failure mode that
matters most in a two-skill split, where the risk is sdp-doc and sdp-code
poaching each other's prompts.

It scans the stream-json for a `Skill` tool_use whose `skill` field names a
skill, so it sees exactly what the model chose in a real run -- not a proxy.
No API key needed (uses the OAuth `claude` CLI). Cross-platform.

This deliberately does NOT use skill-creator's bundled scripts/run_eval.py,
which is unusable here for two reasons:

  1. select() on a pipe. It polls `claude -p` stdout with select.select(),
     which on Windows only accepts sockets -> every query dies with
     WinError 10038 and is silently scored "did not trigger".
  2. Probe-command indirection. It registers the description as a throwaway
     slash command and watches for THAT command, assuming the real skill
     isn't installed. Here sdp-doc and sdp-code ARE installed, so the model
     invokes them by their real names and the probe is never called ->
     false negatives even where select() works. Keying on the real skill
     name (as this runner does) is both more correct and portable.

Model note: this machine talks to Bedrock. Use the Bedrock model id, e.g.
  --model us.anthropic.claude-sonnet-4-5-20250929-v1:0
Plain aliases like "claude-sonnet-4-6" are rejected with a 400.

Usage (from repo root):
  python .claude/skills/sdp-doc/evals/run_trigger_eval.py \
    --eval-set .claude/skills/sdp-doc/evals/sdp-doc.eval.json \
    --skill sdp-doc --sibling sdp-code \
    --runs-per-query 2 --model us.anthropic.claude-sonnet-4-5-20250929-v1:0
"""
import argparse
import json
import os
import subprocess
import sys
from pathlib import Path


def skills_invoked(stream):
    """Return the list of skill names invoked via the Skill tool in a run."""
    names = []
    for line in stream.splitlines():
        line = line.strip()
        if not line or '"Skill"' not in line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        msg = event.get("message", event)
        content = msg.get("content", []) if isinstance(msg, dict) else []
        for item in content:
            if (isinstance(item, dict) and item.get("type") == "tool_use"
                    and item.get("name") == "Skill"):
                s = item.get("input", {}).get("skill")
                if s:
                    names.append(s)
    return names


def run_query(query, timeout, model):
    cli = ["claude", "-p", query, "--output-format", "stream-json", "--verbose"]
    if model:
        cli += ["--model", model]
    env = {k: v for k, v in os.environ.items() if k != "CLAUDECODE"}
    try:
        proc = subprocess.run(
            cli, capture_output=True, text=True, encoding="utf-8",
            errors="replace", timeout=timeout, env=env,
            stdin=subprocess.DEVNULL,
        )
        return skills_invoked(proc.stdout)
    except subprocess.TimeoutExpired as e:
        return skills_invoked(e.stdout or "")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--eval-set", required=True)
    ap.add_argument("--skill", required=True, help="skill name that SHOULD fire, e.g. sdp-doc")
    ap.add_argument("--sibling", default=None, help="the other skill, to flag cross-fires")
    ap.add_argument("--runs-per-query", type=int, default=2)
    ap.add_argument("--trigger-threshold", type=float, default=0.5)
    ap.add_argument("--timeout", type=int, default=120)
    ap.add_argument("--model", default=None)
    args = ap.parse_args()

    eval_set = json.loads(Path(args.eval_set).read_text(encoding="utf-8"))
    print(f"Skill: {args.skill}  sibling: {args.sibling}  "
          f"model: {args.model or 'default'}", file=sys.stderr)

    passed = cross = 0
    for item in eval_set:
        q, want = item["query"], item["should_trigger"]
        hits = sibling_hits = 0
        for _ in range(args.runs_per_query):
            invoked = run_query(q, args.timeout, args.model)
            if args.skill in invoked:
                hits += 1
            if args.sibling and args.sibling in invoked:
                sibling_hits += 1
        rate = hits / args.runs_per_query
        ok = rate >= args.trigger_threshold if want else rate < args.trigger_threshold
        passed += ok
        flag = ""
        if sibling_hits:
            cross += 1
            flag = f"  <-- CROSS-FIRE: {args.sibling} fired {sibling_hits}/{args.runs_per_query}"
        print(f"  [{'PASS' if ok else 'FAIL'}] {args.skill}={hits}/{args.runs_per_query} "
              f"expected={want}: {q[:56]}{flag}", file=sys.stderr)

    print(f"Results: {passed}/{len(eval_set)} passed"
          + (f", {cross} cross-fire(s)" if args.sibling else ""), file=sys.stderr)


if __name__ == "__main__":
    main()
