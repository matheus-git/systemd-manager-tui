# Mantainer: matheus-git <mathiew0@gmail.com>
pkgname=systemd-manager-tui
pkgver=1.1.0
pkgrel=1
pkgdesc="systemd manager tui"
arch=('x86_64')
url="https://github.com/matheus-git/systemd-manager-tui"
license=('MIT')
makedepends=('git' 'rust' 'cargo')
source=("git+https://github.com/matheus-git/systemd-manager-tui.git")
sha256sums=('SKIP')

build() {
  cd "$srcdir/systemd-manager-tui"
  cargo build --release --locked
}

package() {
  cd "$srcdir/systemd-manager-tui"
  install -Dm755 "target/release/systemd-manager-tui" "$pkgdir/usr/bin/systemd-manager-tui"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
