-- ==============================================================================
-- 0007_api_tokens — per-user MCP API tokens (spec .work/features/mcp-server.md).
--
-- Tokens are opaque strings (`pgm_` + 32 random bytes base64url, 43 chars).
-- Only the SHA-256 hex of the token is stored: a database read cannot mint a
-- working credential, and the plaintext is shown exactly once at creation.
-- last_used_timestamp is refreshed on every successful MCP request (same
-- write-on-use policy as sessions.last_refresh_timestamp).
-- ==============================================================================

CREATE TABLE api_tokens (
    id                  UUID PRIMARY KEY,   -- UUIDv7, application-generated
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name                TEXT NOT NULL,
    token_hash          TEXT NOT NULL UNIQUE,  -- sha256 hex; UNIQUE gives the lookup index
    created_timestamp   BIGINT NOT NULL,
    last_used_timestamp BIGINT
);

CREATE INDEX idx_api_tokens_user ON api_tokens (user_id);
