#!/bin/sh
# DEBUG-327: the bench's `cargo build --release` uplifts the real binary over the wrapper;
# put the wrapper back after every build so the launch keeps its Wayland wire log.
/usr/bin/cargo "$@"
rc=$?
W=/home/diggle/repos/quill/.claude/worktrees/ticket-327
if [ "$1" = build ] && [ $rc -eq 0 ]; then
  if ! grep -q DEBUG-327 "$W/target/release/quill" 2>/dev/null; then
    cp "$W/target/release/quill" "$W/target/release/quill-real.new" \
      && mv "$W/target/release/quill-real.new" "$W/target/release/quill-real"
    cp /home/diggle/.claude/jobs/dd9717c1/tmp/quill-wrapper.sh "$W/target/release/quill"
    chmod +x "$W/target/release/quill"
  fi
fi
exit $rc
