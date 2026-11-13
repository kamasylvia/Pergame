-- Plugin registry (PS-D, spec .work/plugin-system/DESIGN.md §5).
--
-- Instance-level installs: one row per plugin slug, replaced in place when a
-- new version is uploaded. Artifacts live in the main bucket under
-- `plugins/<id>/<version>/`; the row keeps the prefix so serving an entry is a
-- single lookup. Per-user capability grants are recorded separately (the
-- admin installs, each user consents), and plugin settings get their own
-- table instead of a `user_settings` namespace: bounded rows, trivial delete.

CREATE TABLE plugins (
    id                TEXT PRIMARY KEY,          -- slug, validated in code
    name              TEXT        NOT NULL,
    version           TEXT        NOT NULL,
    api_version       TEXT        NOT NULL,
    entry             TEXT        NOT NULL,      -- relative .mjs path
    capabilities      TEXT[]      NOT NULL DEFAULT '{}',
    manifest          JSONB       NOT NULL,
    artifact_prefix   TEXT        NOT NULL,      -- plugins/<id>/<version>/
    enabled           BOOLEAN     NOT NULL DEFAULT TRUE,
    installed_by      UUID        REFERENCES users(id) ON DELETE SET NULL,
    created_timestamp BIGINT      NOT NULL,
    updated_timestamp BIGINT      NOT NULL
);

CREATE TABLE plugin_grants (
    user_id           UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    plugin_id         TEXT NOT NULL REFERENCES plugins(id) ON DELETE CASCADE,
    capabilities      TEXT[] NOT NULL DEFAULT '{}',
    updated_timestamp BIGINT NOT NULL,
    PRIMARY KEY (user_id, plugin_id)
);

CREATE TABLE plugin_kv (
    user_id           UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    plugin_id         TEXT NOT NULL REFERENCES plugins(id) ON DELETE CASCADE,
    key               TEXT NOT NULL,
    value             JSONB,
    updated_timestamp BIGINT NOT NULL,
    PRIMARY KEY (user_id, plugin_id, key)
);
