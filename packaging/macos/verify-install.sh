#!/bin/bash
set -euo pipefail

DIST="${1:?usage: verify-install.sh <folder with blackline-macos.zip, its .sha256 and install-macos.sh>}"
DIST="$(cd "$DIST" && pwd)"
WORK="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/blackline-verify"
LIB="$HOME/Library/Application Support/Blackline"
SUPPORT="$HOME/Library/Application Support"
APP="/Applications/Blackline.app"
USER_APPS="$HOME/Applications"

say() { printf '\033[1;34m::\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m:: FAIL\033[0m %s\n' "$*" >&2; exit 1; }

for f in blackline-macos.zip blackline-macos.zip.sha256 install-macos.sh; do
  [ -f "$DIST/$f" ] || die "$DIST/$f is missing"
done
[ ! -e "$APP" ] || die "$APP already exists; this check needs a clean machine"
[ ! -e "$LIB" ] || die "$LIB already exists; this check needs a clean machine"
rm -rf "$WORK"
mkdir -p "$WORK"

library_sum() {
  (cd "$LIB" && find . -type f | LC_ALL=C sort | while IFS= read -r f; do shasum -a 256 "$f"; done) | shasum -a 256 | awk '{print $1}'
}

installer() {
  curl -fsSL "file://$DIST/install-macos.sh" | BLACKLINE_SOURCE="file://$DIST" BLACKLINE_NO_OPEN=1 /bin/bash -s -- ${1+"$@"}
}

smoke_through_finder() {
  local app="$1" dir="$WORK/$2"
  say "Opening $app through macOS, like a double-click"
  open -W -n --stdout "$WORK/$2.out" --stderr "$WORK/$2.err" "$app" --args --smoke "$dir"
  [ -f "$dir/report.txt" ] || { cat "$WORK/$2.err" >&2 || true; die "the app opened from $app wrote no report"; }
  cat "$dir/report.txt"
  [ "$(head -n 1 "$dir/report.txt")" = "PASS" ] || die "the app opened from $app did not pass"
}

check_installed() {
  local app="$1" bin="$1/Contents/MacOS/blackline"
  [ -d "$app" ] || die "$app was not installed"
  codesign --verify --deep --strict --verbose=2 "$app" || die "the installed app's signature does not verify"
  if xattr -p com.apple.quarantine "$app" >/dev/null 2>&1; then
    die "the installed app still carries the quarantine mark"
  fi
  lipo "$bin" -verify_arch arm64 x86_64 || die "the installed app is not universal"
  plutil -lint "$app/Contents/Info.plist" || die "Info.plist does not lint"
  "$bin" --version || die "the installed program does not run"
}

say "macOS $(sw_vers -productVersion) on $(uname -m), /bin/bash $(/bin/bash -c 'echo $BASH_VERSION')"

mkdir -p "$LIB/scripts"
printf 'Title: Keep Me\n\nINT. KITCHEN - NIGHT\n\nNothing here may change.\n' > "$LIB/scripts/keep-me.fountain"
printf 'theme = dark\n' > "$LIB/settings.conf"
before="$(library_sum)"
say "A library is in place ($before)"

say "1. A fresh install, the way the README says"
installer
check_installed "$APP"
[ "$(library_sum)" = "$before" ] || die "the install changed the library"
backups="$(find "$SUPPORT" -maxdepth 1 -name 'Blackline-backup-*' | wc -l | tr -d ' ')"
[ "$backups" = "1" ] || die "expected one library backup, found $backups"
backup="$(find "$SUPPORT" -maxdepth 1 -name 'Blackline-backup-*' | head -n 1)"
[ "$( (cd "$backup" && find . -type f | LC_ALL=C sort | while IFS= read -r f; do shasum -a 256 "$f"; done) | shasum -a 256 | awk '{print $1}')" = "$before" ] || die "the backup does not match the library"
smoke_through_finder "$APP" smoke-first
[ "$(library_sum)" = "$before" ] || die "running the app changed the library"

say "2. An update while Blackline is open"
open -n --stdout "$WORK/during.out" --stderr "$WORK/during.err" "$APP" --args --smoke "$WORK/smoke-during"
waited=0
until pgrep -x blackline >/dev/null 2>&1; do
  [ "$waited" -lt 30 ] || die "the app did not start within 30 seconds"
  sleep 1
  waited=$((waited + 1))
done
installer 2>&1 | tee "$WORK/update.log"
grep -q "Blackline is open" "$WORK/update.log" || say "The app closed before the installer looked; the wait was not needed"
until [ -f "$WORK/smoke-during/report.txt" ] || [ "$waited" -ge 150 ]; do
  sleep 1
  waited=$((waited + 1))
done
[ "$(head -n 1 "$WORK/smoke-during/report.txt" 2>/dev/null)" = "PASS" ] || die "the app that was open during the update did not finish cleanly"
check_installed "$APP"
[ "$(library_sum)" = "$before" ] || die "the update changed the library"
[ -z "$(find /Applications -maxdepth 1 -name '.Blackline.app.*')" ] || die "the update left a half-copied app behind"
smoke_through_finder "$APP" smoke-updated

say "3. Uninstall"
installer --uninstall
[ ! -e "$APP" ] || die "uninstall left $APP"
[ "$(library_sum)" = "$before" ] || die "uninstall changed the library"

say "4. An account that cannot write to /Applications installs to ~/Applications"
export BLACKLINE_APP_DIR="$USER_APPS"
installer
unset BLACKLINE_APP_DIR
check_installed "$USER_APPS/Blackline.app"
[ ! -e "$APP" ] || die "the ~/Applications install also wrote to /Applications"
smoke_through_finder "$USER_APPS/Blackline.app" smoke-user
installer --uninstall
[ ! -e "$USER_APPS/Blackline.app" ] || die "uninstall left $USER_APPS/Blackline.app"
[ "$(library_sum)" = "$before" ] || die "the library changed"

say "PASS: install, update while open, uninstall and a per-user install all work, and the library never changed"
