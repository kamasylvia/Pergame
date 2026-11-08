-- ==============================================================================
-- 0003_node_links — soft links: the same note visible under multiple folders
-- (Trilium-style clone; spec .work/features/note-links.md, decision: link-table
-- approach, no content split).
-- ==============================================================================

CREATE TABLE node_links (
    id                BIGINT PRIMARY KEY,
    parent_id         BIGINT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    node_id           BIGINT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    created_timestamp BIGINT NOT NULL,
    UNIQUE (parent_id, node_id)
);
CREATE INDEX idx_node_links_node ON node_links (node_id);
