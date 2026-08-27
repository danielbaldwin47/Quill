# Maintainer: Daniel Baldwin <danielbaldwin47@gmail.com>
# Build from this checkout:  makepkg -f   (then: sudo pacman -U quill-*.pkg.tar.zst)
pkgname=quill
pkgver=1.0.0.r39.g836e374
pkgrel=1
pkgdesc="A long-form writing environment: textarea+mirror editor, iA Writer Duo/Quattro/Mono, focus and typewriter modes"
arch=('any')
url="https://github.com/danielbaldwin47/Quill"
license=('ISC' 'OFL-1.1')
depends=('bash' 'nodejs' 'chromium' 'curl')
optdepends=('hyprland: place the measured window on a virtual output (bin/quill --measure)'
            'python: helpers used by --measure')
# Built straight from the working tree; no download.
source=()

pkgver() {
  cd "$startdir"
  printf '1.0.0.r%s.g%s' "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
}

package() {
  local share="$pkgdir/usr/share/$pkgname"
  install -dm755 "$share/tools" "$share/bin" "$pkgdir/usr/bin"
  cp -r "$startdir/app" "$share/app"
  install -m644 "$startdir/tools/serve.mjs" "$share/tools/serve.mjs"
  install -m755 "$startdir/bin/quill" "$share/bin/quill"
  ln -s "/usr/share/$pkgname/bin/quill" "$pkgdir/usr/bin/quill"
  install -Dm644 "$startdir/packaging/quill.desktop" "$pkgdir/usr/share/applications/quill.desktop"
  install -Dm644 "$startdir/README.md" "$pkgdir/usr/share/doc/$pkgname/README.md"
  install -Dm644 "$startdir/app/fonts/Duo/LICENSE.md" "$pkgdir/usr/share/licenses/$pkgname/LICENSE-iA-Fonts.md"
}
