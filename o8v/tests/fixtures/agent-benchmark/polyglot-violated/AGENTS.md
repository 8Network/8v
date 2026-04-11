# Polyglot Studio

Full-stack application with multiple language components.

## Stack & Tools

- **Rust** (backend): `cargo check`, `cargo clippy -- -D warnings`, `cargo fmt --check`
- **Python** (automation): `ruff check scripts/`, `ruff format --check scripts/`
- **Go** (services): `cd services/api && go vet ./...`, `cd services/api && gofmt -l .`
- **TypeScript** (frontend): `cd frontend && npx tsc --noEmit`, `cd frontend && npx eslint .`
- **Dockerfile**: `hadolint Dockerfile`
- **Terraform** (infra): `cd infra && tflint`, `cd infra && terraform fmt -check`

## Rules
- Fix all warnings before committing
- Run ALL checks before declaring done
