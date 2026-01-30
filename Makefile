.PHONY: all build-cli build-qt install clean test help

# Configuration
PREFIX ?= /usr/local
CARGO = cargo
CMAKE = cmake
INSTALL = install

all: build-cli build-qt

# Build Rust CLI
build-cli:
	@echo "🦀 Building Rust CLI..."
	cd mash-installer && $(CARGO) build --release
	@echo "✅ CLI built: mash-installer/target/release/mash-installer"

# Build Qt GUI
build-qt:
	@echo "🎨 Building Qt GUI..."
	mkdir -p qt-gui/build
	cd qt-gui/build && $(CMAKE) .. -DCMAKE_BUILD_TYPE=Release
	cd qt-gui/build && $(CMAKE) --build .
	@echo "✅ GUI built: qt-gui/build/mash-installer-qt"

# Install both
install: build-cli build-qt
	@echo "📦 Installing..."
	$(INSTALL) -D -m 755 mash-installer/target/release/mash-installer $(DESTDIR)$(PREFIX)/bin/mash-installer
	$(INSTALL) -D -m 755 qt-gui/build/mash-installer-qt $(DESTDIR)$(PREFIX)/bin/mash-installer-qt
	@if [ -n "$(DESTDIR)" ]; then \
		mkdir -p $(DESTDIR)$(PREFIX)/share/applications; \
		$(INSTALL) -D -m 644 qt-gui/mash-installer.desktop $(DESTDIR)$(PREFIX)/share/applications/; \
	fi
	@echo "✅ Installed to $(PREFIX)/bin"

# Install only CLI
install-cli: build-cli
	@echo "📦 Installing CLI only..."
	$(INSTALL) -D -m 755 mash-installer/target/release/mash-installer $(DESTDIR)$(PREFIX)/bin/mash-installer
	@echo "✅ CLI installed to $(PREFIX)/bin/mash-installer"

# Install only GUI
install-qt: build-qt
	@echo "📦 Installing Qt GUI only..."
	$(INSTALL) -D -m 755 qt-gui/build/mash-installer-qt $(DESTDIR)$(PREFIX)/bin/mash-installer-qt
	@if [ -n "$(DESTDIR)" ]; then \
		mkdir -p $(DESTDIR)$(PREFIX)/share/applications; \
		$(INSTALL) -D -m 644 qt-gui/mash-installer.desktop $(DESTDIR)$(PREFIX)/share/applications/; \
	fi
	@echo "✅ Qt GUI installed to $(PREFIX)/bin/mash-installer-qt"

# Run tests
test:
	@echo "🧪 Running tests..."
	cd mash-installer && $(CARGO) test
	@echo "✅ Tests passed"

# Clean build artifacts
clean:
	@echo "🧹 Cleaning..."
	cd mash-installer && $(CARGO) clean
	rm -rf qt-gui/build
	@echo "✅ Cleaned"

# Development builds (unoptimized, faster compilation)
dev-cli:
	@echo "🦀 Building CLI (dev mode)..."
	cd mash-installer && $(CARGO) build

dev-qt:
	@echo "🎨 Building Qt GUI (dev mode)..."
	mkdir -p qt-gui/build-dev
	cd qt-gui/build-dev && $(CMAKE) .. -DCMAKE_BUILD_TYPE=Debug
	cd qt-gui/build-dev && $(CMAKE) --build .

# Run preflight check
preflight:
	@echo "🔍 Running preflight check..."
	cd mash-installer && $(CARGO) run -- preflight

# Format code
format:
	@echo "✨ Formatting code..."
	cd mash-installer && $(CARGO) fmt
	@echo "✅ Code formatted"

# Lint code
lint:
	@echo "🔍 Linting code..."
	cd mash-installer && $(CARGO) clippy -- -D warnings
	@echo "✅ No lint warnings"

# Create release tarball
dist: build-cli build-qt
	@echo "📦 Creating distribution tarball..."
	@VERSION=$$(grep '^version = ' mash-installer/Cargo.toml | sed 's/version = "\(.*\)"/\1/'); \
	TARBALL="mash-installer-$$VERSION.tar.gz"; \
	mkdir -p dist/mash-installer-$$VERSION; \
	cp mash-installer/target/release/mash-installer dist/mash-installer-$$VERSION/; \
	cp qt-gui/build/mash-installer-qt dist/mash-installer-$$VERSION/; \
	cp README.md LICENSE dist/mash-installer-$$VERSION/; \
	cd dist && tar -czf $$TARBALL mash-installer-$$VERSION; \
	rm -rf mash-installer-$$VERSION; \
	echo "✅ Created dist/$$TARBALL"

# Bump version
bump-major:
	@VERSION=$$(sed -n 's/^version = "\\(.*\\)"/\\1/p' mash-installer/Cargo.toml); \
	MAJOR=$${VERSION%%.*}; \
	NEXT_MAJOR=$$((MAJOR+1)); \
	$(CARGO) run --package mash-tools -- release bump $$NEXT_MAJOR.0.0

bump-minor:
	@VERSION=$$(sed -n 's/^version = "\\(.*\\)"/\\1/p' mash-installer/Cargo.toml); \
	MAJOR=$${VERSION%%.*}; \
	REST=$${VERSION#*.}; \
	MINOR=$${REST%%.*}; \
	NEXT_MINOR=$$((MINOR+1)); \
	$(CARGO) run --package mash-tools -- release bump $$MAJOR.$$NEXT_MINOR.0

bump-patch:
	@VERSION=$$(sed -n 's/^version = "\\(.*\\)"/\\1/p' mash-installer/Cargo.toml); \
	MAJOR=$${VERSION%%.*}; \
	REST=$${VERSION#*.}; \
	MINOR=$${REST%%.*}; \
	PATCH=$${REST#*.}; \
	NEXT_PATCH=$$((PATCH+1)); \
	$(CARGO) run --package mash-tools -- release bump $$MAJOR.$$MINOR.$$NEXT_PATCH

# Rust release tool
mash-release:
	@$(CARGO) run --package mash-tools --

# Help
help:
	@echo "MASH Installer - Build System"
	@echo ""
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@echo "  all            Build both CLI and Qt GUI (default)"
	@echo "  build-cli      Build Rust CLI only"
	@echo "  build-qt       Build Qt GUI only"
	@echo "  install        Install both to $(PREFIX)/bin"
	@echo "  install-cli    Install CLI only"
	@echo "  install-qt     Install Qt GUI only"
	@echo "  test           Run tests"
	@echo "  clean          Remove build artifacts"
	@echo "  dev-cli        Build CLI (debug mode)"
	@echo "  dev-qt         Build Qt GUI (debug mode)"
	@echo "  preflight      Run preflight check"
	@echo "  format         Format code"
	@echo "  lint           Lint code"
	@echo "  dist           Create distribution tarball"
	@echo "  bump-major     Bump major version (X.0.0)"
	@echo "  bump-minor     Bump minor version (0.X.0)"
	@echo "  bump-patch     Bump patch version (0.0.X)"
	@echo "  help           Show this help"
	@echo ""
	@echo "Variables:"
	@echo "  PREFIX         Installation prefix (default: /usr/local)"
	@echo "  DESTDIR        Installation root (for packaging)"
	@echo ""
	@echo "Examples:"
	@echo "  make                    # Build everything"
	@echo "  make install            # Build and install"
	@echo "  make PREFIX=/usr install  # Install to /usr/bin"
	@echo "  make test               # Run tests"
	@echo "  make bump-patch         # Bump version and prepare release"
