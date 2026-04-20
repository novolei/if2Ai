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

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="${RELEASE_OUT_DIR:-$HOME/Desktop/if2aiwen_pkg}"
APP_NAME="If2Ai"
BUMP="${1:-patch}"

export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"

cd "$REPO_ROOT"

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

# ── Step 2: pre-clean stale dmg state ───────────────────────────────────────
echo "==> Pre-clean stale dmg state (Tauri bundle_dmg.sh idempotency)"
hdiutil info | awk '/\/Volumes\/dmg\./{print $1}' | while read -r dev; do
    [[ -n "$dev" ]] && hdiutil detach "$dev" -force >/dev/null 2>&1 || true
done
rm -f target/release/bundle/macos/rw.*.dmg

# ── Step 3: tauri build ─────────────────────────────────────────────────────
echo "==> Tauri build $APP_NAME $VERSION (app + dmg)"
npx tauri build --bundles app,dmg

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
