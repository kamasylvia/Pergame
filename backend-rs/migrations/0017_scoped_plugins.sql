-- Scoped plugins: regular users install their own plugins, admins install
-- global ones. `owner_id` NULL = global (visible to all), otherwise the row
-- belongs to exactly that user and is invisible to everyone else.
--
-- The slug PK becomes a surrogate: slugs repeat across scopes (your "dusk"
-- fork coexists with the global "dusk"), so grants/KV re-key to the row.
-- All existing rows are admin-installed globals; the backfill below is
-- unambiguous. Old artifact prefixes stay valid (rows keep their prefix);
-- new installs use scoped prefixes (see install_plugin).

ALTER TABLE plugins ADD COLUMN owner_id UUID NULL REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE plugins ADD COLUMN seq BIGINT GENERATED ALWAYS AS IDENTITY;

-- New keys first so the backfill has something to point at.
ALTER TABLE plugin_grants ADD COLUMN plugin_seq BIGINT NULL;
UPDATE plugin_grants g SET plugin_seq = p.seq FROM plugins p WHERE p.id = g.plugin_id AND p.owner_id IS NULL;
ALTER TABLE plugin_kv ADD COLUMN plugin_seq BIGINT NULL;
UPDATE plugin_kv k SET plugin_seq = p.seq FROM plugins p WHERE p.id = k.plugin_id AND p.owner_id IS NULL;

-- Drop dependents of the old slug PK, then the PK itself.
ALTER TABLE plugin_grants DROP CONSTRAINT plugin_grants_plugin_id_fkey;
ALTER TABLE plugin_kv DROP CONSTRAINT plugin_kv_plugin_id_fkey;
ALTER TABLE plugins DROP CONSTRAINT plugins_pkey;
ALTER TABLE plugins ADD PRIMARY KEY (seq);
CREATE UNIQUE INDEX plugins_global_slug ON plugins(id) WHERE owner_id IS NULL;
CREATE UNIQUE INDEX plugins_user_slug ON plugins(owner_id, id) WHERE owner_id IS NOT NULL;

-- Re-key grants/KV to the surrogate, with cascades.
ALTER TABLE plugin_grants ALTER COLUMN plugin_seq SET NOT NULL;
ALTER TABLE plugin_grants DROP CONSTRAINT plugin_grants_pkey;
ALTER TABLE plugin_grants ADD PRIMARY KEY (user_id, plugin_seq);
ALTER TABLE plugin_grants DROP COLUMN plugin_id;
ALTER TABLE plugin_grants ADD CONSTRAINT plugin_grants_seq_fkey
    FOREIGN KEY (plugin_seq) REFERENCES plugins(seq) ON DELETE CASCADE;

ALTER TABLE plugin_kv ALTER COLUMN plugin_seq SET NOT NULL;
ALTER TABLE plugin_kv DROP CONSTRAINT plugin_kv_pkey;
ALTER TABLE plugin_kv ADD PRIMARY KEY (user_id, plugin_seq, key);
ALTER TABLE plugin_kv DROP COLUMN plugin_id;
ALTER TABLE plugin_kv ADD CONSTRAINT plugin_kv_seq_fkey
    FOREIGN KEY (plugin_seq) REFERENCES plugins(seq) ON DELETE CASCADE;
