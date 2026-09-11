#!/usr/bin/env bash
# PreToolUse guard on Bash: a file this session has opened with Read, Edit or
# Write changes through Edit. Any rewrite of such a file through Bash comes
# back into context as a diff snippet (CLAUDE.md § Context in an `/implement`
# session, "One tool per file"; #164 paid for nine, and eighteen sessions
# measured on 2026-09-11 routed around the sed refusal 52 times by rewriting
# the file from an inline python3 or node program instead, 27 of them in
# #319). The hook reads the tool call it is about to allow and refuses two
# shapes, each naming a file the session has opened:
#
#   - a `sed` or `perl` at command position with an in-place flag;
#   - an inline program — `python3 -c`, `python3 -`, a `python3 <<` heredoc,
#     `node -e`, a `node <<` heredoc, `perl -e` — whose text both names that
#     file and writes it: `open(..., 'w'|'a')`, `writeFileSync`, `writeFile(`,
#     `write_text(`, or a `>` redirection onto it.
#
# A program that only reads the file passes, and so does a script named by a
# path (`python3 tools/x.py`): the hook reads a command's text, not a file's.
# Everything else passes, and so does every doubt — no jq, no transcript, a
# path it cannot resolve, a quoted path with a space in it.
# tools/edited-files-guard-selftest.mjs is this hook's own test.
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

# An interpreter taking its program from the command line, from stdin or from a
# heredoc, rather than from a file — read off `bare`, so an interpreter quoted
# inside an echo or a commit message is not one, and a script named by a path
# (`python3 tools/x.py`), which carries none of these flags, is not one either.
inline='(^|[[:space:]|(&;])(python3?|node|perl)[[:space:]]+(-c([[:space:]]|$)|-e([[:space:]]|$)|--eval([[:space:]=]|$)|-([[:space:]]|$)|<<)'
program=""
grep -qE "$inline" <<< "$bare" && program=1

[ ${#targets[@]} -gt 0 ] || [ -n "$program" ] || exit 0

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

# What a program writing a file looks like, whatever file it names: Python's
# `open(..., 'w')` and `write_text(`, Node's `writeFileSync` and `writeFile(`.
# A `>` redirection is a write of one named file, so it is asked per file below.
quote="'"
writers="open\\([^)]*[${quote}\"][wa>]|writeFileSync|writeFile\\(|write_text\\("
program_hits=()
if [ -n "$program" ]; then
    writes=""
    grep -qE "$writers" <<< "$cmd" && writes=1
    for s in "${seen[@]}"; do
        rel=${s#"$cwd"/}
        # Named by the program's text, absolutely or as the session would write it.
        case "$cmd" in *"$s"* | *"$rel"*) ;; *) continue ;; esac
        if [ -z "$writes" ]; then
            escaped=$(sed -E 's/[][^$.*\\\/+?(){}|]/\\&/g' <<< "$s")
            escaped_rel=$(sed -E 's/[][^$.*\\\/+?(){}|]/\\&/g' <<< "$rel")
            grep -qE ">[[:space:]]*[${quote}\"]?($escaped|$escaped_rel)" <<< "$cmd" || continue
        fi
        program_hits+=("$rel")
    done
fi

[ ${#hits[@]} -gt 0 ] || [ ${#program_hits[@]} -gt 0 ] || exit 0

named() { printf '%s, ' "$@" | sed 's/, $//'; }
reasons=()
if [ ${#hits[@]} -gt 0 ]; then
    reasons+=("$(named "${hits[@]}") was opened with Read, Edit or Write this session, so it changes through Edit: an in-place sed comes back as a diff snippet (CLAUDE.md § Context in an /implement session, one tool per file).")
fi
if [ ${#program_hits[@]} -gt 0 ]; then
    reasons+=("$(named "${program_hits[@]}") was opened with Read, Edit or Write this session, and this program rewrites it: change it with Edit. A file rewritten through Bash after the file tools have seen it comes back as a diff snippet — 52 python3 and node rewrites in eighteen sessions (CLAUDE.md § Context in an /implement session, one tool per file).")
fi
reason=$(printf '%s ' "${reasons[@]}")
jq -cn --arg reason "${reason% }" '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: $reason}}'
