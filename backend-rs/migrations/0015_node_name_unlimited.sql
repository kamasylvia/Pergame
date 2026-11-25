-- Node titles were capped at 50 chars (Go-era parity with the frontend
-- maxlength and backend max tags). Long imported titles (SiYuan libraries,
-- paper titles) need room: drop the ceiling to TEXT. No btree index covers
-- nodes.name, but the search_tsv generated column references it, so it is
-- dropped and recreated around the ALTER (definition copied from 0001).
DROP INDEX IF EXISTS idx_nodes_search_fts;
ALTER TABLE nodes DROP COLUMN IF EXISTS search_tsv;
ALTER TABLE nodes ALTER COLUMN name TYPE TEXT;
ALTER TABLE nodes ADD COLUMN search_tsv TSVECTOR
    GENERATED ALWAYS AS (to_tsvector('simple',
        coalesce(name, '') || ' ' || coalesce(description, '') || ' ' || coalesce(tags, '') || ' ' || coalesce(content, '')
    )) STORED;
CREATE INDEX idx_nodes_search_fts ON nodes USING GIN (search_tsv);
