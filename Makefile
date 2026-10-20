# Public build/package/install interface. Run `make help` for supported targets.
SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c
.DEFAULT_GOAL := build

ARCH ?= $(shell uname -m)
RUST_TARGET ?= $(ARCH)-unknown-linux-musl
ARCH_SUFFIX ?= $(firstword $(subst -, ,$(RUST_TARGET)))
DEB_ARCH = $(if $(filter x86_64,$(ARCH_SUFFIX)),amd64,$(if $(filter aarch64,$(ARCH_SUFFIX)),arm64,unsupported))
VERSION := $(shell sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
TARGET := workspace-indicator
RELEASE_BIN = target/$(RUST_TARGET)/release/$(TARGET)
PREBUILT ?= $(if $(wildcard src/main.rs),0,1)
ARCHIVE ?= workspace-indicator-linux-$(ARCH_SUFFIX).tar.gz
DEB_PACKAGE = $(abspath dist/workspace-indicator_$(VERSION)-1_$(DEB_ARCH).deb)
PACKAGE ?=
RELEASE_DIR ?= .

PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
XDG_CONFIG ?= $(if $(XDG_CONFIG_HOME),$(XDG_CONFIG_HOME),$(HOME)/.config)
SYSTEMD_USER_DIR ?= $(XDG_CONFIG)/systemd/user
HYPR_DIR ?= $(XDG_CONFIG)/hypr
HYPR_CONF_DIR ?= $(HYPR_DIR)/config
OMARCHY_HOOKS_ROOT ?= $(XDG_CONFIG)/omarchy/hooks
PLUGIN_DIR := $(XDG_CONFIG)/omarchy/plugins/zanzythebar.workspace-indicator
# Set to 0 when staging files without touching the current graphical session.
MANAGE_SERVICE ?= 1
SUDO ?= sudo
export PREFIX BINDIR SYSTEMD_USER_DIR HYPR_DIR HYPR_CONF_DIR OMARCHY_HOOKS_ROOT MANAGE_SERVICE
export XDG_CONFIG_HOME := $(XDG_CONFIG)

.PHONY: all help build inspect install uninstall dist clean test test-release test-packaging test-install \
 build-tar inspect-tar install-tar uninstall-tar build-deb inspect-deb install-deb uninstall-deb \
 build-aur inspect-aur install-aur uninstall-aur build-omarchy inspect-omarchy install-omarchy uninstall-omarchy \
 start stop toolchain release prepare-release update-aur

all: build
help:
	@echo 'make build|inspect|install|uninstall  - local source binary and user integration'
	@echo 'make build-tar|inspect-tar|install-tar|uninstall-tar'
	@echo 'make build-deb|inspect-deb|install-deb|uninstall-deb'
	@echo 'make build-aur|inspect-aur|install-aur|uninstall-aur'
	@echo 'make build-omarchy|inspect-omarchy|install-omarchy|uninstall-omarchy'
	@echo 'ARCH=x86_64|aarch64; PACKAGE=/path/to/package.deb; ARCHIVE=/path/to/release.tar.gz'
	@echo 'make start|stop; make test|test-release|test-packaging|test-install'
	@echo 'make toolchain installs the selected musl target into an existing rustup toolchain.'
	@echo 'make release runs semantic-release (maintainers/CI only).'
	@echo 'MANAGE_SERVICE=0 stages local files without enabling/stopping services.'

build:
	cargo build --release --locked --target "$(RUST_TARGET)"
	cp "$(RELEASE_BIN)" "$(TARGET)"
toolchain:
	rustup target add "$(RUST_TARGET)"
inspect:
	file "$(if $(filter 1,$(PREBUILT)),$(TARGET),$(RELEASE_BIN))"
install: $(if $(filter 1,$(PREBUILT)),,build)
	@test "$(ARCH_SUFFIX)" = "$$(uname -m)" || { echo 'Cross-compiled binaries cannot be installed on this host.'; exit 1; }
	bash ./install.sh
uninstall:
	bash ./install.sh --uninstall

dist: build-tar
build-tar: build
	tar -czf "$(ARCHIVE)" LICENSE Cargo.toml Makefile install.sh scripts/manage-service.sh scripts/manage-tar.sh \
		$(TARGET) scripts/workspace-indicator-focusd systemd/workspace-indicator.service \
		hyprland/workspace-indicator.conf hyprland/workspace-indicator.lua omarchy/theme-set-hook
inspect-tar:
	tar -tzf "$(ARCHIVE)"
install-tar:
	bash scripts/manage-tar.sh "$(abspath $(ARCHIVE))"
uninstall-tar: uninstall

build-deb: build
	node scripts/build-deb.mjs "$(RELEASE_BIN)" "$(DEB_ARCH)"
inspect-deb:
	ar t "$(if $(PACKAGE),$(PACKAGE),$(DEB_PACKAGE))"
	ar p "$(if $(PACKAGE),$(PACKAGE),$(DEB_PACKAGE))" control.tar.xz | tar -xJO ./control
	ar p "$(if $(PACKAGE),$(PACKAGE),$(DEB_PACKAGE))" data.tar.xz | tar -tvJ
install-deb: $(if $(PACKAGE),,build-deb)
	@test "$(DEB_ARCH)" != unsupported
	$(SUDO) apt install "$(abspath $(if $(PACKAGE),$(PACKAGE),$(DEB_PACKAGE)))"
	bash scripts/manage-service.sh start
uninstall-deb:
	bash scripts/manage-service.sh stop
	$(SUDO) apt remove workspace-indicator

# The AUR recipe pins published binaries; it deliberately does not build Rust.
build-aur:
	cd packaging/aur && CARCH="$(ARCH_SUFFIX)" makepkg --force
inspect-aur:
	cd packaging/aur && CARCH="$(ARCH_SUFFIX)" makepkg --verifysource --force
	cd packaging/aur && CARCH="$(ARCH_SUFFIX)" makepkg --printsrcinfo
	@cd packaging/aur && CARCH="$(ARCH_SUFFIX)" makepkg --packagelist | while IFS= read -r pkg; do tar -tvf "$$pkg"; done
install-aur: build-aur
	@cd packaging/aur && mapfile -t packages < <(CARCH="$(ARCH_SUFFIX)" makepkg --packagelist) && $(SUDO) pacman -U --needed "$${packages[@]}"
	bash scripts/manage-service.sh start
uninstall-aur:
	bash scripts/manage-service.sh stop
	$(SUDO) pacman -R workspace-indicator-bin

build-omarchy:
	mkdir -p dist/omarchy/zanzythebar.workspace-indicator
	cp manifest.json Service.qml LICENSE README.md dist/omarchy/zanzythebar.workspace-indicator/
inspect-omarchy: build-omarchy
	omarchy plugin validate dist/omarchy/zanzythebar.workspace-indicator
install-omarchy: inspect-omarchy
	@command -v workspace-indicator >/dev/null || { echo 'Install the binary first (make install, install-deb, install-aur, or install-tar).'; exit 1; }
	@test ! -e "$(PLUGIN_DIR)" || { echo 'Plugin directory exists; uninstall it before replacing it.'; exit 1; }
	bash scripts/manage-service.sh stop
	mkdir -p "$(PLUGIN_DIR)"
	cp dist/omarchy/zanzythebar.workspace-indicator/* "$(PLUGIN_DIR)/"
	omarchy-shell shell rescanPlugins
	omarchy plugin enable zanzythebar.workspace-indicator
uninstall-omarchy:
	omarchy plugin remove zanzythebar.workspace-indicator --yes

start:
	bash scripts/manage-service.sh start
stop:
	bash scripts/manage-service.sh stop
test:
	cargo test --locked
test-release:
	node scripts/test-release.mjs
test-packaging:
	node scripts/test-packaging.mjs
test-install:
	node scripts/test-install.mjs
release:
	npm run release
prepare-release:
	node scripts/prepare-release.mjs "$(RELEASE_VERSION)"
update-aur:
	node scripts/update-aur.mjs "$(VERSION)" "$(RELEASE_DIR)"
clean:
	cargo clean
	rm -f "$(TARGET)" workspace-indicator-linux-x86_64.tar.gz workspace-indicator-linux-aarch64.tar.gz
