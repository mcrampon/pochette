# Build and install Pochette with its desktop file, icons and AppStream metainfo.
#
#   make
#   sudo make install                # into /usr/local
#   make install PREFIX=/usr DESTDIR=pkg   # for packages

APP_ID := io.github.mcrampon.Pochette
# Sizes with an icon drawn for them; the scalable one serves the others.
ICON_SIZES := 16x16 22x22 24x24
PREFIX ?= /usr/local
DESTDIR ?=
CARGO ?= cargo

BINDIR := $(DESTDIR)$(PREFIX)/bin
DATADIR := $(DESTDIR)$(PREFIX)/share

.PHONY: all install uninstall

all:
	$(CARGO) build --release --locked

install:
	install -Dm755 target/release/pochette "$(BINDIR)/pochette"
	install -Dm644 data/$(APP_ID).desktop "$(DATADIR)/applications/$(APP_ID).desktop"
	install -Dm644 data/$(APP_ID).metainfo.xml "$(DATADIR)/metainfo/$(APP_ID).metainfo.xml"
	install -Dm644 data/icons/hicolor/scalable/apps/$(APP_ID).svg \
		"$(DATADIR)/icons/hicolor/scalable/apps/$(APP_ID).svg"
	install -Dm644 data/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg \
		"$(DATADIR)/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg"
	for size in $(ICON_SIZES); do \
		install -Dm644 data/icons/hicolor/$$size/apps/$(APP_ID).svg \
			"$(DATADIR)/icons/hicolor/$$size/apps/$(APP_ID).svg" || exit 1; \
	done

uninstall:
	rm -f "$(BINDIR)/pochette" \
		"$(DATADIR)/applications/$(APP_ID).desktop" \
		"$(DATADIR)/metainfo/$(APP_ID).metainfo.xml" \
		"$(DATADIR)/icons/hicolor/scalable/apps/$(APP_ID).svg" \
		"$(DATADIR)/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg"
	for size in $(ICON_SIZES); do \
		rm -f "$(DATADIR)/icons/hicolor/$$size/apps/$(APP_ID).svg"; \
	done
