#!/usr/bin/env bash
# Derives release versions and deliberately synchronizes source metadata.
set -Eeuo pipefail
IFS=$'\n\t'

readonly REPOSITORY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly WORKSPACE_MANIFEST="$REPOSITORY_ROOT/Cargo.toml"
readonly LOCKFILE="$REPOSITORY_ROOT/Cargo.lock"
readonly FRONTEND_MANIFEST="$REPOSITORY_ROOT/frontend/package.json"

EXPLICIT_VERSION=""
FROM_GIT=1
WRITE_VERSION=0
EXACT_TAG_ONLY=0
OUTPUT_TAG=0

usage() {
  cat <<'EOF'
Usage: scripts/version.sh [options]

Prints the effective Ignitify version. An exact vX.Y.Z (or X.Y.Z) Git tag wins;
otherwise a deterministic development snapshot is generated from the workspace
version, commit count, and short commit SHA.

Options:
  --from-git          Derive from Git (default).
  --exact-tag         Require an exact semantic-version Git tag at HEAD.
  --set VERSION       Validate and write VERSION to root Cargo.toml and frontend/package.json.
  --write             Write the version derived by --from-git to those manifests.
  --tag               Print the release tag form, for example v0.1.0.
  -h, --help          Show this help text.

Examples:
  scripts/version.sh
  scripts/version.sh --exact-tag
  scripts/version.sh --set 0.2.0
  scripts/version.sh --from-git --write
EOF
}

die() {
  printf '%s\n' "[ignitify-version] error: $*" >&2
  exit 1
}

normalize_version() {
  local version="${1#v}"
  [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z-]+([.][0-9A-Za-z-]+)*)?$ ]] \
    || return 1
  printf '%s' "$version"
}

workspace_version() {
  local version
  version="$(awk '
    /^\[workspace\.package\]$/ { in_package = 1; next }
    in_package && /^\[/ { exit }
    in_package && /^version[[:space:]]*=/ {
      value = $0
      sub(/^[^"]*"/, "", value)
      sub(/".*$/, "", value)
      print value
      exit
    }
  ' "$WORKSPACE_MANIFEST")"
  normalize_version "$version" || die "workspace package version is invalid"
}

exact_git_tag_version() {
  command -v git >/dev/null 2>&1 || return 1
  git -C "$REPOSITORY_ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1 || return 1

  local tag version
  while IFS= read -r tag; do
    if version="$(normalize_version "$tag")"; then
      printf '%s' "$version"
      return 0
    fi
  done < <(git -C "$REPOSITORY_ROOT" tag --points-at HEAD --sort=-v:refname)
  return 1
}

derived_git_version() {
  local exact base commit_count commit_sha
  if exact="$(exact_git_tag_version)"; then
    printf '%s' "$exact"
    return 0
  fi

  base="$(workspace_version)"
  base="${base%%-*}"
  if command -v git >/dev/null 2>&1 \
    && git -C "$REPOSITORY_ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    commit_count="$(git -C "$REPOSITORY_ROOT" rev-list --count HEAD)"
    commit_sha="$(git -C "$REPOSITORY_ROOT" rev-parse --short=12 HEAD)"
    printf '%s-dev.%s.g%s' "$base" "$commit_count" "$commit_sha"
  else
    printf '%s-dev.0.local' "$base"
  fi
}

workspace_package_names() {
  local manifest package_name package_names=""
  for manifest in "$REPOSITORY_ROOT"/crates/*/Cargo.toml; do
    package_name="$(awk '
      /^\[package\]$/ { in_package = 1; next }
      in_package && /^\[/ { exit }
      in_package && /^name[[:space:]]*=/ {
        value = $0
        sub(/^[^"]*"/, "", value)
        sub(/".*$/, "", value)
        print value
        exit
      }
    ' "$manifest")"
    [[ -n "$package_name" ]] || die "could not read the package name from $manifest"
    package_names+="${package_names:+|}$package_name"
  done
  printf '%s' "$package_names"
}

write_workspace_version() {
  local version="$1"
  local temporary_manifest temporary_lockfile temporary_frontend_manifest workspace_packages
  [[ -f "$LOCKFILE" ]] || die "Cargo.lock is missing"
  temporary_manifest="$(mktemp "$REPOSITORY_ROOT/.Cargo.toml.version.XXXXXX")"
  temporary_lockfile=""
  temporary_frontend_manifest=""
  trap 'rm -f "${temporary_manifest:-}" "${temporary_lockfile:-}" "${temporary_frontend_manifest:-}"' EXIT

  awk -v version="$version" '
    /^\[workspace\.package\]$/ { in_package = 1; print; next }
    in_package && /^\[/ { in_package = 0 }
    in_package && /^version[[:space:]]*=/ {
      print "version = \"" version "\""
      changed = 1
      next
    }
    { print }
    END { if (!changed) exit 2 }
  ' "$WORKSPACE_MANIFEST" > "$temporary_manifest" \
    || die "could not prepare the workspace version update"

  workspace_packages="$(workspace_package_names)"
  temporary_lockfile="$(mktemp "$REPOSITORY_ROOT/.Cargo.lock.version.XXXXXX")"
  awk -v version="$version" -v workspace_packages="$workspace_packages" '
    BEGIN {
      package_count = split(workspace_packages, package_list, "|")
      for (package_index = 1; package_index <= package_count; package_index++) {
        workspace_package[package_list[package_index]] = 1
      }
    }
    /^\[\[package\]\]$/ { current_package = ""; print; next }
    /^name[[:space:]]*=/ {
      current_package = $0
      sub(/^[^"]*"/, "", current_package)
      sub(/".*$/, "", current_package)
      print
      next
    }
    current_package in workspace_package && /^version[[:space:]]*=/ {
      print "version = \"" version "\""
      updated[current_package] = 1
      next
    }
    { print }
    END {
      for (package_index = 1; package_index <= package_count; package_index++) {
        if (!updated[package_list[package_index]]) exit 2
      }
    }
  ' "$LOCKFILE" > "$temporary_lockfile" \
    || die "could not prepare the Cargo.lock version update"

  temporary_frontend_manifest="$(mktemp "$REPOSITORY_ROOT/frontend/.package.json.version.XXXXXX")"
  node - "$FRONTEND_MANIFEST" "$temporary_frontend_manifest" "$version" <<'NODE'
const fs = require("node:fs");
const [source, destination, version] = process.argv.slice(2);
const manifest = JSON.parse(fs.readFileSync(source, "utf8"));
if (typeof manifest.version !== "string") {
  throw new Error("frontend package manifest has no version field");
}
manifest.version = version;
fs.writeFileSync(destination, `${JSON.stringify(manifest, null, 2)}\n`);
NODE

  mv "$temporary_manifest" "$WORKSPACE_MANIFEST"
  mv "$temporary_lockfile" "$LOCKFILE"
  mv "$temporary_frontend_manifest" "$FRONTEND_MANIFEST"
  trap - EXIT
  printf '%s\n' "[ignitify-version] synchronized source metadata and Cargo.lock to $version" >&2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --from-git)
      FROM_GIT=1
      shift
      ;;
    --exact-tag)
      EXACT_TAG_ONLY=1
      shift
      ;;
    --set)
      [[ $# -ge 2 ]] || die "--set requires a version"
      EXPLICIT_VERSION="$(normalize_version "$2")" || die "version must be semantic version text such as 0.2.0 or 0.2.0-rc.1"
      FROM_GIT=0
      WRITE_VERSION=1
      shift 2
      ;;
    --write)
      WRITE_VERSION=1
      shift
      ;;
    --tag)
      OUTPUT_TAG=1
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

if [[ "$EXACT_TAG_ONLY" -eq 1 && "$FROM_GIT" -eq 0 ]]; then
  die "--exact-tag cannot be combined with --set"
fi

if [[ -n "$EXPLICIT_VERSION" ]]; then
  VERSION="$EXPLICIT_VERSION"
elif [[ "$EXACT_TAG_ONLY" -eq 1 ]]; then
  VERSION="$(exact_git_tag_version)" || die "HEAD does not have an exact semantic-version Git tag"
else
  VERSION="$(derived_git_version)"
fi

if [[ "$WRITE_VERSION" -eq 1 ]]; then
  command -v node >/dev/null 2>&1 || die "node is required to update frontend/package.json"
  write_workspace_version "$VERSION"
fi

if [[ "$OUTPUT_TAG" -eq 1 ]]; then
  printf 'v%s\n' "$VERSION"
else
  printf '%s\n' "$VERSION"
fi
