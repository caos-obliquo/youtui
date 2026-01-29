#!/bin/bash

# Youtui Library Cleanup Script
# This script removes all Library-related code to fix compilation errors

set -e  # Exit on error

YOUTUI_ROOT="${1:-.}"  # First argument is youtui root, or current directory

echo "🧹 Cleaning up Library code from youtui..."
echo "📁 Working in: $YOUTUI_ROOT"
echo ""

# Check if we're in the right directory
if [ ! -f "$YOUTUI_ROOT/Cargo.toml" ]; then
    echo "❌ Error: Cargo.toml not found. Are you in the youtui root directory?"
    echo "Usage: $0 [path-to-youtui]"
    exit 1
fi

echo "Step 1: Deleting library.rs file..."
if [ -f "$YOUTUI_ROOT/youtui/src/app/ui/library.rs" ]; then
    rm "$YOUTUI_ROOT/youtui/src/app/ui/library.rs"
    echo "✓ Deleted library.rs"
else
    echo "⚠ library.rs not found (maybe already deleted)"
fi

echo ""
echo "Step 2: Checking for replacement files..."
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Copy fixed files if they exist in the same directory as this script
if [ -f "$SCRIPT_DIR/ui.rs" ]; then
    echo "✓ Found ui.rs, copying..."
    cp "$SCRIPT_DIR/ui.rs" "$YOUTUI_ROOT/youtui/src/app/ui.rs"
fi

if [ -f "$SCRIPT_DIR/action.rs" ]; then
    echo "✓ Found action.rs, copying..."
    cp "$SCRIPT_DIR/action.rs" "$YOUTUI_ROOT/youtui/src/app/ui/action.rs"
fi

if [ -f "$SCRIPT_DIR/structures.rs" ]; then
    echo "✓ Found structures.rs, copying..."
    cp "$SCRIPT_DIR/structures.rs" "$YOUTUI_ROOT/youtui/src/app/structures.rs"
fi

if [ -f "$SCRIPT_DIR/draw.rs" ]; then
    echo "✓ Found draw.rs, copying..."
    cp "$SCRIPT_DIR/draw.rs" "$YOUTUI_ROOT/youtui/src/app/ui/draw.rs"
fi

if [ -f "$SCRIPT_DIR/config.toml" ]; then
    echo "✓ Found config.toml, copying..."
    cp "$SCRIPT_DIR/config.toml" "$YOUTUI_ROOT/config/config.toml"
fi

echo ""
echo "Step 3: Manual edits still needed..."
echo "Please manually edit these files to remove Library references:"
echo "  - $YOUTUI_ROOT/youtui/src/app/ui/browser.rs (remove ViewLibrary action)"
echo "  - $YOUTUI_ROOT/youtui/src/app/ui/playlist.rs (remove ViewLibrary action)"
echo "  - $YOUTUI_ROOT/youtui/src/config/keymap.rs (remove library keybinds)"
echo ""
echo "See CLEANUP_AND_NEW_FEATURES_GUIDE.md for detailed instructions."
echo ""
echo "✅ Core cleanup complete! Try building with: cargo build"
