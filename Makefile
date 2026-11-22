APP := target/release/Vitrine.app
INSTALL_DIR := /Applications

.PHONY: run build check app install uninstall clean

run:
	cargo run

build:
	cargo build --release

check:
	cargo clippy --all-targets -- -D warnings

app:
	./scripts/bundle-macos.sh

install: app
	rm -rf "$(INSTALL_DIR)/Vitrine.app"
	cp -R "$(APP)" "$(INSTALL_DIR)/"

uninstall:
	rm -rf "$(INSTALL_DIR)/Vitrine.app"

clean:
	cargo clean
