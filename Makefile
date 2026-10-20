# workspace-indicator — Rust build, test, package & install

PREFIX    ?= $(HOME)/.local
BINDIR     = $(PREFIX)/bin
XDG_CONFIG ?= $(HOME)/.config
SYSTEMD_USER_DIR = $(XDG_CONFIG)/systemd/user
HYPR_DIR         = $(XDG_CONFIG)/hypr
HYPR_CONF_DIR    = $(HYPR_DIR)/config
OMARCHY_HOOK_DIR = $(XDG_CONFIG)/omarchy/hooks/theme-set.d

TARGET       = workspace-indicator
RUST_TARGET ?= x86_64-unknown-linux-musl
RELEASE_BIN  = target/$(RUST_TARGET)/release/$(TARGET)
ARCH_SUFFIX ?= x86_64
ARCHIVE      = workspace-indicator-linux-$(ARCH_SUFFIX).tar.gz

.PHONY: all build test clean dist install uninstall

all: $(TARGET)

$(TARGET): build
	cp $(RELEASE_BIN) $(TARGET)

build:
	cargo build --release --target $(RUST_TARGET)

test:
	cargo test

install: $(TARGET)
	PREFIX="$(PREFIX)" BINDIR="$(BINDIR)" XDG_CONFIG_HOME="$(XDG_CONFIG)" ./install.sh

dist: $(TARGET)
	chmod +x install.sh scripts/workspace-indicator-focusd omarchy/theme-set-hook
	tar -czf $(ARCHIVE) \
		$(TARGET) \
		install.sh \
		scripts/workspace-indicator-focusd \
		systemd/workspace-indicator.service \
		hyprland/workspace-indicator.conf \
		hyprland/workspace-indicator.lua \
		omarchy/theme-set-hook

uninstall:
	rm -f $(BINDIR)/$(TARGET)
	rm -f $(BINDIR)/workspace-indicator-focusd
	rm -f $(SYSTEMD_USER_DIR)/workspace-indicator.service
	rm -f $(HYPR_CONF_DIR)/workspace-indicator.local.conf
	rm -f $(HYPR_DIR)/workspace-indicator.lua
	rm -f $(OMARCHY_HOOK_DIR)/workspace-indicator

clean:
	cargo clean
	rm -f $(TARGET) $(ARCHIVE)
