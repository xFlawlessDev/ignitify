#!/usr/bin/env bash
# Creates the artifacts consumed by the public install.sh bootstrapper.
set -Eeuo pipefail
IFS=$'\n\t'

VERSION=""
ARCH=""
BINARY=""
RAILPACK="${IGNITIFY_RAILPACK_BIN:-}"
OUTPUT_ROOT=""

usage() {
  cat <<'EOF'
Usage: scripts/package-release.sh --version VERSION --railpack PATH [options]

Creates:
  dist/VERSION/ignitify-linux-ARCH.tar.gz
  dist/VERSION/SHA256SUMS

Upload the archive, SHA256SUMS, and release metadata to the GitHub Release
tagged VERSION:
  https://github.com/xFlawlessDev/ignitify/releases/tag/VERSION

The bootstrapper downloads a default release from `releases/latest/download/`
and a selected release from `releases/download/VERSION/`. Re-running this
script for a second architecture refreshes SHA256SUMS with every archive in
that directory.

Options:
  --version VERSION       Required release identifier, for example v0.1.0.
  --arch ARCH             amd64 or arm64 (default: current host architecture).
  --binary PATH           Ignitify Linux binary (default: target/release/ignitify-core).
  --railpack PATH         Required matching Railpack Linux binary.
  --output PATH           Artifact root (default: dist).
  -h, --help              Show this help text.

Build the embedded frontend before producing the Rust release binary:
  cd frontend && pnpm install --frozen-lockfile && pnpm run build
  cargo build --locked --release -p ignitify-core
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
    --version)
      VERSION="$(need_value "$1" "${2:-}")"
      shift 2
      ;;
    --arch)
      ARCH="$(need_value "$1" "${2:-}")"
      shift 2
      ;;
    --binary)
      BINARY="$(need_value "$1" "${2:-}")"
      shift 2
      ;;
    --railpack)
      RAILPACK="$(need_value "$1" "${2:-}")"
      shift 2
      ;;
    --output)
      OUTPUT_ROOT="$(need_value "$1" "${2:-}")"
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

[[ -n "$VERSION" ]] || die "--version is required"
case "$VERSION" in
  *[!A-Za-z0-9._-]*|'') die "version must contain only letters, numbers, dots, underscores, or hyphens" ;;
esac

if [[ -z "$ARCH" ]]; then
  case "$(uname -m)" in
    x86_64|amd64) ARCH="amd64" ;;
    aarch64|arm64) ARCH="arm64" ;;
    *) die "unsupported host architecture: $(uname -m)" ;;
  esac
fi
[[ "$ARCH" == "amd64" || "$ARCH" == "arm64" ]] || die "--arch must be amd64 or arm64"

REPOSITORY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
BINARY="${BINARY:-$REPOSITORY_ROOT/target/release/ignitify-core}"
OUTPUT_ROOT="${OUTPUT_ROOT:-$REPOSITORY_ROOT/dist}"
[[ -f "$BINARY" ]] || die "Ignitify binary is missing: $BINARY"
[[ -n "$RAILPACK" && -f "$RAILPACK" ]] || die "a matching Railpack binary is required; pass --railpack PATH"

for command in install mktemp mv sha256sum tar; do
  command -v "$command" >/dev/null 2>&1 || die "required command is unavailable: $command"
done

INGRESS_SOURCE="$REPOSITORY_ROOT/infra/traefik"
for required in \
  compose.yaml \
  entrypoint.sh \
  traefik.yaml \
  fallback/Caddyfile \
  fallback/404.html \
  fallback/ignitify-mark.svg \
  socket-proxy/Dockerfile \
  socket-proxy/entrypoint.sh \
  dynamic/fallback.yml \
  dynamic/middlewares.yml; do
  [[ -f "$INGRESS_SOURCE/$required" ]] || die "release ingress asset is missing: infra/traefik/$required"
done

TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT
BUNDLE_NAME="ignitify-${VERSION}-linux-${ARCH}"
BUNDLE_DIR="$TEMP_DIR/$BUNDLE_NAME"
ARTIFACT_DIR="$OUTPUT_ROOT/$VERSION"
ARCHIVE_NAME="ignitify-linux-${ARCH}.tar.gz"

install -d -m 0755 "$BUNDLE_DIR/infra/traefik/fallback" "$BUNDLE_DIR/infra/traefik/socket-proxy" "$BUNDLE_DIR/infra/traefik/dynamic"
install -m 0755 "$REPOSITORY_ROOT/scripts/install-release.sh" "$BUNDLE_DIR/install"
install -m 0755 "$BINARY" "$BUNDLE_DIR/ignitify-core"
install -m 0755 "$RAILPACK" "$BUNDLE_DIR/railpack"
install -m 0644 "$INGRESS_SOURCE/compose.yaml" "$BUNDLE_DIR/infra/traefik/compose.yaml"
install -m 0755 "$INGRESS_SOURCE/entrypoint.sh" "$BUNDLE_DIR/infra/traefik/entrypoint.sh"
install -m 0644 "$INGRESS_SOURCE/traefik.yaml" "$BUNDLE_DIR/infra/traefik/traefik.yaml"
install -m 0644 "$INGRESS_SOURCE/fallback/Caddyfile" "$BUNDLE_DIR/infra/traefik/fallback/Caddyfile"
install -m 0644 "$INGRESS_SOURCE/fallback/404.html" "$BUNDLE_DIR/infra/traefik/fallback/404.html"
install -m 0644 "$INGRESS_SOURCE/fallback/ignitify-mark.svg" "$BUNDLE_DIR/infra/traefik/fallback/ignitify-mark.svg"
install -m 0644 "$INGRESS_SOURCE/socket-proxy/Dockerfile" "$BUNDLE_DIR/infra/traefik/socket-proxy/Dockerfile"
install -m 0755 "$INGRESS_SOURCE/socket-proxy/entrypoint.sh" "$BUNDLE_DIR/infra/traefik/socket-proxy/entrypoint.sh"
install -m 0644 "$INGRESS_SOURCE/dynamic/fallback.yml" "$BUNDLE_DIR/infra/traefik/dynamic/fallback.yml"
install -m 0644 "$INGRESS_SOURCE/dynamic/middlewares.yml" "$BUNDLE_DIR/infra/traefik/dynamic/middlewares.yml"

install -d -m 0755 "$ARTIFACT_DIR"
tar -C "$TEMP_DIR" -czf "$ARTIFACT_DIR/$ARCHIVE_NAME" "$BUNDLE_NAME"
(
  cd "$ARTIFACT_DIR"
  shopt -s nullglob
  archives=(ignitify-linux-*.tar.gz)
  ((${#archives[@]} > 0)) || die "no release archives found for checksum generation"
  sha256sum "${archives[@]}" > SHA256SUMS.tmp
  mv SHA256SUMS.tmp SHA256SUMS
)
printf '%s\n' "[ignitify-release] created $ARTIFACT_DIR/$ARCHIVE_NAME"
printf '%s\n' "[ignitify-release] created $ARTIFACT_DIR/SHA256SUMS"
