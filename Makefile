PREFIX  ?= /usr/local
BINDIR   = $(DESTDIR)$(PREFIX)/bin
APPDIR   = $(DESTDIR)$(PREFIX)/share/applications
ICONDIR  = $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps

.PHONY: all build install install-bin uninstall clean run

all: build

build:
	cargo build --release

# Just the file-copy step, with no build dependency — used by `install` and
# by update.sh (via sudo/pkexec) to reinstall an already-built release.
install-bin:
	install -Dm755 target/release/velo-player                  "$(BINDIR)/velo-player"
	install -Dm755 uninstall.sh                                 "$(BINDIR)/velo-player-uninstall"
	install -Dm644 assets/velo-player.desktop                  "$(APPDIR)/velo-player.desktop"
	install -Dm644 assets/velo-player.svg                      "$(ICONDIR)/velo-player.svg"
	@update-desktop-database "$(APPDIR)" 2>/dev/null || true
	@gtk-update-icon-cache -f -t "$(DESTDIR)$(PREFIX)/share/icons/hicolor" 2>/dev/null || true
	@echo ""
	@echo "  Velo Player installed to $(PREFIX)"
	@echo "  Run: velo-player"

install: build install-bin

uninstall:
	@PREFIX=$(PREFIX) bash uninstall.sh

clean:
	cargo clean

run:
	cargo run
