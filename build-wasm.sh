#!/bin/bash

# Build script for Aniimax WASM

set -e

echo "Building Aniimax WASM..."

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "wasm-pack not found. Installing..."
    cargo install wasm-pack
fi

# Build WASM
wasm-pack build --target web --out-dir web/pkg

echo "Build complete!"
echo ""
echo "To test locally:"
echo "  cd web && python3 -m http.server 8080"
echo "  Then open http://localhost:8080 in your browser"
echo ""
echo "To deploy to GitHub Pages:"
echo "  Copy the contents of the 'web' directory to your gh-pages branch"
