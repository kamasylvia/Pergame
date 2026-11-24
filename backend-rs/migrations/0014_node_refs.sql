-- Internal-link index (spec: siyuan-auto-directory absorption): `#<uuid>`
-- references parsed from node bodies, powering outgoing references and
-- backlinks in the container directory tree.
CREATE TABLE IF NOT EXISTS node_refs (
    source_id UUID NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    target_id UUID NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    PRIMARY KEY (source_id, target_id)
);
CREATE INDEX IF NOT EXISTS idx_node_refs_target ON node_refs(target_id);

-- Backfill from existing bodies (resources excluded: their content is a
-- filename, not markdown). Targets must be live nodes; legacy `#<digits>`
-- SiYuan ids never match a Pergame UUID and fall out via the join.
INSERT INTO node_refs (source_id, target_id)
SELECT DISTINCT n.id, m.hit::uuid
FROM nodes n
CROSS JOIN LATERAL (
    SELECT (regexp_matches(n.content, '#([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})', 'g'))[1] AS hit
) m
JOIN nodes t ON t.id = m.hit::uuid AND t.deleted_at IS NULL
WHERE n.content IS NOT NULL AND n.role <> 4 AND n.deleted_at IS NULL
ON CONFLICT DO NOTHING;
