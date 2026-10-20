# workspace-indicator — build, package & install
#
# Usage:
#   make                    Build the binary
#   make install            Install binary, focusd, systemd unit, and Hyprland drop-in
#   make dist               Package pre-built release tarball
#   make clean              Remove build artifacts
#   make uninstall          Remove installed files

CC        ?= gcc
CFLAGS    ?= -Wall -Wextra -O2
PKG_DEPS   = gtk+-3.0 gtk-layer-shell-0

CFLAGS    += $(shell pkg-config --cflags $(PKG_DEPS))
LDFLAGS   += $(shell pkg-config --libs   $(PKG_DEPS)) -pthread

PREFIX    ?= $(HOME)/.local
BINDIR     = $(PREFIX)/bin
XDG_CONFIG ?= $(HOME)/.config
SYSTEMD_USER_DIR = $(XDG_CONFIG)/systemd/user
HYPR_CONF_DIR    = $(XDG_CONFIG)/hypr/config

TARGET     = workspace-indicator
ARCHIVE    = workspace-indicator-linux-x86_64.tar.gz

.PHONY: all clean dist install uninstall

all: $(TARGET)

$(TARGET): main.c
	$(CC) $(CFLAGS) -o $@ $< $(LDFLAGS)

install: $(TARGET)
	PREFIX="$(PREFIX)" BINDIR="$(BINDIR)" XDG_CONFIG_HOME="$(XDG_CONFIG)" ./install.sh

dist: $(TARGET)
	chmod +x install.sh scripts/workspace-indicator-focusd
	tar -czf $(ARCHIVE) \
		$(TARGET) \
		install.sh \
		scripts/workspace-indicator-focusd \
		systemd/workspace-indicator.service \
		hyprland/workspace-indicator.conf

uninstall:
	rm -f $(BINDIR)/$(TARGET)
	rm -f $(BINDIR)/workspace-indicator-focusd
	rm -f $(SYSTEMD_USER_DIR)/workspace-indicator.service
	rm -f $(HYPR_CONF_DIR)/workspace-indicator.local.conf

clean:
	rm -f $(TARGET) $(ARCHIVE)
