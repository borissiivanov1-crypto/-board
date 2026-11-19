#!/bin/sh
# -board for GNOME: remove it.
#   ./uninstall.sh                 (from ~/.local)
#   sudo ./uninstall.sh --system   (from /usr/local)
#   ./uninstall.sh --prefix DIR
# Your servers, buttons and settings (~/.local/share/board) and saved logins (the GNOME keyring) are kept;
# add --purge to delete your data and logo cache too.
set -e
APP_ID=io.github.board.Board
PREFIX="$HOME/.local"
PURGE=""
while [ -n "$1" ]; do
  case "$1" in
    --system) PREFIX=/usr/local ;;
    --prefix) PREFIX="$2"; shift ;;
    --purge) PURGE=1 ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
  shift
done
rm -f "$PREFIX/bin/board" \
  "$PREFIX/share/applications/$APP_ID.desktop" \
  "$PREFIX/share/metainfo/$APP_ID.metainfo.xml" \
  "$PREFIX/share/icons/hicolor/scalable/apps/$APP_ID.svg" \
  "$PREFIX/share/board/defaults.json"
rmdir "$PREFIX/share/board" 2>/dev/null || true
gtk-update-icon-cache -q -t "$PREFIX/share/icons/hicolor" 2>/dev/null || true
update-desktop-database -q "$PREFIX/share/applications" 2>/dev/null || true
if [ -n "$PURGE" ]; then
  rm -rf "${XDG_DATA_HOME:-$HOME/.local/share}/board" "${XDG_CACHE_HOME:-$HOME/.cache}/board"
  echo "Removed -board and your -board data (saved logins stay in the GNOME keyring under 'board')."
else
  echo "Removed -board. Your settings are kept in ${XDG_DATA_HOME:-$HOME/.local/share}/board."
fi
