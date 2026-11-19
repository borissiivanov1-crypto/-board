#!/bin/sh
# -board for GNOME: install from this folder.
#   ./install.sh            for you only (~/.local), no password needed
#   sudo ./install.sh --system   for everyone (/usr/local)
#   ./install.sh --prefix DIR    somewhere else
# Needs GTK 4.14+ and libadwaita 1.5+ (Ubuntu 24.04, Debian 13, Fedora 40 and newer).
set -e
APP_ID=io.github.board.Board
HERE=$(cd "$(dirname "$0")" && pwd)
PREFIX="$HOME/.local"
case "$1" in
  --system) PREFIX=/usr/local ;;
  --prefix) PREFIX="$2" ;;
  -h|--help) sed -n '2,6p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
  "") ;;
  *) echo "Unknown option: $1 (try --help)"; exit 1 ;;
esac

# the libraries -board needs
missing=""
for lib in libgtk-4.so.1 libadwaita-1.so.0 libdbus-1.so.3; do
  if ! /sbin/ldconfig -p 2>/dev/null | grep -q "$lib" && ! ldconfig -p 2>/dev/null | grep -q "$lib"; then missing="$missing $lib"; fi
done
if [ -n "$missing" ]; then
  echo "Missing libraries:$missing"
  echo "  Ubuntu / Debian: sudo apt install libgtk-4-1 libadwaita-1-0 libdbus-1-3"
  echo "  Fedora:          sudo dnf install gtk4 libadwaita dbus-libs"
  echo "  Arch:            sudo pacman -S gtk4 libadwaita dbus"
  exit 1
fi

echo "Installing -board to $PREFIX ..."
install -Dm755 "$HERE/bin/board" "$PREFIX/bin/board"
install -Dm644 "$HERE/share/applications/$APP_ID.desktop" "$PREFIX/share/applications/$APP_ID.desktop"
install -Dm644 "$HERE/share/metainfo/$APP_ID.metainfo.xml" "$PREFIX/share/metainfo/$APP_ID.metainfo.xml"
install -Dm644 "$HERE/share/icons/hicolor/scalable/apps/$APP_ID.svg" "$PREFIX/share/icons/hicolor/scalable/apps/$APP_ID.svg"
install -Dm644 "$HERE/share/board/defaults.json" "$PREFIX/share/board/defaults.json"
# the app menu entry starts the installed program by its full path (~/.local/bin isn't always on PATH)
sed -i "s|^Exec=board|Exec=$PREFIX/bin/board|" "$PREFIX/share/applications/$APP_ID.desktop"
gtk-update-icon-cache -q -t "$PREFIX/share/icons/hicolor" 2>/dev/null || true
update-desktop-database -q "$PREFIX/share/applications" 2>/dev/null || true

echo "Done. Open -board from the app grid, or run: $PREFIX/bin/board"
echo "Desktop widgets: right-click the app icon, or run: $PREFIX/bin/board --widget large"
case ":$PATH:" in *":$PREFIX/bin:"*) ;; *) echo "(Add $PREFIX/bin to your PATH to run 'board' by name.)" ;; esac
