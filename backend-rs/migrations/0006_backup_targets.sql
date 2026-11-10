-- ==============================================================================
-- 0005_backup_targets — instance DR fan-out targets (spec backup-restore.md
-- §13): admin-configured offsite stores (S3 / WebDAV / external PG) that
-- receive byte-level replicas of every user's v2 snapshot objects.
-- ==============================================================================

CREATE TABLE backup_targets (
    id            UUID PRIMARY KEY,
    kind          TEXT   NOT NULL,             -- s3 | webdav | pg
    name          TEXT   NOT NULL UNIQUE,      -- display name, also job handle
    endpoint      TEXT   NOT NULL,             -- S3 endpoint / WebDAV url / PG dsn-ish host part
    bucket        TEXT   NOT NULL DEFAULT '',  -- S3 bucket; PG: schema name; WebDAV: ''
    prefix        TEXT   NOT NULL DEFAULT '',
    access_key    TEXT   NOT NULL DEFAULT '',  -- S3 access / WebDAV user / PG user
    secret_key    TEXT   NOT NULL DEFAULT '',  -- S3 secret / WebDAV password / PG password
    secure        BOOLEAN NOT NULL DEFAULT TRUE,
    region        TEXT   NOT NULL DEFAULT 'us-east-1',
    provider      TEXT   NOT NULL DEFAULT 'Other',
    keep_last     INT    NOT NULL DEFAULT 10,
    max_age_days  INT    NOT NULL DEFAULT 0,   -- 0 = age-based pruning off
    enabled       BOOLEAN NOT NULL DEFAULT TRUE,
    -- Owning user set (decision 2026-11-10): a target replicates only the
    -- listed users' snapshot objects; selecting every user == instance-level.
    -- Empty array means unassigned and the target cannot be enabled.
    users         BIGINT[] NOT NULL DEFAULT '{}',
    last_result   TEXT   NOT NULL DEFAULT '',  -- last fan-out outcome for this target
    created_at    BIGINT NOT NULL,
    updated_at    BIGINT NOT NULL
);
CREATE UNIQUE INDEX idx_backup_targets_name ON backup_targets (name);
