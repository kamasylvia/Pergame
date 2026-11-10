-- ==============================================================================
-- 0008_backup_target_namespace — object namespace scoping for backup targets
-- (siyuan issue #15 §1 parity).
--
-- Several instances may share one physical store. Each keeps its snapshot
-- lineage, claim marker and retention under `namespace/` inside the target
-- prefix, so two instances can run cron against the same bucket without
-- touching each other's manifests.
--
-- Empty = default scope (compatible with objects written before this
-- migration). Immutable after creation: changing it orphans every existing
-- snapshot for that target, so the API rejects the edit.
-- ==============================================================================

ALTER TABLE backup_targets ADD COLUMN namespace TEXT NOT NULL DEFAULT '';
