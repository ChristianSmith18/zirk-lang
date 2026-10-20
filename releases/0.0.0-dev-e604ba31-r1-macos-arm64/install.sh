#!/bin/sh
set -eu

VERSION='0.0.0-dev-e604ba31-r1'
TARGET='macos-arm64'
ARCHIVE='zirk-macos-arm64.tar.gz'
EXPECTED_LINK='../share/zirk/current/bin/zirk'
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
DATA_ROOT="$HOME/.local/share/zirk"
BIN_ROOT="$HOME/.local/bin"
VERSIONS_ROOT="$DATA_ROOT/versions"
VERSION_ROOT="$VERSIONS_ROOT/$VERSION-$TARGET"
CURRENT_LINK="$DATA_ROOT/current"
COMMAND_LINK="$BIN_ROOT/zirk"

step() { printf '\n[%s] %s\n' "$1" "$2"; }
fail() { printf '\nERROR: %s\n' "$1" >&2; exit 1; }

step '1/5' 'Checking installation platform...'
[ "$(uname -s)" = 'Darwin' ] || fail 'This release supports macOS only.'
[ "$(uname -m)" = 'arm64' ] || fail 'This release supports Apple silicon (arm64) only.'
[ -n "${HOME:-}" ] || fail 'HOME is not set; cannot determine the user installation directory.'
printf '      Installation scope: current user\n'
printf '      Data directory: %s\n' "$DATA_ROOT"
printf '      Command directory: %s\n' "$BIN_ROOT"

step '2/5' 'Verifying the release archive checksum...'
[ -f "$SCRIPT_DIR/$ARCHIVE" ] || fail "Release archive not found: $SCRIPT_DIR/$ARCHIVE"
[ -f "$SCRIPT_DIR/SHA256SUMS" ] || fail "Checksum file not found: $SCRIPT_DIR/SHA256SUMS"
(cd "$SCRIPT_DIR" && shasum -a 256 -c SHA256SUMS) || fail 'Release archive checksum verification failed.'

step '3/5' 'Installing the versioned compiler and matching runtime...'
mkdir -p "$VERSIONS_ROOT" "$BIN_ROOT"
if [ -e "$VERSION_ROOT" ]; then
  [ -f "$VERSION_ROOT/manifest.json" ] || fail "An incomplete installation already exists at $VERSION_ROOT; remove that version directory and retry."
  grep -q '"version": "0.0.0-dev-e604ba31-r1"' "$VERSION_ROOT/manifest.json" || fail "A different payload already occupies $VERSION_ROOT; remove that version directory and retry."
  printf '      This version is already present; keeping its installed files.\n'
else
  STAGE=$(mktemp -d "$VERSIONS_ROOT/.install-$VERSION.XXXXXX") || fail 'Could not create a temporary installation directory.'
  trap 'rm -rf "$STAGE"' EXIT HUP INT TERM
  tar -xzf "$SCRIPT_DIR/$ARCHIVE" -C "$STAGE" || fail 'Could not unpack the release archive.'
  [ -x "$STAGE/bin/zirk" ] || fail 'The release archive does not contain an executable compiler.'
  [ -f "$STAGE/bin/libzirk_runtime.a" ] || fail 'The matching Zirk runtime archive is missing.'
  [ -f "$STAGE/manifest.json" ] || fail 'The release manifest is missing.'
  mv "$STAGE" "$VERSION_ROOT"
  trap - EXIT HUP INT TERM
  printf '      Compiler: %s/bin/zirk\n' "$VERSION_ROOT"
  printf '      Runtime:  %s/bin/libzirk_runtime.a\n' "$VERSION_ROOT"
  printf '      Private libraries: %s/lib\n' "$VERSION_ROOT"
fi

step '4/5' 'Activating this version and registering the zirk command...'
if [ -e "$CURRENT_LINK" ] && [ ! -L "$CURRENT_LINK" ]; then
  fail "Refusing to replace a non-symlink at $CURRENT_LINK."
fi
if [ -e "$COMMAND_LINK" ] || [ -L "$COMMAND_LINK" ]; then
  [ -L "$COMMAND_LINK" ] || fail "Refusing to replace an existing non-symlink command at $COMMAND_LINK."
  [ "$(readlink "$COMMAND_LINK")" = "$EXPECTED_LINK" ] || fail "Refusing to replace an unrelated zirk command at $COMMAND_LINK."
fi
NEXT_LINK="$DATA_ROOT/.current-$VERSION-$$"
ln -s "$VERSION_ROOT" "$NEXT_LINK"
if [ -L "$CURRENT_LINK" ]; then
  mv -fh "$NEXT_LINK" "$CURRENT_LINK"
else
  mv "$NEXT_LINK" "$CURRENT_LINK"
fi
ln -sfn "$EXPECTED_LINK" "$COMMAND_LINK"
printf '      Active version: %s\n' "$CURRENT_LINK"
printf '      Command: %s/zirk\n' "$BIN_ROOT"

step '5/5' 'Checking that the installed compiler starts without development environment variables...'
VERSION_OUTPUT=$(env -u LLVM_SYS_201_PREFIX PATH="/usr/bin:/bin:/usr/sbin:/sbin:$BIN_ROOT" "$COMMAND_LINK" --version) || fail 'The installed compiler could not start. Review the installation paths and macOS security settings.'
printf '      %s\n' "$VERSION_OUTPUT"
case ":${PATH:-}:" in
  *":$BIN_ROOT:"*) ;;
  *) printf '\nNOTE: %s is not on PATH in this installer process. Add it to your shell configuration to run `zirk` by name.\n' "$BIN_ROOT" ;;
esac
printf '\nZirk %s is installed and ready.\n' "$VERSION"
printf 'Run `zirk --help` to see available commands.\n'
