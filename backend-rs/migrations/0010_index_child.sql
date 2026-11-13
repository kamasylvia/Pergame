-- Directory index uniqueness (issue #4): at most one live index child
-- (`metadata.index = true`) per folder. Trashed indexes are excluded, so
-- delete-then-rebuild and folder restores never collide on the key.
-- The lock is enforced in the routes (name/parent/pin frozen outside the
-- settings batch path); this index only guards the one-index invariant.
CREATE UNIQUE INDEX idx_nodes_index_child
    ON nodes (parent_id)
    WHERE metadata->>'index' = 'true' AND deleted_at IS NULL;
