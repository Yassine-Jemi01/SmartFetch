PREFIX ?= /usr/local
BINARY = target/release/sfetch

all: release

release:
	cargo build --release

debug:
	cargo build

install: release
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 755 $(BINARY) $(DESTDIR)$(PREFIX)/bin/sfetch

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/sfetch

test:
	cargo test --all-targets

clean:
	cargo clean

.PHONY: all release debug install uninstall test clean
