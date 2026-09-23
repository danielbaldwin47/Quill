#!/bin/sh
# DEBUG-327: the bench launches target/release/quill; this keeps the pid (exec) and
# writes the client's Wayland wire log and GDK's per-frame timings beside the run's other probes.
exec env WAYLAND_DEBUG=1 GDK_DEBUG=frames /home/diggle/repos/quill/.claude/worktrees/ticket-327/target/release/quill-real "$@" 2>>"${QUILL_PROBE_ARCHIVE:-/tmp}/wayland-$$.log"
