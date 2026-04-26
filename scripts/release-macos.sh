#!/usr/bin/env bash
# Build macOS .app + .dmg + .pkg for distribution, with auto version bump.
#
# Outputs go to ~/Desktop/if2aiwen_pkg/:
#   - If2Ai.app                          (raw bundle, drag to /Applications)
#   - If2Ai_<version>_aarch64.dmg        (disk image, double-click to mount)
#   - If2Ai_<version>.pkg                (productbuild installer, double-click to install)
#
# Behavior:
#   1. Bump version in package.json + src-tauri/tauri.conf.json + src-tauri/Cargo.toml
#      Default bump level = patch (0.1.0 → 0.1.1). Override with first arg:
#        npm run release:macos -- patch   # default
#        npm run release:macos -- minor   # 0.1.0 → 0.2.0
#        npm run release:macos -- major   # 0.1.0 → 1.0.0
#        npm run release:macos -- none    # skip bump (rebuild current version)
#        npm run release:macos -- 1.2.3   # set explicit version
#   2. Pre-clean stale dmg state (Tauri bundle_dmg.sh idempotency).
#   3. Run tauri build (--bundles app,dmg).
#   4. Stage to ~/Desktop/if2aiwen_pkg + productbuild .pkg.
#
# Env overrides:
#   RELEASE_OUT_DIR=/some/dir   override output directory
#   MACOSX_DEPLOYMENT_TARGET=11.0 (forced; required by whisper.cpp std::filesystem)
#   TAURI_SIGNING_PRIVATE_KEY=/path-or-value supplied by CI/local shell; if unset,
#      this script auto-loads ~/.tauri/if2ai-updater.key for Tauri updater signing.
#   TAURI_SIGNING_PRIVATE_KEY_PASSWORD=... optional; defaults to empty for the
#      local if2ai updater key generated without a passphrase.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="${RELEASE_OUT_DIR:-$HOME/Desktop/if2aiwen_pkg}"
APP_NAME="If2Ai"
BUMP="${1:-patch}"

export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"

cd "$REPO_ROOT"

# ── Step 0: Tauri updater signing key ───────────────────────────────────────
# Tauri refuses release builds when updater.pubkey is configured but no private
# key is present. CI should inject TAURI_SIGNING_PRIVATE_KEY via secrets; local
# builds fall back to the checked-on-this-machine keypair under ~/.tauri.
if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
    LOCAL_UPDATER_KEY="$HOME/.tauri/if2ai-updater.key"
    if [[ -f "$LOCAL_UPDATER_KEY" ]]; then
        export TAURI_SIGNING_PRIVATE_KEY="$(cat "$LOCAL_UPDATER_KEY")"
        echo "==> Loaded Tauri updater signing key from $LOCAL_UPDATER_KEY"
    else
        echo "⚠️  TAURI_SIGNING_PRIVATE_KEY is unset and $LOCAL_UPDATER_KEY was not found." >&2
        echo "   If src-tauri/tauri.conf.json contains an updater pubkey, Tauri build will fail." >&2
    fi
fi

export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"

# ── Step 1: version bump ────────────────────────────────────────────────────
CURRENT="$(node -p "require('./package.json').version")"

if [[ "$BUMP" == "none" ]]; then
    NEW="$CURRENT"
    echo "==> Version unchanged: $NEW"
elif [[ "$BUMP" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    NEW="$BUMP"
    echo "==> Setting version: $CURRENT → $NEW (explicit)"
else
    NEW="$(node -e "
        const [maj, min, pat] = require('./package.json').version.split('.').map(Number);
        const level = process.argv[1];
        let next;
        if (level === 'major') next = [maj + 1, 0, 0];
        else if (level === 'minor') next = [maj, min + 1, 0];
        else if (level === 'patch') next = [maj, min, pat + 1];
        else { console.error('unknown bump:', level); process.exit(2); }
        console.log(next.join('.'));
    " "$BUMP")"
    echo "==> Bumping version: $CURRENT → $NEW ($BUMP)"
fi

if [[ "$NEW" != "$CURRENT" ]]; then
    # package.json
    node -e "
        const fs = require('fs');
        const p = './package.json';
        const j = JSON.parse(fs.readFileSync(p, 'utf8'));
        j.version = '$NEW';
        fs.writeFileSync(p, JSON.stringify(j, null, 4) + '\n');
    "
    # src-tauri/tauri.conf.json
    node -e "
        const fs = require('fs');
        const p = './src-tauri/tauri.conf.json';
        const j = JSON.parse(fs.readFileSync(p, 'utf8'));
        j.version = '$NEW';
        fs.writeFileSync(p, JSON.stringify(j, null, 4) + '\n');
    "
    # src-tauri/Cargo.toml — only the [package] section's `version =` line
    # (avoid clobbering dependency version pins elsewhere in the file).
    perl -i -pe 'BEGIN{$n=0} if (/^\[package\]/){$n=1} elsif (/^\[/){$n=0} if ($n && s/^version\s*=\s*"[^"]+"/version = "'"$NEW"'"/){$n=0}' \
        "$REPO_ROOT/src-tauri/Cargo.toml"
    # Cargo.lock — refresh the version recorded for if2ai-backend so cargo
    # doesn't think it's a dirty change.
    (cd src-tauri && cargo update -p if2ai-backend --offline >/dev/null 2>&1 || true)
fi

VERSION="$NEW"

# ── Helper: clean Tauri bundle_dmg.sh leftover state ────────────────────────
# bundle_dmg.sh occasionally races with Finder/Spotlight on macOS, leaving
# /Volumes/dmg.* mounted and rw.*.dmg temp files behind. Both must be cleared
# before each dmg attempt or hdiutil refuses with cryptic "failed to run
# bundle_dmg.sh".
clean_dmg_state() {
    hdiutil info | awk '/\/Volumes\/dmg\./{print $1}' | while read -r dev; do
        [[ -n "$dev" ]] && hdiutil detach "$dev" -force >/dev/null 2>&1 || true
    done
    # Use find to avoid zsh "no matches found" on empty glob
    find target/release/bundle/macos -maxdepth 1 -name 'rw.*.dmg' -delete 2>/dev/null || true
    find target/release/bundle/dmg -maxdepth 1 -name 'rw.*.dmg' -delete 2>/dev/null || true
}

# ── Step 2: pre-clean stale dmg state ───────────────────────────────────────
echo "==> Pre-clean stale dmg state"
clean_dmg_state

# ── Step 3: tauri build ─────────────────────────────────────────────────────
# Strategy: build .app first (so the freshly-compiled bundle is preserved
# regardless of what dmg packaging does), then build .dmg with retry.
echo "==> Tauri build $APP_NAME $VERSION (app)"
npx tauri build --bundles app

APP_BUNDLE_PATH="target/release/bundle/macos/$APP_NAME.app"
APP_RESTAGE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/if2ai-release-app.XXXXXX")"
trap 'rm -rf "$APP_RESTAGE_DIR"' EXIT

if [[ ! -d "$APP_BUNDLE_PATH" ]]; then
    echo "❌ Expected app bundle was not produced: $APP_BUNDLE_PATH" >&2
    exit 1
fi

echo "==> Snapshot .app for post-dmg restore"
ditto "$APP_BUNDLE_PATH" "$APP_RESTAGE_DIR/$APP_NAME.app"

DMG_MAX_RETRIES=3
dmg_attempt=1
while (( dmg_attempt <= DMG_MAX_RETRIES )); do
    echo "==> Tauri build $APP_NAME $VERSION (dmg) — attempt $dmg_attempt/$DMG_MAX_RETRIES"
    if npx tauri build --bundles dmg; then
        break
    fi
    if (( dmg_attempt == DMG_MAX_RETRIES )); then
        echo "❌ DMG bundling failed after $DMG_MAX_RETRIES attempts" >&2
        exit 1
    fi
    echo "==> dmg attempt $dmg_attempt failed; cleaning state and retrying in 3s"
    clean_dmg_state
    sleep 3
    ((dmg_attempt++))
done

# Tauri's --bundles dmg can clean the macos/.app dir as a side-effect. Restore
# the first build's app bundle instead of running another tauri build, otherwise
# beforeBuildCommand (`npm run build:web`) runs a second time and looks like a
# release loop.
if [[ ! -d "$APP_BUNDLE_PATH" ]]; then
    echo "==> Restore .app snapshot (dmg pass cleaned it)"
    mkdir -p "$(dirname "$APP_BUNDLE_PATH")"
    ditto "$APP_RESTAGE_DIR/$APP_NAME.app" "$APP_BUNDLE_PATH"
fi

# ── Step 4: stage + productbuild ────────────────────────────────────────────
echo "==> Stage to $OUT_DIR"
mkdir -p "$OUT_DIR"
rm -rf "$OUT_DIR/$APP_NAME.app"
cp -Rf "target/release/bundle/macos/$APP_NAME.app" "$OUT_DIR/"
cp -f target/release/bundle/dmg/*.dmg "$OUT_DIR/"

PKG_PATH="$OUT_DIR/${APP_NAME}_${VERSION}.pkg"
echo "==> productbuild → $(basename "$PKG_PATH")"
rm -f "$PKG_PATH"
productbuild --component "$OUT_DIR/$APP_NAME.app" /Applications "$PKG_PATH"

echo
echo "✅ Packaged $APP_NAME $VERSION → $OUT_DIR"
ls -lh "$OUT_DIR" | grep -E "$APP_NAME"
