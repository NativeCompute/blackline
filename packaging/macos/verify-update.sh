#!/bin/bash
set -euo pipefail

OLD_DIST="$(cd "${1:?usage: verify-update.sh <old release folder> <new release folder> <old version> <new version>}" && pwd)"
NEW_DIST="$(cd "${2:?new release folder}" && pwd)"
OLD="${3:?old version}"
NEW="${4:?new version}"
WORK="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/blackline-update-check"
APP="/Applications/Blackline.app"
BIN="$APP/Contents/MacOS/blackline"
SUPPORT="$HOME/Library/Application Support"

say() { printf '\033[1;34m::\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m:: FAIL\033[0m %s\n' "$*" >&2; exit 1; }

[ ! -e "$APP" ] || die "$APP already exists; this check needs a clean machine"
rm -rf "$WORK"
mkdir -p "$WORK"

say "macOS $(sw_vers -productVersion) on $(uname -m)"
say "1. Blackline $OLD installed the standard way"
curl -fsSL "file://$OLD_DIST/install-macos.sh" | BLACKLINE_SOURCE="file://$OLD_DIST" BLACKLINE_NO_OPEN=1 /bin/bash
[ "$("$BIN" --version)" = "blackline $OLD" ] || die "the installed version is not $OLD"

attempt() {
  "$BIN" --ncf-update "$NEW" --parent 0 --target "$APP" --data "$WORK/fail-data" --source "file://$1" \
    --name Blackline --slug blackline --repo NativeCompute/blackline --headless --no-reopen
}

say "2. A damaged download changes nothing"
mkdir -p "$WORK/bad-sum"
cp "$NEW_DIST/blackline-macos.zip" "$NEW_DIST/install-macos.sh" "$WORK/bad-sum/"
printf '%064d  blackline-macos.zip\n' 0 > "$WORK/bad-sum/blackline-macos.zip.sha256"
if attempt "$WORK/bad-sum" 2>&1 | tee "$WORK/bad-sum.log"; then die "a damaged download was accepted"; fi
grep -q "damaged" "$WORK/bad-sum.log" || die "a damaged download did not say so"
[ "$("$BIN" --version)" = "blackline $OLD" ] || die "a damaged download changed the app"

say "3. A missing download changes nothing"
mkdir -p "$WORK/empty"
if attempt "$WORK/empty" 2>&1 | tee "$WORK/empty.log"; then die "a missing download was accepted"; fi
[ "$("$BIN" --version)" = "blackline $OLD" ] || die "a missing download changed the app"

say "4. A new version that won't start is never installed"
mkdir -p "$WORK/broken-src" "$WORK/broken"
ditto -x -k "$NEW_DIST/blackline-macos.zip" "$WORK/broken-src"
printf '#!/bin/sh\nexit 1\n' > "$WORK/broken-src/Blackline.app/Contents/MacOS/blackline"
codesign --force --deep --sign - "$WORK/broken-src/Blackline.app"
(cd "$WORK/broken-src" && ditto -c -k --sequesterRsrc --keepParent Blackline.app "$WORK/broken/blackline-macos.zip")
(cd "$WORK/broken" && shasum -a 256 blackline-macos.zip > blackline-macos.zip.sha256)
cp "$NEW_DIST/install-macos.sh" "$WORK/broken/"
if attempt "$WORK/broken" 2>&1 | tee "$WORK/broken.log"; then die "a broken new version was accepted"; fi
grep -q "does not run" "$WORK/broken.log" || die "a broken new version did not say so"
[ "$("$BIN" --version)" = "blackline $OLD" ] || die "a broken new version changed the app"
codesign --verify --deep --strict "$APP" || die "a failed update damaged the installed app"

say "5. Update now, from the app opened through macOS"
open -W -n --stdout "$WORK/app.out" --stderr "$WORK/app.err" "$APP" --args --smoke-update "$WORK/smoke" "$NEW" "file://$NEW_DIST"
cat "$WORK/smoke/report.txt" 2>/dev/null || { cat "$WORK/app.err" >&2 || true; die "the app wrote no report"; }
[ "$(head -n 1 "$WORK/smoke/report.txt")" = "PASS" ] || die "the app did not hand over to the updater"
waited=0
while pgrep -f "ncf-update-blackline/blackline-updater" >/dev/null 2>&1; do
  [ "$waited" -lt 180 ] || die "the updater did not finish within three minutes"
  sleep 1
  waited=$((waited + 1))
done
LOG="$WORK/smoke/data/Blackline/update.log"
cat "$LOG"
[ "$("$BIN" --version)" = "blackline $NEW" ] || die "the app was not updated to $NEW"
grep -q "Updated to $NEW" "$LOG" || die "the log does not say the update finished"
drawn="$(sed -n 's/.*updater window drawn: \([0-9]*\) colours.*/\1/p' "$LOG" | tail -n 1)"
[ -n "$drawn" ] && [ "$drawn" -ge 12 ] || die "the updater window was not drawn (colours: ${drawn:-none})"
[ -s "$WORK/smoke/updater.png" ] || die "no screenshot of the updater window"
codesign --verify --deep --strict --verbose=2 "$APP" || die "the updated app's signature does not verify"
if xattr -p com.apple.quarantine "$APP" >/dev/null 2>&1; then die "the updated app carries the quarantine mark"; fi
lipo "$BIN" -verify_arch arm64 x86_64 || die "the updated app is not universal"
[ -z "$(find /Applications -maxdepth 1 -name '.Blackline.app.*')" ] || die "the update left a half-copied app behind"

say "6. The new version opened by itself"
reopened=0
for _ in $(seq 1 30); do
  if pgrep -f "$BIN" >/dev/null 2>&1; then reopened=1; break; fi
  sleep 1
done
[ "$reopened" = 1 ] || die "Blackline did not reopen after the update"
sleep 3
pkill -f "$BIN" || true
ls -d "$SUPPORT"/Blackline-backup-* >/dev/null 2>&1 || say "No library to copy aside on this machine before the update"

say "PASS: $OLD updated itself to $NEW through macOS, every failure left $OLD in place"
