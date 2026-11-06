#!/usr/bin/env bash
set -euo pipefail

REL="$(cd "${1:?usage: verify-update.sh <folder with both releases> <old version> <new version>}" && pwd)"
OLD="${2:?old version}"
NEW="${3:?new version}"
WORK="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/blackline-update-check"

say() { printf '\033[1;34m::\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m:: FAIL\033[0m %s\n' "$*" >&2; exit 1; }

rm -rf "$WORK"
mkdir -p "$WORK/home" "$WORK/tmp"
export HOME="$WORK/home"
export TMPDIR="$WORK/tmp"
unset XDG_DATA_HOME
BIN="$HOME/.local/bin/blackline"
LIB="$HOME/.local/share/blackline"

say "1. Blackline $OLD installed the standard way"
tar -xzf "$REL/blackline-v$OLD-linux-x86_64.tar.gz" -C "$WORK"
(cd "$WORK/blackline-v$OLD-linux-x86_64" && ./install.sh)
[ "$("$BIN" --version)" = "blackline $OLD" ] || die "the installed version is not $OLD"
mkdir -p "$LIB/scripts"
printf 'Title: Keep Me\n\nINT. KITCHEN - NIGHT\n' > "$LIB/scripts/keep-me.fountain"

attempt() {
  "$BIN" --ncf-update "$NEW" --parent 0 --target "$BIN" --data "$WORK/fail-data" --source "file://$1" \
    --name Blackline --slug blackline --repo NativeCompute/blackline --headless --no-reopen
}

say "2. A damaged download changes nothing"
mkdir -p "$WORK/bad-sum"
cp "$REL/blackline-v$NEW-linux-x86_64.tar.gz" "$WORK/bad-sum/"
printf '%064d  x\n' 0 > "$WORK/bad-sum/blackline-v$NEW-linux-x86_64.tar.gz.sha256"
if attempt "$WORK/bad-sum" 2>&1 | tee "$WORK/bad-sum.log"; then die "a damaged download was accepted"; fi
grep -q "damaged" "$WORK/bad-sum.log" || die "a damaged download did not say so"
[ "$("$BIN" --version)" = "blackline $OLD" ] || die "a damaged download changed the app"

say "3. A missing download changes nothing"
mkdir -p "$WORK/empty"
if attempt "$WORK/empty" 2>&1 | tee "$WORK/empty.log"; then die "a missing download was accepted"; fi
[ "$("$BIN" --version)" = "blackline $OLD" ] || die "a missing download changed the app"

say "4. A new version that won't start is never installed"
mkdir -p "$WORK/broken-src" "$WORK/broken"
tar -xzf "$REL/blackline-v$NEW-linux-x86_64.tar.gz" -C "$WORK/broken-src"
printf '#!/bin/sh\nexit 1\n' > "$WORK/broken-src/blackline-v$NEW-linux-x86_64/blackline"
(cd "$WORK/broken-src" && tar -czf "$WORK/broken/blackline-v$NEW-linux-x86_64.tar.gz" "blackline-v$NEW-linux-x86_64")
(cd "$WORK/broken" && sha256sum "blackline-v$NEW-linux-x86_64.tar.gz" > "blackline-v$NEW-linux-x86_64.tar.gz.sha256")
if attempt "$WORK/broken" 2>&1 | tee "$WORK/broken.log"; then die "a broken new version was accepted"; fi
grep -q "does not run" "$WORK/broken.log" || die "a broken new version did not say so"
[ "$("$BIN" --version)" = "blackline $OLD" ] || die "a broken new version changed the app"

say "5. Update now, from the app itself"
"$BIN" --smoke-update "$WORK/smoke" "$NEW" "file://$REL" || { cat "$WORK/smoke/report.txt" || true; die "the app did not hand over to the updater"; }
waited=0
while pgrep -f "ncf-update-blackline/blackline-updater" >/dev/null 2>&1; do
  [ "$waited" -lt 120 ] || die "the updater did not finish within two minutes"
  sleep 1
  waited=$((waited + 1))
done
DATA="$WORK/smoke/data/blackline"
cat "$DATA/update.log"
[ "$("$BIN" --version)" = "blackline $NEW" ] || die "the app was not updated to $NEW"
grep -q "Updated to $NEW" "$DATA/update.log" || die "the log does not say the update finished"
drawn="$(sed -n 's/.*updater window drawn: \([0-9]*\) colours.*/\1/p' "$DATA/update.log" | tail -n 1)"
[ -n "$drawn" ] && [ "$drawn" -ge 12 ] || die "the updater window was not drawn (colours: ${drawn:-none})"
[ -s "$WORK/smoke/updater.png" ] || die "no screenshot of the updater window"
ls -d "$WORK/smoke/data"/blackline-backup-* >/dev/null 2>&1 || die "the library was not copied aside first"
[ -f "$LIB/scripts/keep-me.fountain" ] || die "a script went missing"

say "6. The new version opened by itself"
reopened=0
for _ in $(seq 1 30); do
  if pgrep -f "^$BIN" >/dev/null 2>&1; then reopened=1; break; fi
  sleep 1
done
[ "$reopened" = 1 ] || die "Blackline did not reopen after the update"
sleep 3
pkill -f "^$BIN" || true
[ ! -e "$TMPDIR/ncf-update-blackline" ] || die "the updater left its files behind"

say "PASS: $OLD updated itself to $NEW, every failure left $OLD in place, and the library is intact"
