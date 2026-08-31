#!/usr/bin/env bash
# PreToolUse guard on Bash: a file this session has opened with Read, Edit or
# Write changes through Edit. An in-place sed or perl on such a file comes back
# into context as a diff snippet (CLAUDE.md § Context in an `/implement`
# session, "One tool per file"; #164 paid for nine). The hook reads the tool
# call it is about to allow and refuses only that shape: a `sed` or `perl` at
# command position with an in-place flag, naming a file the session has
# opened. Everything else passes, and so does every doubt — no jq, no
# transcript, a path it cannot resolve, a quoted path with a space in it.
set -uo pipefail
set -f # the command is split into words below, and a `*` in it stays a `*`

command -v jq > /dev/null 2>&1 || exit 0
input=$(cat)
cmd=$(jq -r '.tool_input.command // empty' <<< "$input")
[ -n "$cmd" ] || exit 0

# The command with its quoted strings removed, so a `sed -i` inside an echo or
# a grep is not a sed, and a sed's own expression is not a file name.
bare=$(sed -E "s/\"[^\"]*\"//g; s/'[^']*'//g" <<< "$cmd")
# One segment per simple command: split on the operators between them.
mapfile -t segments < <(sed -E 's/(&&|\|\||[;|()]|\$\()/\n/g' <<< "$bare")
inplace='(^|[[:space:]])(-[a-zA-Z]*i[a-zA-Z.]*|--in-place(=[^[:space:]]*)?|-[0-9]*pi[a-zA-Z]*)([[:space:]]|$)'
targets=()
for segment in "${segments[@]}"; do
    segment=${segment#"${segment%%[![:space:]]*}"}
    case "$segment" in sed\ * | perl\ *) ;; *) continue ;; esac
    grep -qE "$inplace" <<< "$segment" || continue
    for word in $segment; do
        case "$word" in sed | perl | -* | /dev/* | "") continue ;; esac
        targets+=("$word")
    done
done
[ ${#targets[@]} -gt 0 ] || exit 0

transcript=$(jq -r '.transcript_path // empty' <<< "$input")
cwd=$(jq -r '.cwd // empty' <<< "$input")
[ -f "$transcript" ] || exit 0

# Every path the dedicated file tools have touched this session.
mapfile -t seen < <(jq -r '
    select(.type == "assistant")
    | .message.content[]?
    | select(.type == "tool_use" and (.name == "Read" or .name == "Edit" or .name == "Write"))
    | .input.file_path // empty' "$transcript" 2> /dev/null | sort -u)
[ ${#seen[@]} -gt 0 ] || exit 0

hits=()
for word in "${targets[@]}"; do
    if [ "${word:0:1}" = / ]; then abs=$word; else abs=$(realpath -m -- "$cwd/$word" 2> /dev/null) || continue; fi
    [ -f "$abs" ] || continue
    for s in "${seen[@]}"; do
        [ "$s" = "$abs" ] && hits+=("${abs#"$cwd"/}")
    done
done
[ ${#hits[@]} -gt 0 ] || exit 0

reason="$(printf '%s, ' "${hits[@]}" | sed 's/, $//') was opened with Read, Edit or Write this session, so it changes through Edit: an in-place sed comes back as a diff snippet (CLAUDE.md § Context in an /implement session, one tool per file)."
jq -cn --arg reason "$reason" '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: $reason}}'
