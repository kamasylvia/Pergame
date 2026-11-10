-- ==============================================================================
-- 0007_backup_targets_users_schedule — follow-up columns for fan-out targets
-- (spec backup-restore.md §14): the owning user set (a target replicates only
-- the listed users; selecting every user == instance-level) and the per-target
-- cron schedule ('' = manual only).
--
-- Kept separate from 0006 because that migration had already been applied to
-- live databases; editing an applied migration is rejected by sqlx.
-- ==============================================================================

ALTER TABLE backup_targets
    ADD COLUMN IF NOT EXISTS users BIGINT[] NOT NULL DEFAULT '{}';

ALTER TABLE backup_targets
    ADD COLUMN IF NOT EXISTS schedule TEXT NOT NULL DEFAULT '';
