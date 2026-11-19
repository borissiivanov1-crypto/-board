# -board for GNOME
#   make            build (release)
#   make test       run the tests
#   make install    install to PREFIX (default /usr/local; use PREFIX=$HOME/.local for your user only)
#   make uninstall
PREFIX ?= /usr/local
DESTDIR ?=
APP_ID = io.github.board.Board

all:
	cargo build --release

test:
	BOARD_SECRETS=file cargo test

install: all
	install -Dm755 target/release/board $(DESTDIR)$(PREFIX)/bin/board
	install -Dm644 data/$(APP_ID).desktop $(DESTDIR)$(PREFIX)/share/applications/$(APP_ID).desktop
	install -Dm644 data/$(APP_ID).metainfo.xml $(DESTDIR)$(PREFIX)/share/metainfo/$(APP_ID).metainfo.xml
	install -Dm644 data/icons/hicolor/scalable/apps/$(APP_ID).svg $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/$(APP_ID).svg
	-gtk-update-icon-cache -q -t $(DESTDIR)$(PREFIX)/share/icons/hicolor 2>/dev/null
	-update-desktop-database -q $(DESTDIR)$(PREFIX)/share/applications 2>/dev/null

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/board
	rm -f $(DESTDIR)$(PREFIX)/share/applications/$(APP_ID).desktop
	rm -f $(DESTDIR)$(PREFIX)/share/metainfo/$(APP_ID).metainfo.xml
	rm -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/$(APP_ID).svg

.PHONY: all test install uninstall
