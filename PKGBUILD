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
license=('GPL-3.0-or-later' 'OFL-1.1' 'Apache-2.0' 'BSD-3-Clause' 'MIT' 'CC0-1.0')
depends=('gtk4' 'enchant' 'hunspell-en_us' 'hicolor-icon-theme')
makedepends=('cargo')
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

  # The data directory: the fonts and the Style check lists. One directory, so
  # an installed build and a development build differ in one path rather than
  # in every lookup. Templates are not here: they are compiled into the binary
  # (`quill_engine::template`).
  install -dm755 "$data/fonts" "$data/data/style" "$data/data/marks"
  install -m644 "$startdir"/fonts/*.ttf "$data/fonts/"
  # One licence per set of files in there: the Faces' own, then Inter's and
  # Source Serif 4's, which ship unmodified and carry their own.
  install -m644 "$startdir"/fonts/OFL*.txt "$data/fonts/"

  # The three lists `quill_engine::data::style()` reads, with the SOURCES.md
  # that says where each entry came from and the licence texts its sources
  # require to travel with the data.
  install -m644 "$startdir"/data/style/*.txt "$startdir/data/style/SOURCES.md" "$data/data/style/"
  # The Selection Mark's four glyphs `quill_engine::data::marks()` reads.
  install -m644 "$startdir"/data/marks/*.svg "$data/data/marks/"

  # The Omarchy template (README.md § Theme Quill with the desktop): a writer
  # copies it into their own themed/ directory, so it is installed where the
  # README can name it rather than into Omarchy's tree, which is Omarchy's.
  install -m644 "$startdir/packaging/quill.toml.tpl" "$data/quill.toml.tpl"

  install -Dm644 "$startdir/packaging/$_appid.desktop" "$share/applications/$_appid.desktop"
  # The icon is pixels, committed at the nine hicolor sizes under
  # packaging/icons/ rather than rendered here, so the build needs no image
  # tool and two builds lay down the same icon.
  local size
  for size in 16 22 24 32 48 64 128 256 512; do
    install -Dm644 "$startdir/packaging/icons/${size}x${size}/$_appid.png" \
      "$share/icons/hicolor/${size}x${size}/apps/$_appid.png"
  done

  install -Dm644 "$startdir/LICENSE" "$share/licenses/$pkgname/LICENSE"
  install -Dm644 "$startdir/packaging/harper-brill-LICENSE" "$share/licenses/$pkgname/harper-brill-LICENSE"
  install -m644 "$startdir"/fonts/OFL*.txt "$share/licenses/$pkgname/"
  # The Style check lists' four, by the Licence rule (docs/architecture.md,
  # "Packaging"): every licence a shipped file carries is readable here.
  install -m644 "$startdir"/data/style/LICENSE-*.txt "$share/licenses/$pkgname/"
  install -Dm644 "$startdir/README.md" "$share/doc/$pkgname/README.md"
}
