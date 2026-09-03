#!/usr/bin/env bash
# PreToolUse guard on Bash: `tools/gate judge <piece>` runs in the background
# and `tools/gate bench` in the foreground (docs/agents/gate.md § Ticket tier,
# "As run"). A judge that asks more than one critic outlasts a foreground
# call's ten-minute cap and is killed with nothing recorded — twenty runs and
# 190 minutes between 2026-08-28 and 2026-09-01, the last after the rule was
# written — and a backgrounded bench loses its keys on the first injection.
# The hook reads the call it is about to allow and refuses only those two
# shapes at command position: `tools/gate judge` without `run_in_background`,
# except `judge latency`, which is arithmetic and takes a second; and
# `tools/gate bench` with it. Everything else passes, and so does every doubt.
set -uo pipefail
set -f # the command is split into words below, and a `*` in it stays a `*`

command -v jq > /dev/null 2>&1 || exit 0
input=$(cat)
cmd=$(jq -r '.tool_input.command // empty' <<< "$input")
[ -n "$cmd" ] || exit 0
background=$(jq -r '.tool_input.run_in_background // false' <<< "$input")

# The command with its quoted strings removed, so a `gate judge` inside a commit
# message or an echo is not a judge.
bare=$(sed -E "s/\"[^\"]*\"//g; s/'[^']*'//g" <<< "$cmd")
# One segment per simple command: split on the operators between them.
mapfile -t segments < <(sed -E 's/(&&|\|\||[;|()]|\$\()/\n/g' <<< "$bare")
judge=""
bench=""
for segment in "${segments[@]}"; do
    segment=${segment#"${segment%%[![:space:]]*}"}
    case "$segment" in
        tools/gate\ judge\ latency* | */tools/gate\ judge\ latency*) ;;
        tools/gate\ judge* | */tools/gate\ judge*) judge=1 ;;
        tools/gate\ bench* | */tools/gate\ bench*) bench=1 ;;
    esac
done

reasons=()
if [ -n "$judge" ] && [ "$background" != true ]; then
    reasons+=("tools/gate judge spends one critic per state, five or six minutes each, and outlasts a foreground call's ten-minute cap — twenty runs were killed there with nothing recorded. Run it with run_in_background: true, output to a file under the job's tmp directory, and end the turn (docs/agents/gate.md § Ticket tier, As run); tools/gate judge latency is arithmetic and runs in the foreground.")
fi
if [ -n "$bench" ] && [ "$background" = true ]; then
    reasons+=("tools/gate bench runs in the foreground: a background run loses its keys on the first injection. A whole --all run is about six minutes, inside the cap (docs/agents/gate.md § Ticket tier, As run).")
fi
[ ${#reasons[@]} -gt 0 ] || exit 0

reason=$(printf '%s ' "${reasons[@]}")
jq -cn --arg reason "${reason% }" '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: $reason}}'
