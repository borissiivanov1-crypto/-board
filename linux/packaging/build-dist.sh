#!/bin/sh
# Builds the standalone packages into linux/dist/:
#   board-<version>-linux-x86_64.tar.gz  (any distro: unpack, run ./install.sh)
#   board_<version>_amd64.deb             (Ubuntu / Debian: sudo apt install ./board_..._amd64.deb)
# Run from anywhere: sh linux/packaging/build-dist.sh
set -e
cd "$(dirname "$0")/.."
APP_ID=io.github.board.Board
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
ARCH=$(uname -m)
DEBARCH=$(dpkg --print-architecture 2>/dev/null || echo amd64)
cargo build --release
rm -rf dist/stage && mkdir -p dist/stage

# --- the tarball: a ready-made install tree plus the scripts ---
T=dist/stage/board-$VERSION-linux-$ARCH
install -Dm755 target/release/board "$T/bin/board"
install -Dm644 data/$APP_ID.desktop "$T/share/applications/$APP_ID.desktop"
install -Dm644 data/$APP_ID.metainfo.xml "$T/share/metainfo/$APP_ID.metainfo.xml"
install -Dm644 data/icons/hicolor/scalable/apps/$APP_ID.svg "$T/share/icons/hicolor/scalable/apps/$APP_ID.svg"
install -Dm644 config/defaults.json "$T/share/board/defaults.json"
install -m755 packaging/install.sh packaging/uninstall.sh "$T/"
install -m644 packaging/INSTALL.txt "$T/INSTALL.txt"
tar -C dist/stage -czf "dist/board-$VERSION-linux-$ARCH.tar.gz" "board-$VERSION-linux-$ARCH"

# --- the .deb ---
D=dist/stage/deb
install -Dm755 target/release/board "$D/usr/bin/board"
install -Dm644 data/$APP_ID.desktop "$D/usr/share/applications/$APP_ID.desktop"
install -Dm644 data/$APP_ID.metainfo.xml "$D/usr/share/metainfo/$APP_ID.metainfo.xml"
install -Dm644 data/icons/hicolor/scalable/apps/$APP_ID.svg "$D/usr/share/icons/hicolor/scalable/apps/$APP_ID.svg"
install -Dm644 config/defaults.json "$D/usr/share/board/defaults.json"
mkdir -p "$D/DEBIAN"
SIZE=$(du -sk "$D/usr" | cut -f1)
cat > "$D/DEBIAN/control" <<CTRL
Package: board
Version: $VERSION
Section: admin
Priority: optional
Architecture: $DEBARCH
Depends: libgtk-4-1 (>= 4.14), libadwaita-1-0 (>= 1.5), libdbus-1-3, libc6 (>= 2.35)
Recommends: gnome-keyring
Installed-Size: $SIZE
Maintainer: -board <noreply@localhost>
Homepage: https://github.com/borissiivanov1-crypto/-board
Description: Live home-server dashboard and desktop widget for GNOME
 Shows whether your home server is up, CPU / RAM / storage / network from
 Glances, service buttons with up / down dots and stacks, several servers,
 Reboot / Shut down for ZimaOS, Proxmox VE, TrueNAS, Unraid, Synology DSM,
 OpenMediaVault and Linux, and the ZimaOS app controls. Classic and Liquid
 Glass looks, and small / medium / large desktop widgets.
CTRL
cat > "$D/DEBIAN/postinst" <<'POST'
#!/bin/sh
set -e
gtk-update-icon-cache -q -t /usr/share/icons/hicolor 2>/dev/null || true
update-desktop-database -q /usr/share/applications 2>/dev/null || true
POST
cp "$D/DEBIAN/postinst" "$D/DEBIAN/postrm"
chmod 755 "$D/DEBIAN/postinst" "$D/DEBIAN/postrm"
dpkg-deb --root-owner-group --build "$D" "dist/board_${VERSION}_$DEBARCH.deb" >/dev/null
rm -rf dist/stage
(cd dist && sha256sum "board-$VERSION-linux-$ARCH.tar.gz" "board_${VERSION}_$DEBARCH.deb" > SHA256SUMS)
ls -l dist
