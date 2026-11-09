-- ==============================================================================
-- 0005_node_trash — soft delete (spec .work/features/trash.md). Upstream
-- Alexandrie deletes nodes physically (Go node.repository.go `DELETE FROM
-- nodes`), taking the whole subtree and its S3 objects with it; this column
-- makes deletion reversible for a retention window (TRASH_RETENTION_DAYS).
--
-- deleted_at = epoch ms, NULL = live. A subtree deleted in one action shares
-- the same timestamp so restore can bring it back as one batch.
-- ==============================================================================

ALTER TABLE nodes ADD COLUMN deleted_at BIGINT NULL;
CREATE INDEX idx_nodes_deleted ON nodes (deleted_at) WHERE deleted_at IS NOT NULL;
