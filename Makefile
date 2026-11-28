# Build and install Pochette with its desktop file, icons and AppStream metainfo.
#
#   make
#   sudo make install                # into /usr/local
#   make install PREFIX=/usr DESTDIR=pkg   # for packages
#
# And the tarballs a release offers, in dist/:
#
#   make dist          # built, for this machine's architecture: unpack, sudo make install
#   make dist-source   # the sources and every crate they need, to build offline

APP_ID := io.github.mcrampon.Pochette
# Sizes with an icon drawn for them; the scalable one serves the others.
ICON_SIZES := 16x16 22x22 24x24
PREFIX ?= /usr/local
DESTDIR ?=
CARGO ?= cargo

BINDIR := $(DESTDIR)$(PREFIX)/bin
DATADIR := $(DESTDIR)$(PREFIX)/share

# So the desktop knows at once that Pochette opens qobuzapp:// links. Packages (DESTDIR set)
# leave it to the system's own triggers.
UPDATE_DESKTOP_DATABASE = @if [ -z "$(DESTDIR)" ] && command -v update-desktop-database >/dev/null; then \
	update-desktop-database -q "$(DATADIR)/applications"; fi

VERSION := $(shell sed -n 's/^version = "\(.*\)"$$/\1/p' Cargo.toml 2>/dev/null | head -n 1)
ARCH := $(shell uname -m)
DIST := dist

.PHONY: all install uninstall dist dist-source

ifneq ($(wildcard Cargo.toml),)
all:
	$(CARGO) build --release --locked
else
# Unpacked from a binary tarball: built already.
all:
	@echo 'Pochette is built already: run "sudo make install".'
endif

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
	$(UPDATE_DESKTOP_DATABASE)

uninstall:
	rm -f "$(BINDIR)/pochette" \
		"$(DATADIR)/applications/$(APP_ID).desktop" \
		"$(DATADIR)/metainfo/$(APP_ID).metainfo.xml" \
		"$(DATADIR)/icons/hicolor/scalable/apps/$(APP_ID).svg" \
		"$(DATADIR)/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg"
	for size in $(ICON_SIZES); do \
		rm -f "$(DATADIR)/icons/hicolor/$$size/apps/$(APP_ID).svg"; \
	done
	$(UPDATE_DESKTOP_DATABASE)

# What `make install` installs, built, with this Makefile to install it.
dist: all
	rm -rf "$(DIST)/pochette-$(VERSION)"
	mkdir -p "$(DIST)/pochette-$(VERSION)/target/release" "$(DIST)/pochette-$(VERSION)/data"
	install -m755 target/release/pochette "$(DIST)/pochette-$(VERSION)/target/release/"
	cp "$(firstword $(MAKEFILE_LIST))" "$(DIST)/pochette-$(VERSION)/Makefile"
	cp LICENSE README.md "$(DIST)/pochette-$(VERSION)/"
	cp -r data/$(APP_ID).desktop data/$(APP_ID).metainfo.xml data/icons \
		"$(DIST)/pochette-$(VERSION)/data/"
	tar -C "$(DIST)" -czf "$(DIST)/pochette-$(VERSION)-$(ARCH)-linux.tar.gz" "pochette-$(VERSION)"
	rm -rf "$(DIST)/pochette-$(VERSION)"

# The sources of the commit checked out, without the Android app, with the crates they need
# and the Cargo configuration that builds with them, offline.
dist-source:
	rm -rf "$(DIST)/pochette-$(VERSION)"
	mkdir -p "$(DIST)"
	git archive --prefix="pochette-$(VERSION)/" HEAD -- . ':!android' ':!fastlane' \
		| tar -x -C "$(DIST)"
	cd "$(DIST)/pochette-$(VERSION)" && mkdir -p .cargo \
		&& $(CARGO) vendor --locked vendor > .cargo/config.toml 2>/dev/null
	tar -C "$(DIST)" -czf "$(DIST)/pochette-$(VERSION)-vendored.tar.gz" "pochette-$(VERSION)"
	rm -rf "$(DIST)/pochette-$(VERSION)"
