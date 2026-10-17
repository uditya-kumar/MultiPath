#!/usr/bin/env bash
# Builds the signed macOS (Apple silicon) release and lays out its GitHub release assets:
#
#   scripts/release-macos.sh ["release notes shown in the update prompt"]
#
# Produces release-assets/ with the DMG, the updater archive and its signature, latest.json
# (the update feed the app reads from the latest release) and SHA256SUMS. Uploads nothing.
#
# The updater key is read from MULTIPATH_UPDATER_KEY (default ~/.config/multipath/updater.key) by
# path; it is never printed, copied or put in the environment as text.
set -euo pipefail

cd "$(dirname "$0")/.."
REPO="timowse/MultiPath-macOS"
TARGET="aarch64-apple-darwin"
KEY="${MULTIPATH_UPDATER_KEY:-$HOME/.config/multipath/updater.key}"
NOTES="${1:-}"
OUT="release-assets"

die() { echo "release-macos: $*" >&2; exit 1; }

[[ -f "$KEY" ]] || die "no updater key at $KEY (see RELEASE_INSTRUCTIONS.md)"
[[ "$(stat -f %Lp "$KEY")" == "600" ]] || die "$KEY must be mode 0600 (chmod 600 '$KEY')"
VERSION="$(node -p 'require("./src-tauri/tauri.conf.json").version')"
CONF_PUBKEY="$(node -p 'require("./src-tauri/tauri.conf.json").plugins.updater.pubkey')"
# The app trusts the key in tauri.conf.json; signing with any other key gives updates it refuses.
if [[ -f "$KEY.pub" && "$(tr -d '\n' < "$KEY.pub")" != "$CONF_PUBKEY" ]]; then
  die "$KEY.pub doesn't match plugins.updater.pubkey in tauri.conf.json"
fi
[[ "$(node -p 'require("./package.json").version')" == "$VERSION" ]] || die "package.json version isn't $VERSION"
grep -q "^version = \"$VERSION\"" src-tauri/Cargo.toml || die "Cargo.toml version isn't $VERSION"

# CI=true skips the DMG's Finder styling AppleScript, which needs Automation access.
# `tauri build` takes a key file's path in TAURI_SIGNING_PRIVATE_KEY (it ignores _PATH).
CI=true TAURI_SIGNING_PRIVATE_KEY="$KEY" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" \
  npx tauri build --target "$TARGET"

BUNDLE="src-tauri/target/$TARGET/release/bundle"
DMG="MultiPath_${VERSION}_aarch64.dmg"
ARCHIVE="MultiPath_${VERSION}_aarch64.app.tar.gz"
[[ -f "$BUNDLE/dmg/$DMG" ]] || die "missing $BUNDLE/dmg/$DMG"
[[ -f "$BUNDLE/macos/MultiPath.app.tar.gz.sig" ]] || die "no updater signature: is createUpdaterArtifacts on?"

rm -rf "$OUT/$DMG" "$OUT"/*.app.tar.gz "$OUT"/*.app.tar.gz.sig "$OUT/latest.json" "$OUT/SHA256SUMS"
mkdir -p "$OUT"
cp "$BUNDLE/dmg/$DMG" "$OUT/$DMG"
# Renaming doesn't touch the signature: it covers the bytes, not the name.
cp "$BUNDLE/macos/MultiPath.app.tar.gz" "$OUT/$ARCHIVE"
cp "$BUNDLE/macos/MultiPath.app.tar.gz.sig" "$OUT/$ARCHIVE.sig"

VERSION="$VERSION" NOTES="$NOTES" URL="https://github.com/$REPO/releases/download/v$VERSION/$ARCHIVE" \
  SIG_FILE="$OUT/$ARCHIVE.sig" node -e '
    const fs = require("fs");
    const feed = {
      version: process.env.VERSION,
      notes: process.env.NOTES || `MultiPath ${process.env.VERSION}`,
      pub_date: new Date().toISOString().replace(/\.\d+Z$/, "Z"),
      platforms: {
        "darwin-aarch64": {
          signature: fs.readFileSync(process.env.SIG_FILE, "utf8").trim(),
          url: process.env.URL,
        },
      },
    };
    fs.writeFileSync("release-assets/latest.json", JSON.stringify(feed, null, 2) + "\n");
  '

(cd "$OUT" && shasum -a 256 "$DMG" "$ARCHIVE" "$ARCHIVE.sig" latest.json > SHA256SUMS)
echo "release-macos: v$VERSION assets in $OUT/:"
ls -l "$OUT"
