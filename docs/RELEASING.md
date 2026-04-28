# Releasing

The release pipeline is one workflow: `.github/workflows/release.yml`. A tag push triggers it.

## How to release

1. Bump the version in `Cargo.toml` (`[workspace.package] version = "X.Y.Z"`).
2. Commit: `git commit -am "Release vX.Y.Z"`
3. Tag: `git tag vX.Y.Z`
4. Push: `git push origin main vX.Y.Z`

The workflow builds the four binaries (darwin-arm64, darwin-x64, linux-arm64, linux-x64), signs and notarizes the macOS ones, and creates a GitHub release with auto-generated notes (`gh release create --generate-notes`).

## Versioning

Semver. Patch for bug fixes. Minor for non-breaking features. Major for breaking changes.

## Required GitHub Secrets

The workflow needs these secrets in repo settings → Secrets → Actions. They're a one-time setup:

| Secret | What it is |
|---|---|
| `MACOS_CERTIFICATE` | Base64 of your Developer ID Application `.p12` |
| `MACOS_CERTIFICATE_PWD` | Password for the `.p12` |
| `APPLE_API_KEY` | Base64 of the `.p8` notarization API key |
| `APPLE_KEY_ID` | The 10-char API key ID (from App Store Connect) |
| `APPLE_ISSUER_ID` | The issuer UUID (from App Store Connect) |

Encode the `.p12` and `.p8` with `base64 -i path/to/file -o /dev/stdout | pbcopy`.

## Release notes

Auto-generated from PR titles since the previous tag. Use clear PR titles (`fix: …`, `feat: …`, `chore: …`) so the categorization is useful.
