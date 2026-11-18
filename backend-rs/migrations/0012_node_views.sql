-- Recently-viewed history backing `SORT viewed_timestamp` in DQL
-- (issue #4 index template: "最近查看" block). One row per (user, node),
-- rewritten on every readable GET /nodes/:id. No data migration: history
-- starts empty, unviewed nodes simply sort last (NULLS LAST).
CREATE TABLE IF NOT EXISTS node_views (
    user_id           UUID   NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    node_id           UUID   NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    viewed_at         BIGINT NOT NULL,
    PRIMARY KEY (user_id, node_id)
);
CREATE INDEX IF NOT EXISTS idx_node_views_user_time
    ON node_views (user_id, viewed_at DESC);
