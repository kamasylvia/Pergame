-- ==============================================================================
-- 0004_node_links_order — sort position for soft-link rows (spec
-- note-links.md §6.2 resolved: link positions take part in tree ordering like
-- regular nodes; -1 pinned reserved, mirrors nodes."order" semantics).
-- ==============================================================================

ALTER TABLE node_links ADD COLUMN "order" INT NOT NULL DEFAULT 0;
