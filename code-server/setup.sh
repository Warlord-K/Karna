#!/bin/sh
# Karna code-server setup — runs before code-server starts via /entrypoint.d/
# Reads code_server section from config.yaml, installs extensions, writes settings.json.
# Falls back to sensible defaults if config is missing or section is absent.

set -e

CONFIG_FILE="/etc/karna/config.yaml"
SETTINGS_DIR="/home/coder/.local/share/code-server/User"
SETTINGS_FILE="${SETTINGS_DIR}/settings.json"

# ── Defaults ──────────────────────────────────────────────────

DEFAULT_THEME="Default Dark Modern"
DEFAULT_EXTENSIONS="GitHub.github-vscode-theme ms-python.python rust-lang.rust-analyzer dbaeumer.vscode-eslint esbenp.prettier-vscode bradlc.vscode-tailwindcss anthropic.claude-code"

# ── Read config ───────────────────────────────────────────────

HAS_CONFIG=false
if [ -f "$CONFIG_FILE" ] && command -v yq >/dev/null 2>&1; then
    SECTION=$(yq '.code_server' "$CONFIG_FILE" 2>/dev/null || echo "null")
    if [ "$SECTION" != "null" ] && [ -n "$SECTION" ]; then
        HAS_CONFIG=true
    fi
fi

if [ "$HAS_CONFIG" = true ]; then
    THEME=$(yq -r '.code_server.theme // "Default Dark Modern"' "$CONFIG_FILE")

    EXTENSIONS=$(yq -r '.code_server.extensions[]' "$CONFIG_FILE" 2>/dev/null || true)
    if [ -z "$EXTENSIONS" ]; then
        EXTENSIONS="$DEFAULT_EXTENSIONS"
    fi

    CUSTOM_SETTINGS=$(yq -o=json '.code_server.settings // {}' "$CONFIG_FILE" 2>/dev/null)
else
    THEME="$DEFAULT_THEME"
    EXTENSIONS="$DEFAULT_EXTENSIONS"
    CUSTOM_SETTINGS="{}"
fi

# ── Install extensions (skip already-installed) ──────────────

INSTALLED=$(code-server --list-extensions 2>/dev/null | tr '[:upper:]' '[:lower:]' || true)
EXT_COUNT=0
SKIP_COUNT=0
for ext in $EXTENSIONS; do
    ext_lower=$(echo "$ext" | tr '[:upper:]' '[:lower:]')
    if echo "$INSTALLED" | grep -qx "$ext_lower"; then
        SKIP_COUNT=$((SKIP_COUNT + 1))
    else
        echo "[karna] Installing extension: $ext"
        code-server --install-extension "$ext" 2>/dev/null || echo "[karna] Warning: failed to install $ext"
        EXT_COUNT=$((EXT_COUNT + 1))
    fi
done

# ── Generate settings.json ────────────────────────────────────

mkdir -p "$SETTINGS_DIR"

# Base settings — good defaults for any dev environment
BASE_SETTINGS=$(jq -n \
    --arg theme "$THEME" \
    '{
        "workbench.colorTheme": $theme,
        "security.workspace.trust.enabled": false,
        "editor.fontSize": 14,
        "editor.tabSize": 2,
        "editor.formatOnSave": true,
        "editor.minimap.enabled": false,
        "editor.bracketPairColorization.enabled": true,
        "editor.guides.bracketPairs": "active",
        "terminal.integrated.defaultProfile.linux": "bash",
        "files.autoSave": "afterDelay",
        "files.autoSaveDelay": 1000,
        "git.autofetch": true,
        "git.confirmSync": false,
        "telemetry.telemetryLevel": "off"
    }')

# Custom settings from config.yaml override base settings
echo "$BASE_SETTINGS" "$CUSTOM_SETTINGS" | jq -s '.[0] * .[1]' > "$SETTINGS_FILE"

echo "[karna] Code-server configured: theme=\"$THEME\", $EXT_COUNT installed, $SKIP_COUNT cached"
