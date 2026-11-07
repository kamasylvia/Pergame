-- ==============================================================================
-- 0001_init — PostgreSQL baseline schema (fresh-database decision 2026-11-07).
--
-- End-state union of the Go backend's MySQL migration chain (000001..000021),
-- rewritten in pg dialect. Original Snowflake ids are generated application-side
-- (backend-rs SnowflakeGenerator); no AUTO_INCREMENT. Timestamps are epoch
-- milliseconds (bigint), matching the Go models.
--
-- Full-text search (replaces MySQL FULLTEXT, research §3): generated tsvector
-- columns with the 'simple' configuration (language-neutral, CJK-safe parity
-- with the MySQL behavior) + GIN indexes.
-- ==============================================================================

-- ---------------------------------------------------------------- users
CREATE TABLE users (
    id               BIGINT PRIMARY KEY,
    username         VARCHAR(30)  NOT NULL UNIQUE,
    firstname        TEXT,
    lastname         TEXT,
    role             INT          NOT NULL DEFAULT 1,   -- 1: user, 2: admin
    type             INT          NOT NULL DEFAULT 0,   -- 0: regular, 1: supervised
    avatar           TEXT,
    email            TEXT UNIQUE,
    password         TEXT,                              -- bcrypt ($2a$/$2b$), nullable for OIDC-only accounts
    totp_secret      TEXT,
    totp_enabled     BOOLEAN      NOT NULL DEFAULT FALSE,
    totp_forced      BOOLEAN      NOT NULL DEFAULT FALSE,
    created_timestamp BIGINT      NOT NULL,
    updated_timestamp BIGINT      NOT NULL
);

-- ------------------------------------------------------- user oidc providers
CREATE TABLE user_oidc_providers (
    id                BIGINT PRIMARY KEY,
    user_id           BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider_name     TEXT   NOT NULL,
    provider_user_id  TEXT   NOT NULL,
    created_timestamp BIGINT NOT NULL,
    updated_timestamp BIGINT NOT NULL,
    UNIQUE (provider_name, provider_user_id)
);
CREATE INDEX idx_oidc_providers_user ON user_oidc_providers (user_id);

-- ------------------------------------------------------------- user settings
CREATE TABLE user_settings (
    user_id  BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    general  JSONB NOT NULL DEFAULT '{}',
    editor   JSONB NOT NULL DEFAULT '{}',
    advanced JSONB NOT NULL DEFAULT '{}'
);

-- ------------------------------------------------- totp recovery codes
CREATE TABLE user_totp_recovery_codes (
    id                BIGINT PRIMARY KEY,
    user_id           BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code              TEXT   NOT NULL,                 -- bcrypt hash of the code
    used              BOOLEAN NOT NULL DEFAULT FALSE,
    created_timestamp BIGINT NOT NULL
);
CREATE INDEX idx_recovery_codes_user ON user_totp_recovery_codes (user_id);

-- ------------------------------------------------------------------ sessions
CREATE TABLE sessions (
    id                      BIGINT PRIMARY KEY,
    user_id                 BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    refresh_token           TEXT   NOT NULL,
    expire_token            BIGINT NOT NULL,
    last_refresh_timestamp  BIGINT NOT NULL DEFAULT 0,
    active                  INT    NOT NULL DEFAULT 1,  -- 0: no, 1: yes (Go parity: int, not bool)
    ip_addr                 TEXT,
    user_agent              TEXT,
    location                TEXT,
    type                    TEXT,
    login_timestamp         BIGINT NOT NULL DEFAULT 0,
    logout_timestamp        BIGINT NOT NULL DEFAULT 0
);
CREATE INDEX idx_sessions_user ON sessions (user_id);
CREATE INDEX idx_sessions_refresh_token ON sessions (refresh_token);
CREATE INDEX idx_sessions_ip ON sessions (ip_addr);

-- --------------------------------------------------------------------- nodes
-- role: 1 workspace, 2 category, 3 document, 4 file/resource.
CREATE TABLE nodes (
    id                BIGINT PRIMARY KEY,
    user_id           BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    parent_id         BIGINT REFERENCES nodes(id) ON DELETE CASCADE,
    name              VARCHAR(50) NOT NULL,
    description       VARCHAR(250),
    tags              VARCHAR(250),
    role              INT   NOT NULL DEFAULT 3,
    color             INT,
    icon              TEXT,
    thumbnail         TEXT,
    theme             TEXT,
    accessibility     INT,                 -- 0 public, 1 private, 2 unlisted
    access            INT   NOT NULL DEFAULT 0,  -- bit field: 0 restricted, 1 view, 2 edit
    display           INT,
    "order"           INT,                 -- -1 pinned, -2 bookmark (reserved word, quoted)
    content           TEXT,                -- markdown source
    content_compiled  TEXT,                -- rendered HTML (frontend-produced)
    size              BIGINT,
    metadata          JSONB,
    created_timestamp BIGINT NOT NULL,
    updated_timestamp BIGINT NOT NULL
);
CREATE INDEX idx_nodes_user ON nodes (user_id);
CREATE INDEX idx_nodes_parent ON nodes (parent_id);
CREATE INDEX idx_nodes_user_role ON nodes (user_id, role);
CREATE INDEX idx_nodes_metadata ON nodes USING GIN (metadata jsonb_path_ops);

-- Full-text search generated columns + GIN (replaces MySQL FULLTEXT 000013).
ALTER TABLE nodes ADD COLUMN content_tsv  TSVECTOR
    GENERATED ALWAYS AS (to_tsvector('simple', coalesce(content, ''))) STORED;
ALTER TABLE nodes ADD COLUMN search_tsv   TSVECTOR
    GENERATED ALWAYS AS (to_tsvector('simple',
        coalesce(name, '') || ' ' || coalesce(description, '') || ' ' || coalesce(tags, '') || ' ' || coalesce(content, '')
    )) STORED;
CREATE INDEX idx_nodes_content_fts  ON nodes USING GIN (content_tsv);
CREATE INDEX idx_nodes_search_fts   ON nodes USING GIN (search_tsv);

-- --------------------------------------------------------------- permissions
CREATE TABLE permissions (
    id                BIGINT PRIMARY KEY,
    node_id           BIGINT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    user_id           BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permission        INT    NOT NULL,   -- 1 view, 2 write, 3 admin
    created_timestamp BIGINT NOT NULL,
    UNIQUE (node_id, user_id)
);
CREATE INDEX idx_permissions_node ON permissions (node_id);
CREATE INDEX idx_permissions_user ON permissions (user_id);

-- ---------------------------------------------------------- node invitations
CREATE TABLE node_invitations (
    id                BIGINT PRIMARY KEY,
    invitation_code   TEXT   NOT NULL UNIQUE,
    permission_level  INT    NOT NULL CHECK (permission_level BETWEEN 1 AND 3),
    node_id           BIGINT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    created_timestamp BIGINT NOT NULL
);
CREATE INDEX idx_invitations_node ON node_invitations (node_id);

-- ---------------------------------------------------------------------- logs
-- P2 note: align field set with backend/repositories/log.repository.go when
-- the logging domain is ported.
CREATE TABLE logs (
    id                BIGINT PRIMARY KEY,
    user_id           BIGINT REFERENCES users(id) ON DELETE SET NULL,
    action            TEXT NOT NULL,
    message           TEXT,
    ip_addr           TEXT,
    created_timestamp BIGINT NOT NULL
);
CREATE INDEX idx_logs_user ON logs (user_id);
CREATE INDEX idx_logs_created ON logs (created_timestamp);
