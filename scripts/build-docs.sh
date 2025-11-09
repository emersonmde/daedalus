#!/usr/bin/env bash
# Build unified documentation (mdBook + cargo doc)
set -eu

echo "Building mdBook documentation..."
mdbook build

echo "Building cargo API documentation..."
RUSTDOCFLAGS="--enable-index-page -Zunstable-options" \
    cargo doc --no-deps --document-private-items

echo "Copying cargo docs into mdbook output..."
mkdir -p docs/book/rustdoc
cp -r target/aarch64-daedalus/doc/* docs/book/rustdoc/

echo "✓ Documentation built successfully!"
echo "  View at: docs/book/index.html"
echo "  Or run: mdbook serve"
