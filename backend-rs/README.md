# Collectanea Backend (Rust)

Fork rewrite of the Go backend in Rust (axum + sqlx + Toasty, PostgreSQL-only).
Tracked in [issue #1](https://github.com/kamasylvia/Collectanea/issues/1); research
and decision log live in `.work/rust-migration/RESEARCH.md` (and `.work/`).

## Status

Parity ported: auth (JWT double-cookie login/refresh/logout, sessions,
TOTP 2FA with backup codes, OIDC login/link), nodes CRUD + tree + FTS search,
permissions + invitations, S3 storage (uploads/avatars/backups bootstrap +
cleanup), user settings, admin stats, backup zip jobs (v1.1.0 archive format),
account-create/password-reset email flows. PostgreSQL replaces MySQL with a
fresh baseline schema (`migrations/0001_init.sql`) — no data migration.

Known deltas vs the Go backend (see research report §7 / GOAL-STATE):
- `S3_CA_PATH` / `S3_INSECURE_TLS` are not wired into the AWS SDK TLS stack yet.
- `logout-all` clears server sessions and cookies; per-device listing exists
  via `GET /auth/sessions`.
- Object-location geolocation (`location` column) is left null pending the
  logging-domain port.

## Run

```bash
cp config.toml /path/to/config.toml   # or rely on env vars
export DATABASE_HOST=localhost DATABASE_PORT=5432 DATABASE_NAME=collectanea \
       DATABASE_USER=collectanea DATABASE_PASSWORD=... \
       JWT_SECRET=... S3_ENDPOINT=... S3_ACCESSKEY=... S3_SECRETKEY=...
cargo run        # BACKEND_PORT (default 8201), CONFIG_PATH overrides the toml
```

Environment variables mirror the Go backend (`DATABASE_*`, `JWT_SECRET`,
`COOKIE_DOMAIN`, `FRONTEND_URL`, `ALLOW_UNSECURE`, `S3_*`, `SMTP_*`,
`OIDC_1_*`..`OIDC_10_*`, `ADMIN_ACCOUNTS`, `CONFIG_DISABLE_SIGNUP`,
`CONFIG_DISABLE_NATIVE_LOGIN`).

Migrations run automatically at startup (sqlx, embedded `migrations/`).

## Checks

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

These are enforced by `.github/workflows/rust-check.yml`.

## Docker

`Dockerfile` is a two-stage build (rust builder → distroless runtime); the
compose `backend` service builds this context.
