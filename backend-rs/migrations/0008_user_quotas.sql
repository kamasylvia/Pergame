-- Per-user upload quotas (NULL = unlimited, the default).
-- Read chain: users.max_*_size → config.toml [Cdn] → built-in default.
-- Admins (role=2) bypass both checks in code, regardless of these columns.

ALTER TABLE users
    ADD COLUMN max_file_size  BIGINT,
    ADD COLUMN max_total_size BIGINT;
