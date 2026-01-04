#!/usr/bin/env bash
set -e

BINARY_NAME="ssearch"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
LIB_DIR="${LIB_DIR:-$HOME/.local/lib/ssearch}"
REPO="junyeong-ai/semantic-search-cli"
SKILL_NAME="semantic-search"
PROJECT_SKILL_DIR=".claude/skills/$SKILL_NAME"
USER_SKILL_DIR="$HOME/.claude/skills/$SKILL_NAME"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/ssearch"
CACHE_DIR="$HOME/.cache/semantic-search-cli"
MODELS_DIR="$CACHE_DIR/models"
EMBEDDING_MODEL="JunyeongAI/qwen3-embedding-0.6b-onnx"

detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)

    case "$os" in
        linux) os="unknown-linux-gnu" ;;
        darwin) os="apple-darwin" ;;
        *) echo "Unsupported OS: $os"; exit 1 ;;
    esac

    case "$arch" in
        x86_64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *) echo "Unsupported architecture: $arch"; exit 1 ;;
    esac

    echo "${arch}-${os}"
}

get_latest_version() {
    curl -sf "https://api.github.com/repos/$REPO/releases/latest" \
        | grep '"tag_name"' \
        | sed -E 's/.*"v([^"]+)".*/\1/' \
        || echo ""
}

download_binary() {
    local version="$1"
    local target="$2"
    local archive="ssearch-v${version}-${target}.tar.gz"
    local url="https://github.com/$REPO/releases/download/v${version}/${archive}"
    local checksum_url="${url}.sha256"

    echo "📥 Downloading $archive..." >&2
    if ! curl -fLO "$url" 2>&2; then
        echo "❌ Download failed" >&2
        return 1
    fi

    echo "🔐 Verifying checksum..." >&2
    if curl -fLO "$checksum_url" 2>&2; then
        if command -v sha256sum >/dev/null; then
            sha256sum -c "${archive}.sha256" >&2 || return 1
        elif command -v shasum >/dev/null; then
            shasum -a 256 -c "${archive}.sha256" >&2 || return 1
        else
            echo "⚠️  No checksum tool found, skipping verification" >&2
        fi
    else
        echo "⚠️  Checksum file not found, skipping verification" >&2
    fi

    echo "📦 Extracting..." >&2
    mkdir -p extracted
    tar -xzf "$archive" -C extracted 2>&2
    rm -f "$archive" "${archive}.sha256"

    # Return the extracted directory path
    echo "extracted"
}

check_onnxruntime() {
    # Check for ONNX Runtime library (must match paths in src/main.rs)
    if [[ "$OSTYPE" == "darwin"* ]]; then
        local ort_paths=(
            "$HOME/.local/lib/ssearch/libonnxruntime.dylib"
            "/opt/homebrew/opt/onnxruntime/lib/libonnxruntime.dylib"
            "/usr/local/opt/onnxruntime/lib/libonnxruntime.dylib"
        )
    else
        local ort_paths=(
            "$HOME/.local/lib/ssearch/libonnxruntime.so"
            "/usr/lib/libonnxruntime.so"
            "/usr/local/lib/libonnxruntime.so"
            "/usr/lib/x86_64-linux-gnu/libonnxruntime.so"
            "/usr/lib/aarch64-linux-gnu/libonnxruntime.so"
        )
    fi
    for path in "${ort_paths[@]}"; do
        [ -f "$path" ] && return 0
    done
    return 1
}

build_from_source() {
    echo "🔨 Building from source..." >&2

    # Check ONNX Runtime for source build
    if ! check_onnxruntime; then
        echo "" >&2
        echo "⚠️  ONNX Runtime not found. Required for source build." >&2
        echo "" >&2
        if [[ "$OSTYPE" == "darwin"* ]]; then
            echo "   Install via Homebrew:" >&2
            echo "     brew install onnxruntime" >&2
        else
            echo "   Install on Ubuntu/Debian:" >&2
            echo "     # Download from https://github.com/microsoft/onnxruntime/releases" >&2
            echo "     sudo cp libonnxruntime.so /usr/local/lib/" >&2
            echo "     sudo ldconfig" >&2
        fi
        echo "" >&2
        read -p "Continue anyway? [y/N]: " choice
        [[ ! "$choice" =~ ^[yY]$ ]] && exit 1
    fi

    if ! cargo build --release 2>&1 | grep -E "Compiling|Finished|error" >&2; then
        echo "❌ Build failed" >&2
        exit 1
    fi
    echo "target/release/$BINARY_NAME"
}

install_binary() {
    local source_path="$1"
    local is_extracted_dir="$2"

    mkdir -p "$INSTALL_DIR"
    mkdir -p "$LIB_DIR"

    if [ "$is_extracted_dir" = "true" ]; then
        # Install from extracted release archive (includes lib/)
        cp "$source_path/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"

        # Install ONNX Runtime libraries
        if [ -d "$source_path/lib" ]; then
            cp -r "$source_path/lib/"* "$LIB_DIR/"
            echo "✅ ONNX Runtime libraries installed to $LIB_DIR" >&2
        fi
        rm -rf "$source_path"
    else
        # Install from source build
        cp "$source_path" "$INSTALL_DIR/$BINARY_NAME"
    fi

    chmod +x "$INSTALL_DIR/$BINARY_NAME"

    if [[ "$OSTYPE" == "darwin"* ]]; then
        codesign --force --deep --sign - "$INSTALL_DIR/$BINARY_NAME" 2>/dev/null || true
    fi

    echo "✅ Installed to $INSTALL_DIR/$BINARY_NAME" >&2
}

get_skill_version() {
    local skill_md="$1"
    [ -f "$skill_md" ] && grep "^version:" "$skill_md" 2>/dev/null | sed 's/version: *//' || echo "unknown"
}

check_skill_exists() {
    [ -d "$USER_SKILL_DIR" ] && [ -f "$USER_SKILL_DIR/SKILL.md" ]
}

compare_versions() {
    local ver1="$1"
    local ver2="$2"

    if [ "$ver1" = "$ver2" ]; then
        echo "equal"
    elif [ "$ver1" = "unknown" ] || [ "$ver2" = "unknown" ]; then
        echo "unknown"
    else
        if [ "$(printf '%s\n' "$ver1" "$ver2" | sort -V | head -n1)" = "$ver1" ]; then
            [ "$ver1" != "$ver2" ] && echo "older" || echo "equal"
        else
            echo "newer"
        fi
    fi
}

backup_skill() {
    local timestamp=$(date +%Y%m%d_%H%M%S)
    local backup_dir="$USER_SKILL_DIR.backup_$timestamp"

    echo "📦 Creating backup: $backup_dir" >&2
    cp -r "$USER_SKILL_DIR" "$backup_dir"
    echo "   ✅ Backup created" >&2
}

install_skill() {
    echo "📋 Installing skill to $USER_SKILL_DIR" >&2
    mkdir -p "$(dirname "$USER_SKILL_DIR")"
    cp -r "$PROJECT_SKILL_DIR" "$USER_SKILL_DIR"
    echo "   ✅ Skill installed" >&2
}

prompt_skill_installation() {
    [ ! -d "$PROJECT_SKILL_DIR" ] && return 0

    local project_version=$(get_skill_version "$PROJECT_SKILL_DIR/SKILL.md")

    echo "" >&2
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" >&2
    echo "🤖 Claude Code Skill Installation" >&2
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" >&2
    echo "" >&2
    echo "Skill: $SKILL_NAME (v$project_version)" >&2
    echo "" >&2

    if check_skill_exists; then
        local existing_version=$(get_skill_version "$USER_SKILL_DIR/SKILL.md")
        local comparison=$(compare_versions "$existing_version" "$project_version")

        echo "Status: Already installed (v$existing_version)" >&2
        echo "" >&2

        case "$comparison" in
            equal)
                echo "✅ Latest version installed" >&2
                echo "" >&2
                read -p "Reinstall? [y/N]: " choice
                [[ "$choice" =~ ^[yY]$ ]] && { backup_skill; rm -rf "$USER_SKILL_DIR"; install_skill; } || echo "   ⏭️  Skipped" >&2
                ;;
            older)
                echo "🔄 New version available: v$project_version" >&2
                echo "" >&2
                read -p "Update? [Y/n]: " choice
                [[ ! "$choice" =~ ^[nN]$ ]] && { backup_skill; rm -rf "$USER_SKILL_DIR"; install_skill; echo "   ✅ Updated to v$project_version" >&2; } || echo "   ⏭️  Keeping current version" >&2
                ;;
            newer)
                echo "⚠️  Installed version (v$existing_version) > project version (v$project_version)" >&2
                echo "" >&2
                read -p "Downgrade? [y/N]: " choice
                [[ "$choice" =~ ^[yY]$ ]] && { backup_skill; rm -rf "$USER_SKILL_DIR"; install_skill; } || echo "   ⏭️  Keeping current version" >&2
                ;;
            *)
                echo "⚠️  Version comparison failed" >&2
                echo "" >&2
                read -p "Reinstall? [y/N]: " choice
                [[ "$choice" =~ ^[yY]$ ]] && { backup_skill; rm -rf "$USER_SKILL_DIR"; install_skill; } || echo "   ⏭️  Skipped" >&2
                ;;
        esac
    else
        echo "Installation options:" >&2
        echo "" >&2
        echo "  [1] User-level install (RECOMMENDED)" >&2
        echo "      → ~/.claude/skills/ (available in all projects)" >&2
        echo "" >&2
        echo "  [2] Project-level only" >&2
        echo "      → Works only in this project directory" >&2
        echo "" >&2
        echo "  [3] Skip" >&2
        echo "" >&2

        read -p "Choose [1-3] (default: 1): " choice
        case "$choice" in
            2)
                echo "" >&2
                echo "✅ Using project-level skill" >&2
                echo "   Location: $(pwd)/$PROJECT_SKILL_DIR" >&2
                ;;
            3)
                echo "" >&2
                echo "⏭️  Skipped" >&2
                ;;
            1|"")
                echo "" >&2
                install_skill
                echo "" >&2
                echo "🎉 Skill installed successfully!" >&2
                echo "" >&2
                echo "Claude Code can now:" >&2
                echo "  • Execute semantic searches automatically" >&2
                echo "  • Index local files and directories" >&2
                echo "  • Sync external sources (Jira, Confluence, Figma)" >&2
                ;;
            *)
                echo "" >&2
                echo "❌ Invalid choice. Skipped." >&2
                ;;
        esac
    fi

    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" >&2
}

check_dependencies() {
    echo "🔍 Checking dependencies..." >&2
    echo "" >&2

    local missing=0

    # Check Rust (only needed for source build)
    if command -v cargo >/dev/null 2>&1; then
        echo "  ✅ Rust: $(cargo --version)" >&2
    else
        echo "  ⚠️  Rust: Not installed (needed only for source build)" >&2
    fi

    # Check Docker (for Qdrant)
    if command -v docker >/dev/null 2>&1; then
        echo "  ✅ Docker: $(docker --version | cut -d' ' -f3 | tr -d ',')" >&2
    else
        echo "  ⚠️  Docker: Not installed (required for Qdrant)" >&2
    fi

    # Check ONNX Runtime (for source build or if not using bundled version)
    if check_onnxruntime; then
        echo "  ✅ ONNX Runtime: Found" >&2
    else
        echo "  ⚠️  ONNX Runtime: Not found (bundled with prebuilt binary)" >&2
    fi

    echo "" >&2
    return $missing
}

setup_config() {
    if [ -f "$CONFIG_DIR/config.toml" ]; then
        echo "ℹ️  Global config already exists at $CONFIG_DIR/config.toml" >&2
        return 0
    fi

    echo "📝 Creating global configuration..." >&2

    if [ ! -f "$INSTALL_DIR/$BINARY_NAME" ]; then
        echo "   ⚠️  Binary not found, skipping config creation" >&2
        return 0
    fi

    if "$INSTALL_DIR/$BINARY_NAME" config init --global >&2; then
        echo "   ✅ Global config created" >&2
    else
        echo "   ⚠️  Failed to create global config (run 'ssearch config init --global' manually)" >&2
    fi
}

# ============================================================================
# Model Download
# ============================================================================

download_model_curl() {
    local repo="$1"
    local dir="$2"
    local base_url="https://huggingface.co/$repo/resolve/main"

    mkdir -p "$MODELS_DIR/$dir"

    echo "  Downloading model.onnx..." >&2
    curl -L --progress-bar "$base_url/model.onnx" -o "$MODELS_DIR/$dir/model.onnx"

    echo "  Downloading model.onnx_data (~1.2GB)..." >&2
    curl -L --progress-bar "$base_url/model.onnx_data" -o "$MODELS_DIR/$dir/model.onnx_data"

    echo "  Downloading tokenizer.json..." >&2
    curl -L --progress-bar "$base_url/tokenizer.json" -o "$MODELS_DIR/$dir/tokenizer.json"
}

download_models() {
    echo "" >&2
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" >&2
    echo "🧠 ML Model Setup" >&2
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" >&2
    echo "" >&2

    mkdir -p "$MODELS_DIR"

    # Embedding model
    local embed_dir="$MODELS_DIR/JunyeongAI--qwen3-embedding-0.6b-onnx"
    if [ -f "$embed_dir/model.onnx" ] && [ -f "$embed_dir/model.onnx_data" ] && [ -f "$embed_dir/tokenizer.json" ]; then
        echo "✅ Embedding model already downloaded" >&2
    else
        echo "Embedding: $EMBEDDING_MODEL (~1.2GB)" >&2
        echo "" >&2
        read -p "Download embedding model? [Y/n]: " choice
        echo

        case "$choice" in
            n|N)
                echo "⏭️  Skipped embedding model" >&2
                ;;
            *)
                echo "📥 Downloading embedding model..." >&2
                if command -v huggingface-cli >/dev/null 2>&1; then
                    huggingface-cli download "$EMBEDDING_MODEL" \
                        --local-dir "$embed_dir" \
                        --include "model.onnx" "model.onnx_data" "tokenizer.json"
                else
                    download_model_curl "$EMBEDDING_MODEL" "JunyeongAI--qwen3-embedding-0.6b-onnx"
                fi

                if [ -f "$embed_dir/model.onnx" ] && [ -f "$embed_dir/model.onnx_data" ]; then
                    echo "✅ Embedding model downloaded" >&2
                else
                    echo "⚠️  Download may have failed" >&2
                fi
                ;;
        esac
    fi
}

main() {
    echo "🚀 Installing Semantic Search CLI (ssearch)..." >&2
    echo "" >&2

    check_dependencies

    local binary_path=""
    local target=$(detect_platform)
    local version=$(get_latest_version)

    echo "Target platform: $target" >&2

    if [ -n "$version" ] && command -v curl >/dev/null; then
        echo "Latest version: v$version" >&2
        echo "" >&2
        echo "Installation method:" >&2
        echo "  [1] Download prebuilt binary (RECOMMENDED - fast)" >&2
        echo "  [2] Build from source (requires Rust toolchain)" >&2
        echo "" >&2
        read -p "Choose [1-2] (default: 1): " method

        case "$method" in
            2)
                if ! command -v cargo >/dev/null 2>&1; then
                    echo "❌ Rust toolchain not found. Install from https://rustup.rs" >&2
                    exit 1
                fi
                binary_path=$(build_from_source)
                is_extracted="false"
                ;;
            1|"")
                binary_path=$(download_binary "$version" "$target") || {
                    echo "⚠️  Download failed, falling back to source build" >&2
                    if ! command -v cargo >/dev/null 2>&1; then
                        echo "❌ Rust toolchain not found. Install from https://rustup.rs" >&2
                        exit 1
                    fi
                    binary_path=$(build_from_source)
                    is_extracted="false"
                }
                is_extracted="true"
                ;;
            *)
                echo "❌ Invalid choice" >&2
                exit 1
                ;;
        esac
    else
        [ -z "$version" ] && echo "⚠️  Cannot fetch latest version, building from source" >&2
        if ! command -v cargo >/dev/null 2>&1; then
            echo "❌ Rust toolchain not found and cannot download binary." >&2
            echo "   Install Rust from https://rustup.rs" >&2
            exit 1
        fi
        binary_path=$(build_from_source)
        is_extracted="false"
    fi

    install_binary "$binary_path" "$is_extracted"

    echo "" >&2

    # Check PATH
    local path_ok=true
    if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
        path_ok=false
    fi

    # Check if ONNX Runtime is auto-detectable (bundled or system-installed)
    local ort_ok=false
    if check_onnxruntime; then
        ort_ok=true
    fi

    if [ "$path_ok" = "true" ] && [ "$ort_ok" = "true" ]; then
        echo "✅ Environment configured correctly" >&2
    else
        echo "⚠️  Add to shell profile (~/.bashrc, ~/.zshrc):" >&2
        echo "" >&2
        if [ "$path_ok" = "false" ]; then
            echo "  export PATH=\"\$HOME/.local/bin:\$PATH\"" >&2
        fi
        # Only show ORT_DYLIB_PATH if not auto-detectable
        if [ "$ort_ok" = "false" ]; then
            echo "" >&2
            echo "  # ONNX Runtime not found. Install or set path manually:" >&2
            if [[ "$OSTYPE" == "darwin"* ]]; then
                echo "  # Option 1: brew install onnxruntime" >&2
                echo "  # Option 2: export ORT_DYLIB_PATH=\"/path/to/libonnxruntime.dylib\"" >&2
            else
                echo "  # Option 1: Install from package manager or GitHub releases" >&2
                echo "  # Option 2: export ORT_DYLIB_PATH=\"/path/to/libonnxruntime.so\"" >&2
            fi
        fi
    fi
    echo "" >&2

    if [ -f "$INSTALL_DIR/$BINARY_NAME" ]; then
        echo "Installed version:" >&2
        "$INSTALL_DIR/$BINARY_NAME" --version >&2 || echo "  v0.1.0" >&2
        echo "" >&2
    fi

    setup_config
    download_models
    prompt_skill_installation

    echo "" >&2
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" >&2
    echo "🎉 Installation Complete!" >&2
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" >&2
    echo "" >&2
    echo "Next steps:" >&2
    echo "" >&2
    echo "1. Start Qdrant:" >&2
    echo "   docker-compose up -d qdrant" >&2
    echo "" >&2
    echo "2. Check status:         ssearch status" >&2
    echo "3. Index files:          ssearch index add <path>" >&2
    echo "4. Search:               ssearch search \"your query\"" >&2
    echo "" >&2
    echo "External sources (optional):" >&2
    echo "   ssearch source sync jira --query \"project=ABC\"" >&2
    echo "   ssearch source sync confluence --query \"space=DOCS\"" >&2
    echo "" >&2
}

main
