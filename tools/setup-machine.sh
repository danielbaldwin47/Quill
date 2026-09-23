#!/usr/bin/env bash
# Set up a fresh Omarchy (Arch + Hyprland) machine to continue Quill development.
#
# Run it twice, in two places:
#
#   1. On the OLD machine:  tools/setup-machine.sh export <dest>
#      Bundles everything that lives outside this repo (Claude memory, global
#      Claude config, personal skills) into <dest> (a directory; copy it over
#      with a USB stick, rsync, whatever).
#
#   2. On the NEW machine:  tools/setup-machine.sh install [<bundle-dir>]
#      Installs packages, the Rust toolchain, npm deps, and (if a bundle dir is
#      given) the exported Claude state, re-slugged for the new checkout path.
#
# Before step 2 you need the repo itself, which needs auth (it is private):
#
#   sudo pacman -S --needed git github-cli   # or: mise use -g gh@latest
#   gh auth login                            # browser flow; grants git credentials too
#   gh repo clone danielbaldwin47/Quill ~/repos/quill
#   cd ~/repos/quill && tools/setup-machine.sh install <bundle-dir>
#
# Cloning to the same path (~/repos/quill) is not required — the install step
# re-slugs Claude's per-project directory from wherever the checkout sits.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
mode="${1:-}"

# The slug Claude Code derives a project's ~/.claude/projects/ directory from:
# the absolute path with every '/' (and '.') turned into '-'.
slug() { echo "$1" | sed 's![/.]!-!g'; }

export_state() {
  local dest="${1:?usage: setup-machine.sh export <dest-dir>}"
  mkdir -p "$dest"
  local proj="$HOME/.claude/projects/$(slug "$repo")"

  # Project memory: the auto-memory files for this repo. These live only in
  # ~/.claude, keyed to the checkout's absolute path — they are not in git.
  [ -d "$proj/memory" ] && cp -r "$proj/memory" "$dest/memory"

  # Global Claude config that the project's CLAUDE.md assumes exists.
  mkdir -p "$dest/claude-global"
  for f in CLAUDE.md settings.json statusline.sh; do
    [ -e "$HOME/.claude/$f" ] && cp "$HOME/.claude/$f" "$dest/claude-global/"
  done
  [ -d "$HOME/.claude/agents" ] && cp -r "$HOME/.claude/agents" "$dest/claude-global/agents"
  [ -d "$HOME/.claude/skills" ] && cp -r "$HOME/.claude/skills" "$dest/claude-global/skills"

  # Repo-local Claude settings that are gitignored (output style, local plugin
  # enables). The committed .claude/settings.json travels with the clone.
  [ -e "$repo/.claude/settings.local.json" ] && cp "$repo/.claude/settings.local.json" "$dest/"

  echo "exported to $dest — copy this directory to the new machine"
  echo "NOT exported (redo by hand on the new machine):"
  echo "  - gh auth login / git identity (git config --global user.name + user.email)"
  echo "  - the Wine prefix with iA Writer (~/.wine); reinstall iA Writer for"
  echo "    Windows under wine if the Design-oracle work needs a drivable app"
}

install_state() {
  local bundle="${1:-}"

  # --- System packages -------------------------------------------------------
  # rustup        - toolchain manager ('rust' via pacman conflicts with it)
  # gtk4 enchant  - the app's runtime deps (PKGBUILD depends)
  # hunspell-en_us- spell checking (PKGBUILD optdepends)
  # base-devel    - makepkg, for the Arch-package Hand tests
  # nodejs npm    - the Gate's browser tools and dev/legacy/
  # chromium      - tools/*.mjs launch it by absolute path (/usr/bin/chromium)
  # imagemagick   - shot cropping/judging helpers
  # python-fonttools - tools/fontbuild.py (only to rebuild fonts/ from dev/ref/ia)
  # wine          - drives iA Writer for Mac-parity questions (memory: ia-writer-is-drivable)
  # grim slurp    - screenshots (Omarchy ships these; --needed makes it a no-op)
  # mise-bin      - version manager; the old machine ran node, gh and claude from it
  # python-pillow - reading a capture's pixels from a one-off script (three sessions
  #                 crashed on `import PIL` in two days and fell back to imagemagick)
  sudo pacman -S --needed rustup gtk4 enchant hunspell-en_us base-devel \
    nodejs npm chromium imagemagick python-fonttools python-pillow wine grim slurp mise-bin

  # --- Rust toolchain --------------------------------------------------------
  rustup default stable
  rustup component add clippy rustfmt rust-analyzer rust-src
  # Arch's rustup does not ship a rust-analyzer proxy in /usr/bin, and the
  # rust-analyzer-lsp Claude plugin needs the binary on PATH: a symlink to
  # rustup dispatches to the component (this is how the old machine did it).
  mkdir -p "$HOME/.local/bin"
  ln -sf /usr/bin/rustup "$HOME/.local/bin/rust-analyzer"

  # --- Node deps (two manifests: Gate tools at the root, legacy app) --------
  (cd "$repo" && npm i)
  (cd "$repo/legacy" && npm i)

  # --- Claude Code -----------------------------------------------------------
  # The old machine runs claude, gh and node through mise:
  command -v claude >/dev/null || mise use -g claude@latest
  command -v gh >/dev/null || mise use -g gh@latest

  # --- Exported Claude state -------------------------------------------------
  if [ -n "$bundle" ]; then
    local proj="$HOME/.claude/projects/$(slug "$repo")"
    mkdir -p "$proj" "$HOME/.claude"
    [ -d "$bundle/memory" ] && cp -r "$bundle/memory" "$proj/memory"
    if [ -d "$bundle/claude-global" ]; then
      cp -n "$bundle/claude-global/CLAUDE.md" "$HOME/.claude/" 2>/dev/null || true
      cp -n "$bundle/claude-global/settings.json" "$HOME/.claude/" 2>/dev/null || true
      cp -n "$bundle/claude-global/statusline.sh" "$HOME/.claude/" 2>/dev/null || true
      [ -d "$bundle/claude-global/agents" ] && cp -rn "$bundle/claude-global/agents" "$HOME/.claude/" || true
      [ -d "$bundle/claude-global/skills" ] && cp -rn "$bundle/claude-global/skills" "$HOME/.claude/" || true
    fi
    [ -e "$bundle/settings.local.json" ] && cp "$bundle/settings.local.json" "$repo/.claude/"
  fi

  # --- Verify ---------------------------------------------------------------
  (cd "$repo" && cargo check -q --message-format=short)
  echo
  echo "install done. Remaining by-hand steps:"
  echo "  1. gh auth login  (private repo; also sets git credentials)"
  echo "  2. git config --global user.name/user.email"
  echo "  3. In the repo, run 'claude' once and install the plugins:"
  echo "       /plugin install rust-analyzer-lsp@claude-plugins-official  (project scope)"
  echo "       /plugin install mattpocock-skills@claude-plugins-official  (user scope)"
  echo "       /plugin install skill-creator@claude-plugins-official      (user scope)"
  echo "  4. Wine + iA Writer if driving the Design oracle: install the Windows"
  echo "     iA Writer into ~/.wine (see memory: ia-writer-is-drivable)"
  echo "  5. tools/gate check  — the Commit tier green means the machine works"
}

case "$mode" in
  export)  shift; export_state "$@" ;;
  install) shift; install_state "${1:-}" ;;
  *) sed -n '2,20p' "$0"; exit 1 ;;
esac
