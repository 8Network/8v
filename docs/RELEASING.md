# Releasing 8v

Two paths exist: a local script (used today) and a tag-triggered CI workflow (available once secrets are added).

## Path 1: Local script

```sh
./scripts/release.sh X.Y.Z          # full release
./scripts/release.sh X.Y.Z --dry-run # verify without committing
```

The script: bumps `[workspace.package] version` in `Cargo.toml`, builds all 4 platform binaries, signs and notarizes macOS binaries, generates `checksums.txt`, commits and tags, pushes to `origin`, and creates the GitHub release with `--generate-notes`.

**Local prerequisites** (must be present on the machine running the script):

| Tool | Install |
|------|---------|
| `cargo-zigbuild` | `cargo install cargo-zigbuild` |
| `zig` | `brew install zig` |
| `codesign`, `xcrun` | Xcode command-line tools |
| `gh` | `brew install gh` |
| Developer ID cert | imported into keychain |
| `~/.8v/secrets/apple/notarize.env` | `APPLE_API_KEY=<path-to-.p8>`, `APPLE_KEY_ID=<id>`, `APPLE_ISSUER_ID=<uuid>` |

## Path 2: Tag-triggered CI (.github/workflows/release.yml)

Push a tag after the version-bump commit is on `main`:

```sh
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

CI builds all 4 platforms, signs and notarizes macOS, generates checksums, and publishes the GitHub release with `--generate-notes`.

**Required GitHub Secrets** (Settings → Secrets and variables → Actions → New repository secret):

| Secret | Contents |
|--------|----------|
| `MACOS_CERTIFICATE` | Base64-encoded Developer ID `.p12`: `base64 -i cert.p12` |
| `MACOS_CERTIFICATE_PWD` | Password for the `.p12` |
| `APPLE_API_KEY` | Base64-encoded App Store Connect `.p8`: `base64 -i AuthKey_*.p8` |
| `APPLE_KEY_ID` | 10-character key ID (e.g. `ABCDE12345`) |
| `APPLE_ISSUER_ID` | Issuer UUID from App Store Connect |

See [GitHub docs on encrypted secrets](https://docs.github.com/en/actions/security-guides/using-secrets-in-github-actions).

The workflow does **not** bump versions — that step always runs locally via `scripts/release.sh` or `scripts/bump-version.sh` before the tag is pushed.

## Versioning policy

Semver: `MAJOR.MINOR.PATCH`

- `PATCH` — bug fixes, security updates, no API changes
- `MINOR` — new features, backward-compatible
- `MAJOR` — breaking changes

No `v`-prefix in `Cargo.toml`; the tag carries the `v` (e.g. `v0.1.20`).

## Release notes

Notes are auto-generated from PR titles since the previous tag (`--generate-notes`). Write descriptive PR titles — they become the changelog. Format: `type: short description` (e.g. `fix: race in append`, `feat: CRLF preservation`).
