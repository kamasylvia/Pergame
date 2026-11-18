-- Folder index files are removed (spec .work/features/directory-index.md,
-- archived 2026-11-18): the folder/workspace container view now renders the
-- overview tables inline, so auto-created index nodes have no purpose.
-- Existing index nodes are soft-deleted into the trash (one shared batch
-- stamp, matching the trash feature's restore semantics).
DROP INDEX IF EXISTS idx_nodes_index_child;

UPDATE nodes
SET deleted_at = (EXTRACT(EPOCH FROM NOW()) * 1000)::bigint
WHERE metadata->>'index' = 'true'
  AND deleted_at IS NULL;
