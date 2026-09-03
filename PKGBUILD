# Maintainer: Daniel Baldwin <danielbaldwin47@gmail.com>
# Build from this checkout: README.md § Build, install and run.
#
# The Rust workspace is built straight from the working tree: nothing is
# downloaded except in prepare(), so `makepkg -f` needs the network once and
# build() runs offline (docs/architecture.md, "Packaging").
pkgname=quill
_appid=io.github.danielbaldwin47.Quill
pkgver=0.1.0.r80.g105c39f
pkgrel=1
pkgdesc="A long-form writing environment for Linux: plain Markdown, typography first"
arch=('x86_64')
url="https://github.com/danielbaldwin47/Quill"
license=('GPL-3.0-or-later' 'OFL-1.1')
depends=('gtk4' 'enchant' 'hicolor-icon-theme')
makedepends=('cargo')
optdepends=('hunspell-en_us: English spell checking')
source=()

# The version the workspace names, plus the commit count: 0.1.0.rN.gHASH.
# Read out of `[workspace.package]` by name rather than off the first `version`
# line, so a `version` added to another table cannot quietly rename the package.
pkgver() {
  cd "$startdir"
  local version
  version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
  printf '%s.r%s.g%s' "$version" "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
}

prepare() {
  cd "$startdir"
  cargo fetch --locked
}

build() {
  cd "$startdir"
  # Compiled in by `quill-engine`'s `data` module: an installed Quill finds its
  # Faces, Templates and OFL.txt under /usr/share/quill with no variable set.
  QUILL_DATA_DIR="/usr/share/$pkgname" cargo build --release --locked --offline
}

package() {
  local share="$pkgdir/usr/share"
  local data="$share/$pkgname"

  install -Dm755 "$startdir/target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"

  # The data directory: the Faces today, Templates and the Style check lists as
  # their Pieces land. One directory, so an installed build and a development
  # build differ in one path rather than in every lookup.
  install -dm755 "$data/fonts" "$data/templates" "$data/data"
  install -m644 "$startdir"/fonts/*.ttf "$data/fonts/"
  # One licence per set of files in there: the Faces' own, then Inter's and
  # Source Serif 4's, which ship unmodified and carry their own.
  install -m644 "$startdir"/fonts/OFL*.txt "$data/fonts/"

  # The Omarchy template (README.md § Theme Quill with the desktop): a writer
  # copies it into their own themed/ directory, so it is installed where the
  # README can name it rather than into Omarchy's tree, which is Omarchy's.
  install -m644 "$startdir/packaging/quill.toml.tpl" "$data/quill.toml.tpl"

  install -Dm644 "$startdir/packaging/$_appid.desktop" "$share/applications/$_appid.desktop"
  install -Dm644 "$startdir/packaging/$_appid.svg" \
    "$share/icons/hicolor/scalable/apps/$_appid.svg"

  install -Dm644 "$startdir/LICENSE" "$share/licenses/$pkgname/LICENSE"
  install -m644 "$startdir"/fonts/OFL*.txt "$share/licenses/$pkgname/"
  install -Dm644 "$startdir/README.md" "$share/doc/$pkgname/README.md"
}
