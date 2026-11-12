-- ==============================================================================
-- 0006_backup_targets — per-user offsite snapshot targets (spec
-- backup-restore.md §13): user-configured S3 / WebDAV stores that receive
-- byte-level replicas of v2 snapshot objects. Merged 2026-11-12 from
-- 0006+0007+0008 (pre-launch, zero live rows): users/schedule/namespace are
-- baseline columns now.
-- ==============================================================================

CREATE TABLE backup_targets (
    id            UUID PRIMARY KEY,   -- UUIDv7, application-generated
    kind          TEXT   NOT NULL,             -- s3 | webdav
    name          TEXT   NOT NULL UNIQUE,      -- display name, also job handle
    endpoint      TEXT   NOT NULL,             -- S3 endpoint / WebDAV url
    bucket        TEXT   NOT NULL DEFAULT '',  -- S3 bucket; WebDAV: ''
    prefix        TEXT   NOT NULL DEFAULT '',
    access_key    TEXT   NOT NULL DEFAULT '',  -- S3 access / WebDAV user
    secret_key    TEXT   NOT NULL DEFAULT '',  -- S3 secret / WebDAV password
    secure        BOOLEAN NOT NULL DEFAULT TRUE,
    region        TEXT   NOT NULL DEFAULT 'us-east-1',
    provider      TEXT   NOT NULL DEFAULT 'Other',
    keep_last     INT    NOT NULL DEFAULT 10,
    max_age_days  INT    NOT NULL DEFAULT 0,   -- 0 = age-based pruning off
    enabled       BOOLEAN NOT NULL DEFAULT TRUE,
    -- Owning user set: a target replicates only the listed users' snapshot
    -- objects; selecting every user == instance-level. Empty array means
    -- unassigned and the target cannot be enabled.
    users         UUID[] NOT NULL DEFAULT '{}',
    -- Standard 5-field cron; empty = manual only.
    schedule      TEXT   NOT NULL DEFAULT '',
    -- Object namespace scope (siyuan #15 §1): server-allocated ULID,
    -- immutable after creation.
    namespace     TEXT   NOT NULL DEFAULT '',
    last_result   TEXT   NOT NULL DEFAULT '',  -- last fan-out outcome for this target
    created_at    BIGINT NOT NULL,
    updated_at    BIGINT NOT NULL
);
CREATE UNIQUE INDEX idx_backup_targets_name ON backup_targets (name);
