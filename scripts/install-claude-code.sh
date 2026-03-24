#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

COMMANDS_DIR="$HOME/.claude/commands"
SETTINGS_FILE="$HOME/.claude/settings.json"

echo "Installing cody Claude Code integration..."

# ── 1. Install slash command ─────────────────────────────────────────────────
mkdir -p "$COMMANDS_DIR"
cp "$REPO_ROOT/examples/claude-command/cody.md" "$COMMANDS_DIR/cody.md"
echo "  ✓ Slash command installed: $COMMANDS_DIR/cody.md"

# ── 2. Add permissions to settings.json ─────────────────────────────────────
if [ ! -f "$SETTINGS_FILE" ]; then
    mkdir -p "$(dirname "$SETTINGS_FILE")"
    echo '{"permissions":{"allow":[]}}' > "$SETTINGS_FILE"
fi

# Detect jq availability
if command -v jq &>/dev/null; then
    # Use jq to safely merge permissions
    PERMS_TO_ADD='["Bash(cody:*)","Bash(./target/release/cody:*)"]'
    UPDATED=$(jq \
        --argjson new "$PERMS_TO_ADD" \
        '.permissions.allow = ((.permissions.allow // []) + $new | unique)' \
        "$SETTINGS_FILE")
    echo "$UPDATED" > "$SETTINGS_FILE"
    echo "  ✓ Permissions added to $SETTINGS_FILE"
else
    echo "  ! jq not found — add these permissions to $SETTINGS_FILE manually:"
    echo '    "Bash(cody:*)"'
    echo '    "Bash(./target/release/cody:*)"'
fi

# ── 3. Check binary availability ─────────────────────────────────────────────
if command -v cody &>/dev/null; then
    echo "  ✓ cody found in PATH: $(which cody)"
elif [ -f "$REPO_ROOT/target/release/cody" ]; then
    echo "  ✓ cody found at $REPO_ROOT/target/release/cody"
    echo "    Tip: add to PATH with: export PATH=\"\$PATH:$REPO_ROOT/target/release\""
else
    echo "  ! cody binary not found. Build it with: cargo build --release"
fi

echo ""
echo "Done. Generate a codemap for your project:"
echo "  cody ./my-project --out codemap.md"
echo ""
echo "Then start a Claude Code session and try:"
echo "  /cody how does authentication work in this project?"
