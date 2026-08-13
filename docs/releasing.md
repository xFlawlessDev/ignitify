# Releasing Ignitify

This guide covers creating, publishing, verifying, and maintaining a Linux
Ignitify binary release. It applies to the tag-driven GitHub Actions release
workflow in `.github/workflows/release.yml`.

The release process currently produces one native Linux amd64 archive. ARM64
delivery is intentionally disabled until it has been validated on native
hardware. It does not run the release installer, Docker, Compose, SSH, or
other production infrastructure effects.

## Release Flow

```text
reviewed source commit
        |
        v
version metadata commit -> annotated vX.Y.Z tag -> push tag
                                                   |
                                                   v
                              validate tag and source metadata
                                                   |
                                                   v
                                      native amd64 build
                                                   |
                                                   v
                         audit dependencies and generate an SBOM
                                                   |
                                                   v
                              production approval -> GitHub Release
```

The public `install.sh` bootstrapper resolves the default download through the
latest published GitHub Release. A selected version resolves assets from that
release's tag.

## One-Time Repository Setup

Complete these steps before creating the first automated release.

1. Merge the CI and release workflow files into the `main` branch. A tag uses
   the workflow definition contained in the tagged commit.
2. In **Settings > Actions > General**, allow workflows to use
   **Read and write permissions**. The publish job needs `contents: write` to
   create and upload GitHub Release assets.
3. Create the `production` environment under **Settings > Environments**.
   Add required reviewers when a human approval must gate publication.
4. Confirm that an `ubuntu-24.04` Linux amd64 runner is available. ARM64 is not
   currently built or distributed.
5. Protect `main` and the `v*` tag namespace so only trusted maintainers can
   publish a release.

The workflow pins each GitHub Action to a full commit SHA. Its only write token
is scoped to the publish job, after both build jobs complete.

## Choose The Version

Ignitify uses semantic versions without the leading `v` in source metadata and
with the leading `v` for Git tags and releases.

| Change                     | Example  |
| -------------------------- | -------- |
| First public release       | `v0.1.0` |
| Compatible bug fix         | `v0.1.1` |
| Compatible feature release | `v0.2.0` |
| Breaking change            | `v1.0.0` |

The release workflow rejects a tag unless all of the following agree:

- the tag is an exact semantic `vX.Y.Z` or permitted pre-release form;
- the tag points at the checked-out commit;
- `Cargo.toml` contains the tag version without `v`;
- `frontend/package.json` contains the same version.

## Prepare A Release

Finish feature work, review it, and make the worktree clean before updating the
version. Do not use `--allow-dirty` for an official build. Decide whether each
uncommitted change belongs in the release; commit or set it aside explicitly.

From the repository root, replace the example version with the selected one:

```sh
git status --short

VERSION=0.1.1
bash scripts/version.sh --set "$VERSION"
git diff --check
git add Cargo.toml Cargo.lock frontend/package.json
git commit -m "chore: prepare v$VERSION"

git tag -a "v$VERSION" -m "Ignitify v$VERSION"
git push origin main --follow-tags
```

`scripts/version.sh --set` synchronizes the workspace version, all workspace
package entries in `Cargo.lock`, and the frontend package manifest. Do not edit
those files by hand for a release.

The tag push starts the release workflow automatically. To start a workflow
again after a transient failure, use **Actions > Release > Run workflow** and
enter the already-pushed tag. The workflow checks out and validates that tag;
it does not build the branch selected in the UI.

## What The Workflow Does

The `validate` job checks the tag shape and verifies that the source metadata
matches it. The `build` job then runs on a native Linux amd64 runner. It:

1. installs Node.js 22 and pnpm `11.18.0`;
2. installs the stable Rust toolchain;
3. downloads Railpack `v0.35.0` from its official GitHub Release;
4. verifies Railpack against the reviewed SHA-256 checksum in
   `scripts/download-railpack.sh`;
5. runs `scripts/build-release.sh --require-tag`;
6. audits Rust and production frontend dependencies;
7. generates CycloneDX SBOMs for the Rust crates and embedded frontend bundle;
   and
8. uploads its archive, metadata, and SBOM as a temporary workflow artifact.

`build-release.sh` installs frontend dependencies, runs the frontend quality
gate, builds the embedded frontend, runs the Rust quality gate, builds
`ignitify-core`, and packages the native release archive.

After the build finishes, the `publish` job waits for any `production`
environment approval. It downloads the build artifact, creates `SHA256SUMS`,
verifies the archive, and creates or updates the GitHub Release with these
files:

```text
ignitify-linux-amd64.tar.gz
SHA256SUMS
release-linux-amd64.json
ignitify-rust-<crate>.cdx.json
ignitify-frontend.cdx.json
```

The first published release for a tag receives generated release notes. A
deliberate rerun updates the assets for that same tag with `--clobber`; only
rerun after confirming that the tag still points to the intended commit.

## Verify The Published Release

Before announcing a release, check the GitHub Release page for the expected
tag, the amd64 archive, `SHA256SUMS`, the amd64 metadata JSON file, and the
CycloneDX SBOM set (`ignitify-rust-<crate>.cdx.json` and
`ignitify-frontend.cdx.json`).

Download all release assets to an isolated directory and verify them:

```sh
sha256sum -c SHA256SUMS
```

The command must report every release asset as `OK`. Do not publish an archive
or SBOM that is missing from `SHA256SUMS` and do not replace the checksum file
manually without re-verifying all release assets.

The installer supports an explicit release selection:

```sh
curl -fsSL https://raw.githubusercontent.com/xFlawlessDev/ignitify/main/install.sh \
  | sh -s -- --release v0.1.1
```

Only test the installer on an authorized disposable Linux host. Never test it
against a live deployment host merely to validate a release.

## Manual Release Fallback

Use this path only when GitHub Actions is unavailable. Run the build on a
native Linux `amd64` host. The source commit must have the exact `vX.Y.Z` tag.

```sh
temporary_dir="$(mktemp -d)"
bash scripts/download-railpack.sh --output "$temporary_dir/railpack"
cd frontend
pnpm install --frozen-lockfile
pnpm audit --prod
cd ..
cargo install --locked cargo-audit --version 0.22.2
cargo audit --ignore RUSTSEC-2023-0071
cargo install --locked cargo-cyclonedx --version 0.5.9
bash scripts/build-release.sh --require-tag --skip-install --railpack "$temporary_dir/railpack"
```

Generate the SBOM and checksum file in `dist/vX.Y.Z/`:

```sh
command -v cargo-cyclonedx >/dev/null
bash scripts/generate-release-sbom.sh dist/vX.Y.Z
cd dist/vX.Y.Z
sha256sum ignitify-linux-amd64.tar.gz release-linux-amd64.json \
  ignitify-frontend.cdx.json \
  ignitify-rust-*.cdx.json > SHA256SUMS
sha256sum -c SHA256SUMS
```

Create a non-draft, non-prerelease GitHub Release using the same tag and upload
the amd64 archive, `SHA256SUMS`, `release-linux-amd64.json`, and
`ignitify-frontend.cdx.json`, plus every `ignitify-rust-<crate>.cdx.json` file.
Never upload `.env` files, runtime data, databases, generated certificates,
credentials, or source-build workspaces.

## Troubleshooting

| Symptom                                  | Cause and resolution                                                                                                                                                           |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `Resource not accessible by integration` | Enable GitHub Actions read/write workflow permissions, then rerun the release job.                                                                                             |
| Publish waits indefinitely               | A required reviewer has not approved the `production` environment.                                                                                                             |
| Release tag validation fails             | Run `scripts/version.sh --set X.Y.Z`, commit the synchronized metadata, and create a new tag on that commit. Do not retag a published release.                                 |
| ARM64 host runs the installer            | ARM64 delivery is not currently supported. The installer exits before downloading an unavailable archive.                                                                      |
| Railpack checksum mismatch               | Stop the build. Review the intended upstream Railpack release and update its version and both checksums in `scripts/download-railpack.sh` together. Never bypass verification. |
| A build or upload job failed transiently | Rerun the existing release workflow with the same pushed tag after reviewing the failure.                                                                                      |

## Maintaining The Release Toolchain

Railpack is deliberately pinned. To update it, review its upstream release,
verify the Linux amd64 archive name and SHA-256 value, then update
`RAILPACK_VERSION` and its checksum constant in
`scripts/download-railpack.sh` in one reviewed pull request. The next release
will use the new pin.

Review updates to pinned GitHub Action SHAs with the same care. Do not replace
a commit SHA with a floating action tag in a release workflow.
