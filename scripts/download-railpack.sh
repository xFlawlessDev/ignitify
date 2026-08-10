#!/usr/bin/env bash
# Downloads the reviewed Railpack binary used in Ignitify release bundles.
set -Eeuo pipefail
IFS=$'\n\t'

readonly RAILPACK_VERSION="0.35.0"
readonly RELEASE_URL="https://github.com/railwayapp/railpack/releases/download/v${RAILPACK_VERSION}"

OUTPUT=""

usage() {
  cat <<'EOF'
Usage: scripts/download-railpack.sh --output PATH

Downloads the Linux amd64 Railpack binary needed by build-release.sh and
verifies it against a reviewed SHA-256 checksum before writing PATH.
EOF
}

die() {
  printf '%s\n' "[ignitify-release] error: $*" >&2
  exit 1
}

need_value() {
  local option="$1"
  local value="${2:-}"
  [[ -n "$value" ]] || die "$option requires a value"
  printf '%s' "$value"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output)
      OUTPUT="$(need_value "$1" "${2:-}")"
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

[[ -n "$OUTPUT" ]] || die "--output is required"
[[ "$OUTPUT" != *$'\n'* ]] || die "--output cannot contain a newline"
[[ ! -e "$OUTPUT" ]] || die "refusing to overwrite an existing output: $OUTPUT"

for command in curl dirname install mktemp sha256sum tar; do
  command -v "$command" >/dev/null 2>&1 || die "required command is unavailable: $command"
done

case "$(uname -m)" in
  x86_64|amd64)
    TARGET="x86_64-unknown-linux-musl"
    SHA256="d039785dd926ba059031c9c463c51f1462f344c844f828ac872c1f6d46fed7f1"
    ;;
  aarch64|arm64) die "Railpack download for ARM64 is disabled until Ignitify ARM64 releases are validated" ;;
  *)
    die "unsupported Linux architecture: $(uname -m)"
    ;;
esac

ARCHIVE="railpack-v${RAILPACK_VERSION}-${TARGET}.tar.gz"
TEMPORARY_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMPORARY_DIR"' EXIT

curl --fail --location --retry 3 --retry-delay 1 \
  "$RELEASE_URL/$ARCHIVE" \
  -o "$TEMPORARY_DIR/$ARCHIVE"
(
  cd "$TEMPORARY_DIR"
  printf '%s  %s\n' "$SHA256" "$ARCHIVE" | sha256sum --check --status -
)

tar -xzf "$TEMPORARY_DIR/$ARCHIVE" -C "$TEMPORARY_DIR" railpack
install -d -m 0755 "$(dirname "$OUTPUT")"
install -m 0755 "$TEMPORARY_DIR/railpack" "$OUTPUT"
printf '%s\n' "[ignitify-release] downloaded verified Railpack v${RAILPACK_VERSION} for $TARGET"
