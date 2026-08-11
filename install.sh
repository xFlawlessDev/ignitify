#!/bin/sh
# Bootstrap installer. Safe to invoke with:
# curl -fsSL https://raw.githubusercontent.com/xFlawlessDev/ignitify/main/install.sh | sh
set -eu

RELEASES_URL="${IGNITIFY_RELEASES_URL:-https://github.com/xFlawlessDev/ignitify/releases}"
RELEASE="${IGNITIFY_VERSION:-latest}"

usage() {
  cat <<'EOF'
Usage: curl -fsSL https://raw.githubusercontent.com/xFlawlessDev/ignitify/main/install.sh | sh

To select a version:
  curl -fsSL https://raw.githubusercontent.com/xFlawlessDev/ignitify/main/install.sh | sh -s -- --release v0.1.0

Downloads the matching Linux Ignitify release bundle, verifies its SHA-256
checksum, and runs its privileged installer. The installer configures Docker
Engine, Docker Compose, Docker Buildx, Git, OpenSSH, Traefik assets with
writable fallback-page storage, and the Ignitify systemd service on supported
Linux hosts.

Environment:
  IGNITIFY_VERSION          Release tag to install (default: latest)
  IGNITIFY_RELEASES_URL     Alternate GitHub Releases-compatible URL for testing or mirrors
EOF
}

info() {
  printf '%s\n' "[ignitify] $*"
}

die() {
  printf '%s\n' "[ignitify] error: $*" >&2
  exit 1
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --release)
      [ "$#" -ge 2 ] || die "--release requires a version"
      RELEASE="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown option: $1"
      ;;
  esac
done

case "$RELEASE" in
  *[!A-Za-z0-9._-]*|'') die "release must contain only letters, numbers, dots, underscores, or hyphens" ;;
esac

[ "$(uname -s)" = "Linux" ] || die "Ignitify release bundles support Linux only"
case "$(uname -m)" in
  x86_64|amd64) ARCH="amd64" ;;
  aarch64|arm64) die "Ignitify releases currently support Linux amd64 only; ARM64 delivery is not available yet" ;;
  *) die "unsupported CPU architecture: $(uname -m)" ;;
esac

if command -v curl >/dev/null 2>&1; then
  fetch() {
    curl -fsSL --retry 3 --retry-delay 1 "$1" -o "$2"
  }
elif command -v wget >/dev/null 2>&1; then
  fetch() {
    wget -qO "$2" "$1"
  }
else
  die "curl or wget is required to download the Ignitify release bundle"
fi

command -v tar >/dev/null 2>&1 || die "tar is required to unpack the Ignitify release bundle"
command -v awk >/dev/null 2>&1 || die "awk is required to verify the release checksum"
command -v bash >/dev/null 2>&1 || die "bash is required by the Ignitify release installer"

RELEASES_URL="${RELEASES_URL%/}"
ARCHIVE_NAME="ignitify-linux-${ARCH}.tar.gz"
if [ "$RELEASE" = "latest" ]; then
  RELEASE_URL="$RELEASES_URL/latest/download"
else
  RELEASE_URL="$RELEASES_URL/download/$RELEASE"
fi
TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/ignitify-install.XXXXXX")" || die "could not create a temporary directory"
trap 'rm -rf "$TEMP_DIR"' EXIT HUP INT TERM

info "downloading Ignitify $RELEASE for linux/$ARCH"
fetch "$RELEASE_URL/$ARCHIVE_NAME" "$TEMP_DIR/$ARCHIVE_NAME"
fetch "$RELEASE_URL/SHA256SUMS" "$TEMP_DIR/SHA256SUMS"

EXPECTED_HASH="$(awk -v filename="$ARCHIVE_NAME" '$2 == filename || $2 == "*" filename { print $1; exit }' "$TEMP_DIR/SHA256SUMS")"
case "$EXPECTED_HASH" in
  ''|*[!0123456789abcdefABCDEF]*) die "SHA256SUMS does not contain a valid checksum for $ARCHIVE_NAME" ;;
esac

if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL_HASH="$(sha256sum "$TEMP_DIR/$ARCHIVE_NAME" | awk '{ print $1 }')"
elif command -v shasum >/dev/null 2>&1; then
  ACTUAL_HASH="$(shasum -a 256 "$TEMP_DIR/$ARCHIVE_NAME" | awk '{ print $1 }')"
elif command -v openssl >/dev/null 2>&1; then
  ACTUAL_HASH="$(openssl dgst -sha256 "$TEMP_DIR/$ARCHIVE_NAME" | awk '{ print $NF }')"
else
  die "sha256sum, shasum, or openssl is required to verify the release bundle"
fi

[ "$ACTUAL_HASH" = "$EXPECTED_HASH" ] || die "release checksum verification failed"

EXTRACT_DIR="$TEMP_DIR/extracted"
mkdir -p "$EXTRACT_DIR"
tar -xzf "$TEMP_DIR/$ARCHIVE_NAME" -C "$EXTRACT_DIR"

BUNDLE_INSTALLER=""
for candidate in "$EXTRACT_DIR"/*/install; do
  if [ -f "$candidate" ]; then
    BUNDLE_INSTALLER="$candidate"
    break
  fi
done
[ -n "$BUNDLE_INSTALLER" ] || die "release bundle does not contain its installer"

if [ "$(id -u)" -eq 0 ]; then
  bash "$BUNDLE_INSTALLER"
elif command -v sudo >/dev/null 2>&1; then
  info "administrator privileges are required to install system prerequisites"
  sudo bash "$BUNDLE_INSTALLER"
else
  die "run as root or install sudo, then run this command again"
fi
