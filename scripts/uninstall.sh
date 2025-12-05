#!/usr/bin/env bash
set -e

BINARY_NAME="ssearch"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
SKILL_NAME="semantic-search"
USER_SKILL_DIR="$HOME/.claude/skills/$SKILL_NAME"
CONFIG_DIR="$HOME/.config/ssearch"

echo "🗑️  Uninstalling Semantic Search CLI (ssearch)..."
echo

# ============================================================================
# Binary Removal
# ============================================================================

if [ -f "$INSTALL_DIR/$BINARY_NAME" ]; then
    rm "$INSTALL_DIR/$BINARY_NAME"
    echo "✅ Removed $INSTALL_DIR/$BINARY_NAME"
else
    echo "⚠️  Binary not found at $INSTALL_DIR/$BINARY_NAME"
fi

# ============================================================================
# Skill Cleanup
# ============================================================================

cleanup_skill() {
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "🤖 Claude Code Skill Cleanup"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""

    if [ -d "$USER_SKILL_DIR" ]; then
        echo "User-level skill found at: $USER_SKILL_DIR"
        echo ""
        read -p "Remove user-level skill? [y/N]: " choice
        echo

        case "$choice" in
            y|Y)
                # Check for backups (both old and new formats)
                local backup_count=0
                local old_backups=$(ls -d "${USER_SKILL_DIR}.bak-"* 2>/dev/null | wc -l | tr -d ' ')
                local new_backups=$(ls -d "${USER_SKILL_DIR}.backup_"* 2>/dev/null | wc -l | tr -d ' ')
                backup_count=$((old_backups + new_backups))

                if [ "$backup_count" -gt 0 ]; then
                    echo "Found $backup_count backup(s):"
                    ls -d "${USER_SKILL_DIR}.bak-"* 2>/dev/null | while read backup; do
                        echo "  • $(basename "$backup")"
                    done
                    ls -d "${USER_SKILL_DIR}.backup_"* 2>/dev/null | while read backup; do
                        echo "  • $(basename "$backup")"
                    done
                    echo ""
                    read -p "Remove skill backups too? [y/N]: " backup_choice
                    echo

                    case "$backup_choice" in
                        y|Y)
                            rm -rf "${USER_SKILL_DIR}.bak-"* 2>/dev/null || true
                            rm -rf "${USER_SKILL_DIR}.backup_"* 2>/dev/null || true
                            echo "✅ Removed skill backups"
                            ;;
                        *)
                            echo "⏭️  Kept skill backups"
                            ;;
                    esac
                fi

                rm -rf "$USER_SKILL_DIR"
                echo "✅ Removed user-level skill"

                # Cleanup empty parent directories
                if [ -d "$HOME/.claude/skills" ] && [ -z "$(ls -A "$HOME/.claude/skills")" ]; then
                    rmdir "$HOME/.claude/skills"
                    echo "   Cleaned up empty skills directory"

                    if [ -d "$HOME/.claude" ] && [ -z "$(ls -A "$HOME/.claude")" ]; then
                        rmdir "$HOME/.claude"
                        echo "   Cleaned up empty .claude directory"
                    fi
                fi
                ;;
            *)
                echo "⏭️  Kept user-level skill"
                ;;
        esac
    else
        echo "⚠️  User-level skill not found at: $USER_SKILL_DIR"
    fi

    echo ""
    echo "Note: Project-level skill at ./.claude/skills/$SKILL_NAME is NOT removed."
    echo "It's part of the project repository and may be useful for development."
}

cleanup_skill

# ============================================================================
# Configuration Cleanup
# ============================================================================

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🔧 Configuration Cleanup"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

if [ -d "$CONFIG_DIR" ]; then
    echo "Configuration found at: $CONFIG_DIR"
    echo ""
    read -p "Remove configuration? [y/N]: " choice
    echo

    case "$choice" in
        y|Y)
            rm -rf "$CONFIG_DIR"
            echo "✅ Removed configuration: $CONFIG_DIR"
            ;;
        *)
            echo "⏭️  Kept configuration"
            ;;
    esac
else
    echo "⚠️  Configuration not found at: $CONFIG_DIR"
fi

# ============================================================================
# Final Message
# ============================================================================

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ Uninstallation Complete!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Remaining items (not automatically removed):"
echo "  • Project-level skill: ./.claude/skills/$SKILL_NAME"
echo "  • Qdrant data in Docker volumes (docker-compose down -v to remove)"
echo "  • Embedding server process (pkill -f 'python.*server.py')"
echo ""
echo "To reinstall: ./scripts/install.sh"
echo ""
