#!/usr/bin/env bash
# Builds and packages one native Linux Ignitify release archive.
set -Eeuo pipefail
IFS=$'\n\t'

readonly REPOSITORY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly VERSION_SCRIPT="$REPOSITORY_ROOT/scripts/version.sh"
readonly PACKAGE_SCRIPT="$REPOSITORY_ROOT/scripts/package-release.sh"

VERSION=""
ARCH=""
RAILPACK="${IGNITIFY_RAILPACK_BIN:-}"
OUTPUT_ROOT=""
SKIP_INSTALL=0
SKIP_CHECK=0
REQUIRE_TAG=0
DRY_RUN=0
ALLOW_DIRTY=0

usage() {
  cat <<'EOF'
Usage: scripts/build-release.sh --railpack PATH [options]

Builds the embedded Vue frontend and the native Linux Ignitify binary, then
creates a signed-by-checksum release archive. The version comes from an exact
vX.Y.Z Git tag when present, or a deterministic development snapshot.

Options:
  --railpack PATH       Required matching native Railpack binary.
  --version VERSION     Override the generated semantic version.
  --arch ARCH           amd64 or arm64; must match the Linux build host.
  --output PATH         Artifact root (default: dist).
  --require-tag         Refuse untagged snapshot releases.
  --skip-install        Skip pnpm install --frozen-lockfile.
  --skip-check          Skip frontend and Cargo quality checks.
  --allow-dirty         Permit a working tree with uncommitted files.
  --dry-run             Print the release plan without building or writing artifacts.
  -h, --help            Show this help text.

Run this once on each native Linux architecture. package-release.sh refreshes
SHA256SUMS so a shared release directory can contain both archives.
EOF
}

die() {
  printf '%s\n' "[ignitify-release] error: $*" >&2
  exit 1
}

normalize_version() {
  local version="${1#v}"
  [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z-]+([.][0-9A-Za-z-]+)*)?$ ]] \
    || return 1
  printf '%s' "$version"
}

host_architecture() {
  case "$(uname -m)" in
    x86_64|amd64) printf 'amd64' ;;
    aarch64|arm64) printf 'arm64' ;;
    *) die "unsupported Linux build host architecture: $(uname -m)" ;;
  esac
}

run() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf '+ '
    printf '%q ' "$@"
    printf '\n'
    return 0
  fi
  "$@"
}

run_in_directory() {
  local directory="$1"
  shift
  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf '+ (cd %q && ' "$directory"
    printf '%q ' "$@"
    printf ')\n'
    return 0
  fi
  (
    cd "$directory"
    "$@"
  )
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --railpack)
      [[ $# -ge 2 ]] || die "--railpack requires a path"
      RAILPACK="$2"
      shift 2
      ;;
    --version)
      [[ $# -ge 2 ]] || die "--version requires a version"
      VERSION="$(normalize_version "$2")" || die "version must be semantic version text such as 0.2.0 or 0.2.0-rc.1"
      shift 2
      ;;
    --arch)
      [[ $# -ge 2 ]] || die "--arch requires amd64 or arm64"
      ARCH="$2"
      shift 2
      ;;
    --output)
      [[ $# -ge 2 ]] || die "--output requires a path"
      OUTPUT_ROOT="$2"
      shift 2
      ;;
    --require-tag)
      REQUIRE_TAG=1
      shift
      ;;
    --skip-install)
      SKIP_INSTALL=1
      shift
      ;;
    --skip-check)
      SKIP_CHECK=1
      shift
      ;;
    --allow-dirty)
      ALLOW_DIRTY=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
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

[[ "$(uname -s)" == "Linux" ]] || die "release builds run on native Linux amd64 or arm64 hosts"
[[ -x "$VERSION_SCRIPT" || -f "$VERSION_SCRIPT" ]] || die "version script is missing: $VERSION_SCRIPT"
[[ -f "$PACKAGE_SCRIPT" ]] || die "package script is missing: $PACKAGE_SCRIPT"

HOST_ARCH="$(host_architecture)"
ARCH="${ARCH:-$HOST_ARCH}"
[[ "$ARCH" == "amd64" || "$ARCH" == "arm64" ]] || die "--arch must be amd64 or arm64"
[[ "$ARCH" == "$HOST_ARCH" ]] || die "cross-compilation is not configured; run the build on native linux/$ARCH"

if [[ -z "$VERSION" ]]; then
  if [[ "$REQUIRE_TAG" -eq 1 ]]; then
    VERSION="$(bash "$VERSION_SCRIPT" --exact-tag)"
  else
    VERSION="$(bash "$VERSION_SCRIPT" --from-git)"
  fi
fi
if [[ "$REQUIRE_TAG" -eq 1 ]]; then
  TAG_VERSION="$(bash "$VERSION_SCRIPT" --exact-tag)"
  [[ "$VERSION" == "$TAG_VERSION" ]] \
    || die "--version must match the exact Git tag when --require-tag is set"
fi
RELEASE_TAG="v$VERSION"
OUTPUT_ROOT="${OUTPUT_ROOT:-$REPOSITORY_ROOT/dist}"

if [[ "$DRY_RUN" -eq 0 ]]; then
  for command in cargo git node pnpm; do
    command -v "$command" >/dev/null 2>&1 || die "required command is unavailable: $command"
  done
  [[ -n "$RAILPACK" && -f "$RAILPACK" ]] || die "a matching Railpack binary is required; pass --railpack PATH"
fi
if [[ "$ALLOW_DIRTY" -eq 0 ]] && [[ -n "$(git -C "$REPOSITORY_ROOT" status --porcelain)" ]]; then
  die "working tree is not clean; commit, stash, or pass --allow-dirty deliberately"
fi

printf '%s\n' "[ignitify-release] version=$VERSION tag=$RELEASE_TAG architecture=$ARCH"

if [[ "$SKIP_INSTALL" -eq 0 ]]; then
  run_in_directory "$REPOSITORY_ROOT/frontend" pnpm install --frozen-lockfile
fi
if [[ "$SKIP_CHECK" -eq 0 ]]; then
  run_in_directory "$REPOSITORY_ROOT/frontend" pnpm run check
  run_in_directory "$REPOSITORY_ROOT/frontend" pnpm run test
fi

if [[ "$DRY_RUN" -eq 1 ]]; then
  printf '+ (cd %q && IGNITIFY_APP_VERSION=%q pnpm run build)\n' "$REPOSITORY_ROOT/frontend" "$VERSION"
else
  (
    cd "$REPOSITORY_ROOT/frontend"
    IGNITIFY_APP_VERSION="$VERSION" pnpm run build
  )
fi

if [[ "$SKIP_CHECK" -eq 0 ]]; then
  run cargo fmt --all -- --check
  run cargo check --workspace
  run cargo test --workspace
  run cargo clippy --workspace --all-targets -- -D warnings
fi
run cargo build --locked --release -p ignitify-core

CORE_BINARY="$REPOSITORY_ROOT/target/release/ignitify-core"
if [[ "$DRY_RUN" -eq 0 ]]; then
  [[ -f "$CORE_BINARY" ]] || die "release binary was not produced: $CORE_BINARY"
fi

run bash "$PACKAGE_SCRIPT" \
  --version "$RELEASE_TAG" \
  --arch "$ARCH" \
  --binary "$CORE_BINARY" \
  --railpack "$RAILPACK" \
  --output "$OUTPUT_ROOT"

if [[ "$DRY_RUN" -eq 1 ]]; then
  printf '+ write %q\n' "$OUTPUT_ROOT/$RELEASE_TAG/release-linux-$ARCH.json"
  exit 0
fi

COMMIT_SHA="$(git -C "$REPOSITORY_ROOT" rev-parse HEAD)"
BUILT_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
ARTIFACT_DIR="$OUTPUT_ROOT/$RELEASE_TAG"
RELEASE_METADATA="$ARTIFACT_DIR/release-linux-$ARCH.json"
cat > "$RELEASE_METADATA" <<EOF
{
  "version": "$VERSION",
  "tag": "$RELEASE_TAG",
  "commit": "$COMMIT_SHA",
  "architecture": "$ARCH",
  "built_at": "$BUILT_AT"
}
EOF
printf '%s\n' "[ignitify-release] created $RELEASE_METADATA"
