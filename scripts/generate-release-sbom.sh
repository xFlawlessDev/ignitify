#!/usr/bin/env bash
# Generates the CycloneDX documents shipped beside a Linux release archive.
set -Eeuo pipefail
IFS=$'\n\t'

readonly REPOSITORY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly GENERATED_SBOM_NAME="ignitify-release.cdx.json"
readonly FRONTEND_SBOM_NAME="ignitify-frontend.cdx.json"

usage() {
  cat <<'EOF'
Usage: scripts/generate-release-sbom.sh OUTPUT_DIRECTORY

Creates CycloneDX 1.5 SBOMs for every Rust crate linked into the native binary
and the embedded frontend bundle. The following pinned tools must already be
available:
  cargo-cyclonedx 0.5.9
  pnpm 11.18.0
EOF
}

die() {
  printf '%s\n' "[ignitify-sbom] error: $*" >&2
  exit 1
}

cleanup_rust_sboms() {
  find "$REPOSITORY_ROOT/crates" -type f -name "$GENERATED_SBOM_NAME" -delete
}

[[ $# -eq 1 ]] || {
  usage >&2
  exit 1
}

readonly OUTPUT_DIRECTORY="$1"
[[ -d "$OUTPUT_DIRECTORY" ]] || die "output directory does not exist: $OUTPUT_DIRECTORY"
command -v cargo >/dev/null 2>&1 || die "cargo is required"
command -v node >/dev/null 2>&1 || die "node is required"
command -v pnpm >/dev/null 2>&1 || die "pnpm is required"

if find "$REPOSITORY_ROOT/crates" -type f -name "$GENERATED_SBOM_NAME" -print -quit | grep -q .; then
  die "refusing to replace an existing Rust SBOM in the source tree"
fi

export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git -C "$REPOSITORY_ROOT" log -1 --format=%ct)}"

trap cleanup_rust_sboms EXIT
(
  cd "$REPOSITORY_ROOT"
  cargo cyclonedx \
    --manifest-path crates/ignitify-core/Cargo.toml \
    --format json \
    --spec-version 1.5 \
    --all \
    --no-build-deps \
    --target x86_64-unknown-linux-gnu \
    --override-filename "${GENERATED_SBOM_NAME%.json}"
)
mapfile -t generated_sboms < <(
  find "$REPOSITORY_ROOT/crates" -type f -name "$GENERATED_SBOM_NAME" -print | sort
)
(( ${#generated_sboms[@]} > 0 )) || die "Rust SBOMs were not created"
for source in "${generated_sboms[@]}"; do
  crate_name="$(basename "$(dirname "$source")")"
  destination="$OUTPUT_DIRECTORY/ignitify-rust-$crate_name.cdx.json"
  [[ ! -e "$destination" ]] || die "SBOM output already exists: $destination"
  mv "$source" "$destination"
  printf '%s\n' "[ignitify-sbom] created $destination"
done
cleanup_rust_sboms
trap - EXIT

readonly FRONTEND_OUTPUT_PATH="$OUTPUT_DIRECTORY/$FRONTEND_SBOM_NAME"
node "$REPOSITORY_ROOT/scripts/generate-frontend-sbom.mjs" "$FRONTEND_OUTPUT_PATH"
test -s "$FRONTEND_OUTPUT_PATH" || die "frontend SBOM was not created"
