.PHONY: fmt clippy test build dojo-bundle

fmt:
	cd mash-installer && cargo fmt

clippy:
	cd mash-installer && cargo clippy -- -D warnings

test:
	cd mash-installer && cargo test

build:
	cd mash-installer && cargo build

dojo-bundle:
	rm -f dojo_bundle.zip
	zip -r dojo_bundle.zip dojo_bundle
