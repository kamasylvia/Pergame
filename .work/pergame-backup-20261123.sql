--
-- PostgreSQL database dump
--

\restrict b6CxPpDa4f9ZCtfYdEekVchm4evJbeH1tIkyheyIkCEzxamdtJmZxWJNYfOcjIq

-- Dumped from database version 18.2 (Debian 18.2-1.pgdg12+1)
-- Dumped by pg_dump version 18.2 (Debian 18.2-1.pgdg12+1)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: _sqlx_migrations; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public._sqlx_migrations (
    version bigint NOT NULL,
    description text NOT NULL,
    installed_on timestamp with time zone DEFAULT now() NOT NULL,
    success boolean NOT NULL,
    checksum bytea NOT NULL,
    execution_time bigint NOT NULL
);


ALTER TABLE public._sqlx_migrations OWNER TO postgres;

--
-- Name: api_tokens; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.api_tokens (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    name text NOT NULL,
    token_hash text NOT NULL,
    created_timestamp bigint NOT NULL,
    last_used_timestamp bigint
);


ALTER TABLE public.api_tokens OWNER TO postgres;

--
-- Name: backup_targets; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.backup_targets (
    id uuid NOT NULL,
    kind text NOT NULL,
    name text NOT NULL,
    endpoint text NOT NULL,
    bucket text DEFAULT ''::text NOT NULL,
    prefix text DEFAULT ''::text NOT NULL,
    access_key text DEFAULT ''::text NOT NULL,
    secret_key text DEFAULT ''::text NOT NULL,
    secure boolean DEFAULT true NOT NULL,
    region text DEFAULT 'us-east-1'::text NOT NULL,
    provider text DEFAULT 'Other'::text NOT NULL,
    keep_last integer DEFAULT 10 NOT NULL,
    max_age_days integer DEFAULT 0 NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    users uuid[] DEFAULT '{}'::uuid[] NOT NULL,
    schedule text DEFAULT ''::text NOT NULL,
    namespace text DEFAULT ''::text NOT NULL,
    last_result text DEFAULT ''::text NOT NULL,
    created_at bigint NOT NULL,
    updated_at bigint NOT NULL
);


ALTER TABLE public.backup_targets OWNER TO postgres;

--
-- Name: logs; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.logs (
    id uuid NOT NULL,
    user_id uuid,
    action text NOT NULL,
    message text,
    ip_addr text,
    created_timestamp bigint NOT NULL
);


ALTER TABLE public.logs OWNER TO postgres;

--
-- Name: node_invitations; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.node_invitations (
    id uuid NOT NULL,
    invitation_code text NOT NULL,
    permission_level integer NOT NULL,
    node_id uuid NOT NULL,
    created_timestamp bigint NOT NULL,
    CONSTRAINT node_invitations_permission_level_check CHECK (((permission_level >= 1) AND (permission_level <= 3)))
);


ALTER TABLE public.node_invitations OWNER TO postgres;

--
-- Name: node_links; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.node_links (
    id uuid NOT NULL,
    parent_id uuid NOT NULL,
    node_id uuid NOT NULL,
    created_timestamp bigint NOT NULL,
    "order" integer DEFAULT 0 NOT NULL
);


ALTER TABLE public.node_links OWNER TO postgres;

--
-- Name: node_views; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.node_views (
    user_id uuid NOT NULL,
    node_id uuid NOT NULL,
    viewed_at bigint NOT NULL
);


ALTER TABLE public.node_views OWNER TO postgres;

--
-- Name: nodes; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.nodes (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    parent_id uuid,
    name character varying(50) NOT NULL,
    description character varying(250),
    tags character varying(250),
    role integer DEFAULT 3 NOT NULL,
    color integer,
    icon text,
    thumbnail text,
    theme text,
    accessibility integer,
    access integer DEFAULT 0 NOT NULL,
    display integer,
    "order" integer,
    content text,
    content_compiled text,
    size bigint,
    metadata jsonb,
    created_timestamp bigint NOT NULL,
    updated_timestamp bigint NOT NULL,
    content_tsv tsvector GENERATED ALWAYS AS (to_tsvector('simple'::regconfig, COALESCE(content, ''::text))) STORED,
    search_tsv tsvector GENERATED ALWAYS AS (to_tsvector('simple'::regconfig, (((((((COALESCE(name, ''::character varying))::text || ' '::text) || (COALESCE(description, ''::character varying))::text) || ' '::text) || (COALESCE(tags, ''::character varying))::text) || ' '::text) || COALESCE(content, ''::text)))) STORED,
    deleted_at bigint
);


ALTER TABLE public.nodes OWNER TO postgres;

--
-- Name: object_blobs; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.object_blobs (
    sha256 character(64) NOT NULL,
    size bigint NOT NULL,
    refcount integer DEFAULT 1 NOT NULL,
    created_timestamp bigint NOT NULL
);


ALTER TABLE public.object_blobs OWNER TO postgres;

--
-- Name: permissions; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.permissions (
    id uuid NOT NULL,
    node_id uuid NOT NULL,
    user_id uuid NOT NULL,
    permission integer NOT NULL,
    created_timestamp bigint NOT NULL
);


ALTER TABLE public.permissions OWNER TO postgres;

--
-- Name: plugin_grants; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.plugin_grants (
    user_id uuid NOT NULL,
    plugin_id text NOT NULL,
    capabilities text[] DEFAULT '{}'::text[] NOT NULL,
    updated_timestamp bigint NOT NULL
);


ALTER TABLE public.plugin_grants OWNER TO postgres;

--
-- Name: plugin_kv; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.plugin_kv (
    user_id uuid NOT NULL,
    plugin_id text NOT NULL,
    key text NOT NULL,
    value jsonb,
    updated_timestamp bigint NOT NULL
);


ALTER TABLE public.plugin_kv OWNER TO postgres;

--
-- Name: plugins; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.plugins (
    id text NOT NULL,
    name text NOT NULL,
    version text NOT NULL,
    api_version text NOT NULL,
    entry text NOT NULL,
    capabilities text[] DEFAULT '{}'::text[] NOT NULL,
    manifest jsonb NOT NULL,
    artifact_prefix text NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    installed_by uuid,
    created_timestamp bigint NOT NULL,
    updated_timestamp bigint NOT NULL
);


ALTER TABLE public.plugins OWNER TO postgres;

--
-- Name: sessions; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.sessions (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    refresh_token text NOT NULL,
    expire_token bigint NOT NULL,
    last_refresh_timestamp bigint DEFAULT 0 NOT NULL,
    active integer DEFAULT 1 NOT NULL,
    ip_addr text,
    user_agent text,
    location text,
    type text,
    login_timestamp bigint DEFAULT 0 NOT NULL,
    logout_timestamp bigint DEFAULT 0 NOT NULL
);


ALTER TABLE public.sessions OWNER TO postgres;

--
-- Name: user_oidc_providers; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_oidc_providers (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    provider_name text NOT NULL,
    provider_user_id text NOT NULL,
    created_timestamp bigint NOT NULL,
    updated_timestamp bigint NOT NULL
);


ALTER TABLE public.user_oidc_providers OWNER TO postgres;

--
-- Name: user_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_settings (
    user_id uuid NOT NULL,
    general jsonb DEFAULT '{}'::jsonb NOT NULL,
    editor jsonb DEFAULT '{}'::jsonb NOT NULL,
    advanced jsonb DEFAULT '{}'::jsonb NOT NULL
);


ALTER TABLE public.user_settings OWNER TO postgres;

--
-- Name: user_totp_recovery_codes; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_totp_recovery_codes (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    code text NOT NULL,
    used boolean DEFAULT false NOT NULL,
    created_timestamp bigint NOT NULL
);


ALTER TABLE public.user_totp_recovery_codes OWNER TO postgres;

--
-- Name: users; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.users (
    id uuid NOT NULL,
    username character varying(30) NOT NULL,
    firstname text,
    lastname text,
    role integer DEFAULT 1 NOT NULL,
    type integer DEFAULT 0 NOT NULL,
    avatar text,
    email text,
    password text,
    totp_secret text,
    totp_enabled boolean DEFAULT false NOT NULL,
    totp_forced boolean DEFAULT false NOT NULL,
    created_timestamp bigint NOT NULL,
    updated_timestamp bigint NOT NULL,
    password_reset_token text,
    max_file_size bigint,
    max_total_size bigint
);


ALTER TABLE public.users OWNER TO postgres;

--
-- Data for Name: _sqlx_migrations; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public._sqlx_migrations (version, description, installed_on, success, checksum, execution_time) FROM stdin;
1	init	2026-11-12 13:31:43.402187+08	t	\\x2101e488aadc7cc9aa1f9f32473340cf08a45080f98b6a8f976490301c99c92cfd348be90500a34d80f75a8c2d33f15f	323576049
2	password reset token	2026-11-12 13:31:43.75506+08	t	\\xda39a6538f1194db56c3653cc6e86a0b2e7d455e0a72e96d2e57c5e6b4d3dd5f0b277d327a20b47b15df97903847fd1e	41690836
3	node links	2026-11-12 13:31:43.84888+08	t	\\xc6a88e7047e41ba44b62ab5882067781982fbf255f83098d806cc6a2d5d9c9498fd973852e4179cc16b19a48746f0727	33249319
4	node links order	2026-11-12 13:31:43.943185+08	t	\\x2bf4876870be92a5676bf50464bc569854f24e980c625d5eac46c2ffbebc9116a8b81abfc8d22203624baeefa7b697f8	20766938
5	node trash	2026-11-12 13:31:43.975627+08	t	\\xd366c20d2d3b30f1ae53696499483ae747018facd6f913198a4201ea7183005f3f8b3d199bdd65c88d547213993a78d2	10173670
6	backup targets	2026-11-12 13:31:44.008548+08	t	\\x8f152a14796104255399401c78f9bd11a6f07ccfe32ae84b2e2ce342bb4843cfa0e32f68f1efecab650ef74e4ce1ad7b	53051609
7	api tokens	2026-11-12 21:40:40.727662+08	t	\\xafe491f21597af4e4e78991193db165eee2671447cf9dc76f2cbb8c7e6df4be85f3e994396dc72e9ef706f2091955e7c	244843209
8	user quotas	2026-11-13 11:20:20.523793+08	t	\\xc62de841335d177d1515dc284df7abfecf3d0d8ff49ae260c4701fc9e8569d6d7bb29a33d9ff712a83de04fa61bd9358	119353166
9	object blobs	2026-11-13 11:20:20.667782+08	t	\\x17d0a5a79da13f5ecc8f71bbce0edc512512c2bab975e1315fec3c2fe973514db0e0944a330aed1e008ced0988a453b0	238279500
10	index child	2026-11-13 18:39:17.794517+08	t	\\x458a4cf892f23b937ad4c8a3060d4af6854655edaee0fb31a7052f7aebed99b5c494716f502024279d400f30dc005a80	354858208
11	plugins	2026-11-14 02:23:25.556493+08	t	\\xde525cfa28468e2577e063c239e572fbe4d85cfb222202ebe3470ca27d02084be2351cef49c1e208b63b87aae1dbe0a4	823457209
12	node views	2026-11-16 11:38:13.195027+08	t	\\x884c2a8ae8f60442ee6e5c6651a1bed5e307c3c27d2f9ac45d1572e45720a646a900b1e85e3eefcd62fa5d62fb00484c	791534875
13	drop index files	2026-11-21 03:37:07.35698+08	t	\\xcc85b41a8fa5f30e1d47a3ff6ae5c2a945ab2bf9cfcda2452e4ce0df2a1c2b79e3d713cdd6c9e6232dd5bd574512da9a	3498485963
\.


--
-- Data for Name: api_tokens; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.api_tokens (id, user_id, name, token_hash, created_timestamp, last_used_timestamp) FROM stdin;
7900a42b-36a5-4c5f-9492-b450942d13d7	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp-transfer-session	c4e86410856a18af47dcaaae53a796cf44f8ab3e7d63e555a45a5e4f394a7ad2	1794721640584	1794723781919
01a1d014-ff67-7695-b697-11a9e6ad9e74	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp-2026-11-12	d83619a0796bbf6b50eedd850b2fe83f933413ea5ae385bb8dc3eb0a3d982e75	1794492399463	\N
01a1d01a-a933-756a-9cb2-efb66b447275	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp-2026-11-12	e31430ab6e90647dc5330fb0ea7a1a7f595f0688fc1742438312dca585d0c15e	1794492770611	1794966275720
\.


--
-- Data for Name: backup_targets; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.backup_targets (id, kind, name, endpoint, bucket, prefix, access_key, secret_key, secure, region, provider, keep_last, max_age_days, enabled, users, schedule, namespace, last_result, created_at, updated_at) FROM stdin;
\.


--
-- Data for Name: logs; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.logs (id, user_id, action, message, ip_addr, created_timestamp) FROM stdin;
01a1d301-6d9f-73bb-87c1-c915f8d236d1	\N	mcp.tool.create_note	created node 01a1d301-6d8b-70cf-b6eb-9ffe6d362ab8 (token 01a1d301-305b-702b-a153-e090047d41a7)	\N	1794541448608
01a1d00d-a4de-74d6-86b4-a0d5369e49df	\N	mcp.token.create	created api token cli-check	\N	1794491917534
01a1d00f-4705-77e4-a6e6-70bf1b25ed64	\N	mcp.token.create	created api token mcp-2026-11-12	\N	1794492024581
01a1d015-005a-7631-9ca6-c1c91b85059c	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.token.create	created api token mcp-2026-11-12	\N	1794492399706
01a1d018-70cb-72ce-82b2-39885e72367a	\N	mcp.token.create	created api token mcp-2026-11-12	\N	1794492625099
01a1d01a-a952-74c8-997d-0850de6ae4a2	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.token.create	created api token mcp-2026-11-12	\N	1794492770642
01a1d01c-ddfd-7226-8daa-104e49c655bc	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1d01c-dda4-70a6-96c7-a66a836cdaf8 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794492915197
01a1d01c-fff9-753a-8b1b-964d0b3b97c7	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.update_note	updated node 01a1d01c-dda4-70a6-96c7-a66a836cdaf8 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794492923897
01a1d01d-19dc-72f9-9928-0edbf47698d6	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1d01d-19c7-715a-902d-5ab8abaa7146 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794492930524
01a1d01d-1b08-740b-8f82-4275126551e4	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.move_node	moved node 01a1d01d-19c7-715a-902d-5ab8abaa7146 to 01a1d01c-dda4-70a6-96c7-a66a836cdaf8 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794492930824
01a1d01d-1bcb-753b-8eee-e9b7f5c724e3	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.delete_node	trashed node 01a1d01c-dda4-70a6-96c7-a66a836cdaf8 (2 nodes, token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794492931019
01a1d027-089c-70e7-90b5-061874e30e7c	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1d027-0890-755c-ad9c-b6c0a6c77c39 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794493581468
01a1d027-7c5c-745f-b858-11bc73a584bd	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1d027-7c3d-71e3-a719-47bdc7d7d823 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794493611100
01a1d029-28fa-74fd-846d-668ff9c63695	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1d029-28e2-77a3-9a17-38667d4852e2 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794493720826
01a1d029-eb53-73fe-b01d-49a0c2656807	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1d029-eb3f-7130-b7b2-eb44711ff6d2 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794493770579
01a1d02a-06e2-7524-8630-b2b7e36138f6	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1d02a-067e-74dd-b9c9-65b14f9876fd (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794493777634
01a1d02a-1be3-75f7-a40f-463dfe30e5d9	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1d02a-1bd5-71c1-9f60-06380f37c75b (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794493783011
01a1d02b-8c39-71f9-be4b-b496ef7e43cf	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1d02b-8c2e-7154-8d0f-8a43d1d13857 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794493877305
01a1d02b-9f5b-73f5-a7b4-e1754a7c5bbf	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.update_note	updated node 01a1d02b-8c2e-7154-8d0f-8a43d1d13857 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794493882203
01a1d02b-aef5-77ac-992e-4edd40c3469c	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1d02b-aed8-74bb-925b-50c9bd4d6881 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794493886197
01a1d02b-c2ef-7435-a27a-da7087556b06	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.update_note	updated node 01a1d02b-aed8-74bb-925b-50c9bd4d6881 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794493891311
01a1d02b-d0ca-7416-8e21-b93cf579e425	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1d02b-d0b2-7357-b726-5fcc0eeb5506 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794493894858
01a1d02b-df3d-701f-80da-279457d37494	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.update_note	updated node 01a1d02b-d0b2-7357-b726-5fcc0eeb5506 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794493898557
01a1d033-49a7-75bb-8430-a9f57c99dd1b	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1d033-494e-75ec-8564-b4f3bd9b4742 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794494384551
01a1d033-59c7-7048-903e-af5917845e75	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.update_note	updated node 01a1d033-494e-75ec-8564-b4f3bd9b4742 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794494388679
01a1d033-e82f-725c-acc5-95945716887d	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1d033-e81d-749a-9b64-09d8ff47ca7b (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794494425135
01a1d033-fa4a-75a1-ae21-769bf18a51a6	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.update_note	updated node 01a1d033-e81d-749a-9b64-09d8ff47ca7b (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794494429770
01a1d48b-88a6-7218-9e1e-fb584ad75e7e	\N	mcp.token.create	created api token integration	\N	1794567276710
01a1d48b-8992-7421-b821-a11adf26f089	\N	mcp.tool.create_note	created node 01a1d48b-8981-77af-a2bd-a7a578d58876 (token 01a1d48b-8873-741f-9d7e-360b0668dc3c)	\N	1794567276946
01a1d48b-9216-71cd-8ed8-839ed4b974f4	\N	mcp.token.revoke	revoked api token 01a1d48b-8873-741f-9d7e-360b0668dc3c	\N	1794567279126
01a1d2a4-6663-76f2-aa07-7fed36a34b2e	\N	mcp.token.create	created api token integration	\N	1794535351907
01a1d2a4-6794-727a-b133-a2f5f44fbbce	\N	mcp.tool.create_note	created node 01a1d2a4-677e-70bf-9d07-4a2d23f3e853 (token 01a1d2a4-6644-7533-867b-242c7eec3fc4)	\N	1794535352212
01a1d2a4-6849-776f-837f-dff0d049e2e1	\N	mcp.tool.update_note	updated node 01a1d2a4-677e-70bf-9d07-4a2d23f3e853 (token 01a1d2a4-6644-7533-867b-242c7eec3fc4)	\N	1794535352393
01a1d2a4-69ba-7220-87b8-abff9acfcefc	\N	mcp.tool.create_note	created node 01a1d2a4-69a5-758d-b5ed-f83813cefd3d (token 01a1d2a4-6644-7533-867b-242c7eec3fc4)	\N	1794535352762
01a1d2a4-6a59-7643-8e6b-e93ad2a18f2f	\N	mcp.tool.move_node	moved node 01a1d2a4-69a5-758d-b5ed-f83813cefd3d to 01a1d2a4-677e-70bf-9d07-4a2d23f3e853 (token 01a1d2a4-6644-7533-867b-242c7eec3fc4)	\N	1794535352921
01a1d2a4-7297-728c-9dc9-300afb416499	\N	mcp.tool.delete_node	trashed node 01a1d2a4-677e-70bf-9d07-4a2d23f3e853 (2 nodes, token 01a1d2a4-6644-7533-867b-242c7eec3fc4)	\N	1794535355031
01a1d2a4-7164-74ac-9b63-d9ead711f449	\N	mcp.token.create	created api token stranger	\N	1794535354724
01a1d48b-98fa-728b-b2ac-048160fbe67f	\N	mcp.token.create	created api token lock-window	\N	1794567280890
01a1d48b-dba9-77f8-801f-1579b47a6e94	\N	mcp.token.revoke	revoked api token 01a1d48b-98ea-75b7-82f5-266fa9a9fcc9	\N	1794567297961
01a1d632-d066-77f0-ad1e-e2a6e683d11f	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.grant	plugin md-demo granted: markdown:render,ui	\N	1794595016806
01a1d2a4-7c18-750c-8bd5-477d942adb06	\N	mcp.token.create	created api token lock-window	\N	1794535357464
01a1d2a4-7cc6-72fe-a669-e6a07df0fb96	\N	mcp.tool.create_note	created node 01a1d2a4-7caf-7395-890b-7643f61b919a (token 01a1d2a4-7bfb-77f6-88ff-0cd70573b364)	\N	1794535357638
01a1d2a4-b96f-7548-be08-ba22a88ecbdb	\N	mcp.tool.create_note	created node 01a1d2a4-b952-723c-aed1-aac5baeea3a7 (token 01a1d2a4-7bfb-77f6-88ff-0cd70573b364)	\N	1794535373167
01a1d643-4df8-772a-b2dd-77bca286ee26	\N	plugin.grant	plugin it-demo-01a1d643-4126-70fc-9335-228bd4713a83 granted: ui	\N	1794596097528
01a1cffd-b5c6-769e-afd3-7eac70055709	\N	mcp.tool.create_note	created node 01a1cffd-b506-721c-a01b-9e0f4ae366cb (token 6ab61c87-19ce-4ae6-87d2-c652740f29df)	\N	1794490873286
01a1d2a4-72f0-7168-889f-770ebd473c0b	\N	mcp.token.revoke	revoked api token 01a1d2a4-6644-7533-867b-242c7eec3fc4	\N	1794535355121
01a1d2a4-b9b5-71ad-b5f0-a079e1239b1f	\N	mcp.token.revoke	revoked api token 01a1d2a4-7bfb-77f6-88ff-0cd70573b364	\N	1794535373237
01a1d301-6ddc-70da-a43d-12b989be42d0	\N	mcp.token.revoke	revoked api token 01a1d301-305b-702b-a153-e090047d41a7	\N	1794541448668
01a1d48b-90e6-77a8-8b9e-2f1bd9ccece1	\N	mcp.token.create	created api token stranger	\N	1794567278822
01a1d2ad-432a-7619-86c3-8ab25fbd418c	\N	mcp.token.create	created api token integration	\N	1794535932714
01a1d2ad-447e-70cf-9e65-347aa1aebdd2	\N	mcp.tool.create_note	created node 01a1d2ad-446c-7580-ab4c-bf3333966fdc (token 01a1d2ad-430d-74ec-8cd4-83c6d118e5b2)	\N	1794535933054
01a1d2ad-4525-7136-96c2-77f79f8185ad	\N	mcp.tool.update_note	updated node 01a1d2ad-446c-7580-ab4c-bf3333966fdc (token 01a1d2ad-430d-74ec-8cd4-83c6d118e5b2)	\N	1794535933221
01a1d2ad-4679-7703-a3ed-139baf92b360	\N	mcp.tool.create_note	created node 01a1d2ad-4664-7495-a615-a33f65442dae (token 01a1d2ad-430d-74ec-8cd4-83c6d118e5b2)	\N	1794535933561
01a1d2ad-4713-718d-9330-be5b472704c3	\N	mcp.tool.move_node	moved node 01a1d2ad-4664-7495-a615-a33f65442dae to 01a1d2ad-446c-7580-ab4c-bf3333966fdc (token 01a1d2ad-430d-74ec-8cd4-83c6d118e5b2)	\N	1794535933715
01a1d2ad-4f6e-73f8-a865-c866e997025d	\N	mcp.tool.delete_node	trashed node 01a1d2ad-446c-7580-ab4c-bf3333966fdc (2 nodes, token 01a1d2ad-430d-74ec-8cd4-83c6d118e5b2)	\N	1794535935854
01a1d2ad-4fc1-751d-b36a-4875b2378d3c	\N	mcp.token.revoke	revoked api token 01a1d2ad-430d-74ec-8cd4-83c6d118e5b2	\N	1794535935937
01a1d2ad-4e30-76d7-a1cc-2012c8643dd5	\N	mcp.token.create	created api token stranger	\N	1794535935536
01a1d2ad-5805-7448-a805-d7f7638a8420	\N	mcp.token.create	created api token lock-window	\N	1794535938053
01a1d2ad-589b-71e9-a0d6-dc1e4a8d0a6f	\N	mcp.tool.create_note	created node 01a1d2ad-5887-74e7-b094-a1932e74b97f (token 01a1d2ad-57e9-73ab-90de-661b09c8824e)	\N	1794535938203
01a1d2ad-9d42-723f-abe1-f4e852905a60	\N	mcp.tool.create_note	created node 01a1d2ad-9d24-701b-b2f9-e93bd0953fb2 (token 01a1d2ad-57e9-73ab-90de-661b09c8824e)	\N	1794535955778
01a1d2ad-9d7c-7788-9624-ad093e88d553	\N	mcp.token.revoke	revoked api token 01a1d2ad-57e9-73ab-90de-661b09c8824e	\N	1794535955836
01a1d2b6-0582-7412-af9d-182653946ac6	\N	mcp.token.create	created api token integration	\N	1794536506754
01a1d2b6-0d6a-75e6-9345-eac4a504c62b	\N	mcp.tool.create_note	created node 01a1d2b6-0d54-773f-bb46-c2ac1e91e532 (token 01a1d2b6-055b-77e8-b485-99d8575bb633)	\N	1794536508778
01a1d2b6-0e21-77ab-8d40-600d07eef0c0	\N	mcp.tool.update_note	updated node 01a1d2b6-0d54-773f-bb46-c2ac1e91e532 (token 01a1d2b6-055b-77e8-b485-99d8575bb633)	\N	1794536508961
01a1d2b6-13d4-7281-94fc-8c9a08a6b9a3	\N	mcp.tool.create_note	created node 01a1d2b6-13bd-75b2-872c-1a89d1bfe454 (token 01a1d2b6-055b-77e8-b485-99d8575bb633)	\N	1794536510420
01a1d2b6-1472-74d4-8c03-8b42a73a4811	\N	mcp.tool.move_node	moved node 01a1d2b6-13bd-75b2-872c-1a89d1bfe454 to 01a1d2b6-0d54-773f-bb46-c2ac1e91e532 (token 01a1d2b6-055b-77e8-b485-99d8575bb633)	\N	1794536510578
01a1d2b6-201c-775d-bd0e-b618efcd1f5c	\N	mcp.tool.delete_node	trashed node 01a1d2b6-0d54-773f-bb46-c2ac1e91e532 (2 nodes, token 01a1d2b6-055b-77e8-b485-99d8575bb633)	\N	1794536513564
01a1d2b6-20aa-7468-963d-d48e7211808a	\N	mcp.token.revoke	revoked api token 01a1d2b6-055b-77e8-b485-99d8575bb633	\N	1794536513706
01a1d2b6-1bbb-72b6-af9e-25528b3880d5	\N	mcp.token.create	created api token stranger	\N	1794536512443
01a1d2b6-2b7b-7567-b279-b08eb50a3da2	\N	mcp.token.create	created api token lock-window	\N	1794536516475
01a1d2b6-2c42-77ca-a820-04767754770b	\N	mcp.tool.create_note	created node 01a1d2b6-2c1e-7248-9d74-bfc2ab1b4d87 (token 01a1d2b6-2b5d-745c-b3e1-537c6d51ad30)	\N	1794536516674
01a1d2b6-7791-7497-897b-eadf32b06d88	\N	mcp.tool.create_note	created node 01a1d2b6-776f-74ad-8051-5899bec95fd7 (token 01a1d2b6-2b5d-745c-b3e1-537c6d51ad30)	\N	1794536535953
01a1d2b6-77d8-772a-afb1-29b11436bfd8	\N	mcp.token.revoke	revoked api token 01a1d2b6-2b5d-745c-b3e1-537c6d51ad30	\N	1794536536024
01a1d2b8-a319-74e9-bce8-7743b9dd9d18	\N	mcp.token.create	created api token integration	\N	1794536678169
01a1d2b8-a4ab-7232-98ef-da5eb4b2e55d	\N	mcp.tool.create_note	created node 01a1d2b8-a48c-75d4-8640-883921509692 (token 01a1d2b8-a2f0-7109-974d-0e4b5c82afe2)	\N	1794536678571
01a1d2b8-a568-719a-b302-96e4548faeff	\N	mcp.tool.update_note	updated node 01a1d2b8-a48c-75d4-8640-883921509692 (token 01a1d2b8-a2f0-7109-974d-0e4b5c82afe2)	\N	1794536678760
01a1d2b8-a6c5-76bb-be5a-b862bf702025	\N	mcp.tool.create_note	created node 01a1d2b8-a6b1-729e-8faf-5d1c6fcf16e1 (token 01a1d2b8-a2f0-7109-974d-0e4b5c82afe2)	\N	1794536679109
01a1d2b8-a754-76a1-9cf0-a5a248816b2e	\N	mcp.tool.move_node	moved node 01a1d2b8-a6b1-729e-8faf-5d1c6fcf16e1 to 01a1d2b8-a48c-75d4-8640-883921509692 (token 01a1d2b8-a2f0-7109-974d-0e4b5c82afe2)	\N	1794536679252
01a1d2b8-b0bb-76be-992c-7af7e9600d5d	\N	mcp.tool.delete_node	trashed node 01a1d2b8-a48c-75d4-8640-883921509692 (2 nodes, token 01a1d2b8-a2f0-7109-974d-0e4b5c82afe2)	\N	1794536681659
01a1d2b8-b104-7718-b197-7a8073614095	\N	mcp.token.revoke	revoked api token 01a1d2b8-a2f0-7109-974d-0e4b5c82afe2	\N	1794536681732
01a1d2b8-aebc-760b-963a-c93da6e7e4eb	\N	mcp.token.create	created api token stranger	\N	1794536681148
01a1d2b8-ba18-7348-8532-c5bd1462ae46	\N	mcp.token.create	created api token lock-window	\N	1794536684056
01a1d2b8-bbbb-726f-9f81-d32c25e251e0	\N	mcp.tool.create_note	created node 01a1d2b8-bb9d-7692-86ac-70571a286852 (token 01a1d2b8-b9f4-7346-b98d-dd1657b1ecaf)	\N	1794536684475
01a1d2b8-f8e2-73b0-9612-0e1a3d3070a2	\N	mcp.tool.create_note	created node 01a1d2b8-f8c5-7693-b9d6-4824d5f6e147 (token 01a1d2b8-b9f4-7346-b98d-dd1657b1ecaf)	\N	1794536700130
01a1d2b8-f933-74e4-9439-37832a577401	\N	mcp.token.revoke	revoked api token 01a1d2b8-b9f4-7346-b98d-dd1657b1ecaf	\N	1794536700211
01a1d2fb-1b5e-7167-97f7-401011d5c5bf	\N	mcp.token.create	created api token integration	\N	1794541034334
01a1d2fb-1d16-7731-9a34-f6949e1181bc	\N	mcp.tool.create_note	created node 01a1d2fb-1d01-71e1-b22b-c7ba69d2807c (token 01a1d2fb-1b37-777a-b205-9f2a2a5af4be)	\N	1794541034774
01a1d2fb-1de8-72f9-84e7-abd640167555	\N	mcp.tool.update_note	updated node 01a1d2fb-1d01-71e1-b22b-c7ba69d2807c (token 01a1d2fb-1b37-777a-b205-9f2a2a5af4be)	\N	1794541034984
01a1d2fb-1f52-73aa-b31e-1d191d722e98	\N	mcp.tool.create_note	created node 01a1d2fb-1f3b-70b0-830b-cab3b88fc2ec (token 01a1d2fb-1b37-777a-b205-9f2a2a5af4be)	\N	1794541035346
01a1d2fb-1ff9-7368-b8e6-e6cdf02c4594	\N	mcp.tool.move_node	moved node 01a1d2fb-1f3b-70b0-830b-cab3b88fc2ec to 01a1d2fb-1d01-71e1-b22b-c7ba69d2807c (token 01a1d2fb-1b37-777a-b205-9f2a2a5af4be)	\N	1794541035513
01a1d2fb-2918-7459-b4a0-2ec11bb48053	\N	mcp.tool.delete_node	trashed node 01a1d2fb-1d01-71e1-b22b-c7ba69d2807c (2 nodes, token 01a1d2fb-1b37-777a-b205-9f2a2a5af4be)	\N	1794541037848
01a1d2fb-274b-7374-b8b7-d678771dda93	\N	mcp.token.create	created api token stranger	\N	1794541037387
01a1d2fb-73e6-755a-aa72-108e258f62d0	\N	mcp.tool.create_note	created node 01a1d2fb-73cf-77fc-873f-ec7c365bfde3 (token 01a1d2fb-3208-7226-a539-799b3f3e3b47)	\N	1794541056998
01a1d2fb-741d-74ec-9ab7-84e07a5e6b7f	\N	mcp.token.revoke	revoked api token 01a1d2fb-3208-7226-a539-799b3f3e3b47	\N	1794541057053
01a1d2fb-2962-72c2-8b5a-6d6201f413f3	\N	mcp.token.revoke	revoked api token 01a1d2fb-1b37-777a-b205-9f2a2a5af4be	\N	1794541037922
01a1d301-3072-7664-9f2b-91ae03483e73	\N	mcp.token.create	created api token lock-window	\N	1794541432946
01a1d301-310e-751f-b7fd-1553954a2d0d	\N	mcp.tool.create_note	created node 01a1d301-30fa-749f-b522-ddc1cfc48aaa (token 01a1d301-305b-702b-a153-e090047d41a7)	\N	1794541433102
01a1d2fb-3227-7024-a4ab-6309cc7f4d51	\N	mcp.token.create	created api token lock-window	\N	1794541040167
01a1d2fb-32e4-72e7-b2e9-78b766f631e1	\N	mcp.tool.create_note	created node 01a1d2fb-32c8-701c-910b-6e1648543c8d (token 01a1d2fb-3208-7226-a539-799b3f3e3b47)	\N	1794541040356
01a1d48b-8a10-7321-ae8c-cbfe4f2164d6	\N	mcp.tool.update_note	updated node 01a1d48b-8981-77af-a2bd-a7a578d58876 (token 01a1d48b-8873-741f-9d7e-360b0668dc3c)	\N	1794567277072
01a1d48b-8b38-7482-89ad-9d5519ee754e	\N	mcp.tool.move_node	moved node 01a1d48b-8ab7-741d-bb7f-53eaef700229 to 01a1d48b-8981-77af-a2bd-a7a578d58876 (token 01a1d48b-8873-741f-9d7e-360b0668dc3c)	\N	1794567277368
01a1d48b-91d5-7519-825b-5170de65f21f	\N	mcp.tool.delete_node	trashed node 01a1d48b-8981-77af-a2bd-a7a578d58876 (2 nodes, token 01a1d48b-8873-741f-9d7e-360b0668dc3c)	\N	1794567279062
01a1d48b-8ac5-72e3-a9b9-2693c74f123a	\N	mcp.tool.create_note	created node 01a1d48b-8ab7-741d-bb7f-53eaef700229 (token 01a1d48b-8873-741f-9d7e-360b0668dc3c)	\N	1794567277253
01a1d48b-db4b-738a-a7ca-16b179e1c876	\N	mcp.tool.create_note	created node 01a1d48b-db26-702d-8bb2-972c87a62531 (token 01a1d48b-98ea-75b7-82f5-266fa9a9fcc9)	\N	1794567297867
01a1d48b-996b-77ea-8661-6b52348b7326	\N	mcp.tool.create_note	created node 01a1d48b-995b-7284-b8f9-a7ddd3d618b1 (token 01a1d48b-98ea-75b7-82f5-266fa9a9fcc9)	\N	1794567281003
01a1d301-19fe-7424-8d9d-03a0a88512da	\N	mcp.token.create	created api token integration	\N	1794541427198
01a1d301-1b46-70b1-9211-13c0de039d8d	\N	mcp.tool.create_note	created node 01a1d301-1b2e-71a2-8367-2a683f8ab305 (token 01a1d301-19e1-776f-8c69-8196bde8a074)	\N	1794541427526
01a1d301-1c11-76c2-a04c-4503332ef1eb	\N	mcp.tool.update_note	updated node 01a1d301-1b2e-71a2-8367-2a683f8ab305 (token 01a1d301-19e1-776f-8c69-8196bde8a074)	\N	1794541427729
01a1d301-1d92-7651-b31b-55ee6864effd	\N	mcp.tool.create_note	created node 01a1d301-1d78-76fb-8ac9-ea2cf564c684 (token 01a1d301-19e1-776f-8c69-8196bde8a074)	\N	1794541428114
01a1d301-1e30-752e-8324-f31384f12f98	\N	mcp.tool.move_node	moved node 01a1d301-1d78-76fb-8ac9-ea2cf564c684 to 01a1d301-1b2e-71a2-8367-2a683f8ab305 (token 01a1d301-19e1-776f-8c69-8196bde8a074)	\N	1794541428272
01a1d301-26fe-73fa-a845-968f63efa994	\N	mcp.tool.delete_node	trashed node 01a1d301-1b2e-71a2-8367-2a683f8ab305 (2 nodes, token 01a1d301-19e1-776f-8c69-8196bde8a074)	\N	1794541430526
01a1d301-2785-730c-888a-74dffa5e70b7	\N	mcp.token.revoke	revoked api token 01a1d301-19e1-776f-8c69-8196bde8a074	\N	1794541430661
01a1d301-25cd-7785-bf6b-8468816a20ab	\N	mcp.token.create	created api token stranger	\N	1794541430221
01a1d631-16a1-749f-b442-8dab129a15c4	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.grant	plugin e2e-demo granted: nodes:read,ui	\N	1794594903713
01a1d631-3794-77ad-9e60-ba6db00d9d53	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.grant	plugin web-clipper granted: nodes:read,nodes:write,ui,net:*	\N	1794594912148
01a1d631-3c61-73ee-9ca4-8af9b9f5960f	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.proxy	plugin web-clipper proxied example.com (200, 577 bytes)	\N	1794594913378
01a1d632-6a17-77f0-8877-1c0c94dd48df	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.grant	plugin e2e-demo granted: nodes:read,ui	\N	1794594990615
01a1d631-1546-75cb-9123-bb8d4f5043f6	\N	plugin.install	installed e2e-demo 0.1.0 (2 files, caps: nodes:read,ui)	\N	1794594903366
01a1d631-372b-74a0-988f-8130a861d0e7	\N	plugin.install	installed web-clipper 0.1.0 (2 files, caps: nodes:read,nodes:write,ui,net:*)	\N	1794594912043
01a1d632-cff6-7345-b680-e7fd6f9fe6dc	\N	plugin.install	installed md-demo 0.1.0 (2 files, caps: markdown:render,ui)	\N	1794595016694
01a1d636-7ce7-7715-91c1-0fe312808509	\N	plugin.uninstall	uninstalled plugin e2e-demo	\N	1794595257575
01a1d636-7d6a-76a1-b07d-f2fc0b764bf4	\N	plugin.uninstall	uninstalled plugin web-clipper	\N	1794595257706
01a1d636-7dd6-708e-8af7-b87e13874342	\N	plugin.uninstall	uninstalled plugin md-demo	\N	1794595257814
01a1d643-4cf1-74d2-a965-91c3cccc7519	\N	plugin.install	installed it-demo-01a1d643-4126-70fc-9335-228bd4713a83 0.1.0 (2 files, caps: ui,net:example.com)	\N	1794596097265
01a1d643-4f7f-7761-aeba-73b49ba5e181	\N	plugin.enable	plugin it-demo-01a1d643-4126-70fc-9335-228bd4713a83 enabled=false	\N	1794596097919
01a1d643-5049-7031-ae03-4043ff3f0ced	\N	plugin.uninstall	uninstalled plugin it-demo-01a1d643-4126-70fc-9335-228bd4713a83	\N	1794596098121
01a1d6a1-7c44-734e-8b62-ceb589b4aeb7	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.grant	plugin read-it-later granted: nodes:read,nodes:write,ui,settings,net:*	\N	1794602269764
01a1d6a2-0abb-7401-a52a-a7c9e57538e5	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.grant	plugin read-it-later granted: nodes:read,nodes:write,ui,settings,net:*	\N	1794602306235
01a1d6a5-919e-70e2-86e1-26867393019c	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.grant	plugin read-it-later granted: nodes:read,nodes:write,ui,settings,net:*	\N	1794602537374
01a1d6a6-e852-72bf-84fd-56b87313d28f	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.grant	plugin read-it-later granted: nodes:read,nodes:write,ui,settings,net:*	\N	1794602625106
01a1d6a7-6026-73c0-99ed-f610ae45ea7b	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.grant	plugin read-it-later granted: nodes:read,nodes:write,ui,settings,net:*	\N	1794602655782
01a1d6a7-7f30-7580-9ef9-be164457c6d7	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.proxy	plugin read-it-later proxied example.com (200, 577 bytes)	\N	1794602663728
01a1d6a0-b128-72fb-8674-09e67ce1784f	\N	plugin.install	installed read-it-later 0.2.0 (2 files, caps: nodes:read,nodes:write,ui,settings,net:*)	\N	1794602217768
01a1d6a5-77ee-75b6-aa7c-094597b2a791	\N	plugin.install	installed read-it-later 0.2.0 (2 files, caps: nodes:read,nodes:write,ui,settings,net:*)	\N	1794602530798
01a1d6a7-47cf-7355-85f3-90142058a006	\N	plugin.install	installed read-it-later 0.2.0 (2 files, caps: nodes:read,nodes:write,ui,settings,net:*)	\N	1794602649551
01a1d6a8-4f0d-703c-8c46-57f061faf934	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.proxy	plugin read-it-later proxied www.zhihu.com (403, 694 bytes)	\N	1794602716941
01a1d6a9-aa1b-75a9-b839-230e48f233ee	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.proxy	plugin read-it-later proxied binary httpbin.org (8090 bytes)	\N	1794602805787
01a1d6ae-225e-729a-81b5-d7d655ccc62f	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.proxy	plugin read-it-later proxied en.wikipedia.org (403, 126 bytes)	\N	1794603098718
01a1d6ae-25f6-716d-899c-63181a36aa7e	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.proxy	plugin read-it-later proxied en.wikipedia.org (403, 126 bytes)	\N	1794603099638
01a1d6ae-5318-7776-9ea4-136504aa7bc4	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.proxy	plugin read-it-later proxied www.zhihu.com (403, 650 bytes)	\N	1794603111192
01a1d6ae-9f25-7014-99e4-b2a85286a64f	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.proxy	plugin read-it-later proxied en.wikipedia.org (403, 126 bytes)	\N	1794603130661
01a1d6b0-df2d-75ba-b54d-9917edbc6e4c	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.proxy	plugin read-it-later proxied en.wikipedia.org (200, 179374 bytes)	\N	1794603278125
01a1d6b1-1383-71c3-8c70-9b78e2afa002	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.proxy	plugin read-it-later proxied en.wikipedia.org (200, 1859 bytes)	\N	1794603291523
01a1d6b1-4692-72b9-9f6c-23c67a3f298d	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.proxy	plugin read-it-later proxied www.zhihu.com (403, 650 bytes)	\N	1794603304594
01a1d6b1-dbdd-7268-a9e4-9c6b303e5b43	\N	plugin.uninstall	uninstalled plugin read-it-later	\N	1794603342813
01a1d6b8-20c9-7314-a318-32941d220086	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.grant	plugin read-it-later granted: nodes:read,nodes:write,ui,settings,net:*	\N	1794603753673
01a1d6b8-3fd0-708c-9954-fa8f57a0d9b0	01a1ce57-dceb-77b1-8122-68e49e40e2c2	plugin.proxy	plugin read-it-later proxied example.com (200, 577 bytes)	\N	1794603761616
01a1d6c0-d30f-74c0-babb-9642b57bf9d2	\N	plugin.install	installed it-demo-01a1d6c0-c34d-7358-b665-a4d4e844967e 0.1.0 (2 files, caps: ui,net:example.com)	\N	1794604323599
01a1d6c0-d3b4-7303-91de-43edaeb2d819	\N	plugin.grant	plugin it-demo-01a1d6c0-c34d-7358-b665-a4d4e844967e granted: ui	\N	1794604323764
01a1d6c0-d494-73fe-86b1-be2e24700a86	\N	plugin.enable	plugin it-demo-01a1d6c0-c34d-7358-b665-a4d4e844967e enabled=false	\N	1794604323988
01a1d6c0-d516-74a7-95a2-863ddb7a8a3f	\N	plugin.uninstall	uninstalled plugin it-demo-01a1d6c0-c34d-7358-b665-a4d4e844967e	\N	1794604324118
01a1d7a6-ce34-76be-911c-89adb0357757	\N	plugin.install	installed web-clipper 0.2.0 (2 files, caps: nodes:read,nodes:write,ui,settings,net:*)	\N	1794619395636
01a1d7a6-f589-7406-8503-9e65b6821d4c	\N	plugin.uninstall	uninstalled plugin web-clipper	\N	1794619405705
01a1cffd-b66a-7746-a9af-50865762eaf6	\N	mcp.tool.create_note	created node 01a1cffd-b65f-71e9-8d46-8a3c93f596ec (token 6ab61c87-19ce-4ae6-87d2-c652740f29df)	\N	1794490873450
01a1cffd-b6c6-7125-ab15-f8143a929ca2	\N	mcp.tool.update_note	updated node 01a1cffd-b65f-71e9-8d46-8a3c93f596ec (token 6ab61c87-19ce-4ae6-87d2-c652740f29df)	\N	1794490873542
01a1cffd-b72f-76ad-9b94-5a3491c0732e	\N	mcp.tool.delete_node	trashed node 01a1cffd-b65f-71e9-8d46-8a3c93f596ec (1 nodes, token 6ab61c87-19ce-4ae6-87d2-c652740f29df)	\N	1794490873647
01a1dddf-5fce-7113-bc60-22141edcfde1	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1dddf-5d33-7138-8a35-291084a90501 (token 7900a42b-36a5-4c5f-9492-b450942d13d7)	\N	1794723766222
01a1e23f-b4ec-73af-a289-3469ac5f5907	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1e23f-b26d-7373-9eeb-00c9e99a0368 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794797188332
01a1e23f-b702-711c-b472-5a3148448d31	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1e23f-b6bb-76bf-8a63-82e916ad55e4 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794797188866
01a1e23f-b86f-7051-aca3-93f1028853b9	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1e23f-b83b-7385-87a5-f3221168c5f8 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794797189231
01a1e23f-ba45-75e3-84c9-7c908eeef9d2	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1e23f-ba01-759e-b5e6-f2f55431138a (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794797189701
01a1e23f-bbb5-7286-936e-5c478b1bcd95	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1e23f-bb71-7272-83d8-dd80515be4c5 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794797190069
01a1e23f-bd6a-741a-815a-f5059c45a42e	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.create_note	created node 01a1e23f-bd27-7695-8361-fea4f5276117 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794797190506
01a1e25a-50ff-7537-a6c2-3b4eb3420192	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.move_node	moved node 01a1e23f-bb71-7272-83d8-dd80515be4c5 to 01a1e253-c5da-77e2-8724-f1ba8679486e (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794798932223
01a1e25a-54e7-712a-addd-40d2ea129ce5	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.move_node	moved node 01a1e23f-bd27-7695-8361-fea4f5276117 to 01a1e253-c5da-77e2-8724-f1ba8679486e (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794798933223
01a1e25a-5957-70a1-a610-09e7b118f90b	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.delete_node	trashed node 01a1e23f-b26d-7373-9eeb-00c9e99a0368 (4 nodes, token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794798934359
01a1e25a-65d2-742c-9e9b-8450576e27e5	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.delete_node	trashed node 01a1e253-8819-7120-a32d-3a3e01fd6cf5 (1 nodes, token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794798937554
01a1e27b-9068-7384-95da-65e40a37bb4a	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.delete_node	trashed node 01a1e26f-788d-778c-8fe7-686b71a1ef2c (2 nodes, token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794801111144
01a1e27b-92bc-74a8-8b31-108d4af836a6	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.delete_node	trashed node 01a1e26f-b740-7045-af15-4944157f556f (2 nodes, token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794801111740
01a1e27b-9503-72e8-bafd-0e5bd007f6ee	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.delete_node	trashed node 01a1e270-108e-72ff-840c-656306ca0c4e (8 nodes, token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794801112323
01a1e27b-98f3-75c4-ba49-207cee646237	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.delete_node	trashed node 01a1e274-c676-76df-a2de-fe0b98febc5d (8 nodes, token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794801113331
01a1e27b-9a39-726d-a260-f08af5ea0957	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.delete_node	trashed node 01a1e277-fd90-7755-b21f-b70400c88a22 (4 nodes, token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794801113657
01a1e27b-9ba3-7598-b1cf-40c94fa5b063	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.delete_node	trashed node 01a1e279-7ff2-7317-bce4-6d7abb37a05a (4 nodes, token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794801114019
01a1e27b-e239-7466-aa19-b9717c7d7b8d	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.delete_node	trashed node 01a1e26f-db2c-73ce-809c-abdaed4866dd (2 nodes, token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794801132089
01a1e27d-c13f-76bc-b8d7-c8dbf11df25c	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.update_note	updated node 01a1e261-f1ab-7181-857e-3c59d8390447 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794801254719
01a1e27d-d36f-7538-af2b-f34ee9a200b9	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.update_note	updated node 01a1e278-f4c9-73b4-b522-24582d2093d2 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794801259375
01a1e27d-e342-7416-924a-11b0088f5887	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.update_note	updated node 01a1e278-f8cc-7284-9d81-1bef05f50d6a (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794801263426
01a1e283-f051-7637-a883-298265963fcc	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.update_note	updated node 01a1e256-a143-735b-823d-6023bbee7a57 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794801659985
01a1e27d-f0a3-76f3-b324-07ed8c5d9116	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.update_note	updated node 01a1e256-a143-735b-823d-6023bbee7a57 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794801266851
01a1e283-bf09-763e-a4b6-6905d608dc27	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.update_note	updated node 01a1e261-f1ab-7181-857e-3c59d8390447 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794801647370
01a1e283-cf66-7559-a2bd-1c094bfc2019	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.update_note	updated node 01a1e278-f4c9-73b4-b522-24582d2093d2 (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794801651558
01a1e283-de15-73bf-b510-5a85cf3d7ede	01a1ce57-dceb-77b1-8122-68e49e40e2c2	mcp.tool.update_note	updated node 01a1e278-f8cc-7284-9d81-1bef05f50d6a (token 01a1d01a-a933-756a-9cb2-efb66b447275)	\N	1794801655317
\.


--
-- Data for Name: node_invitations; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.node_invitations (id, invitation_code, permission_level, node_id, created_timestamp) FROM stdin;
\.


--
-- Data for Name: node_links; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.node_links (id, parent_id, node_id, created_timestamp, "order") FROM stdin;
\.


--
-- Data for Name: node_views; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.node_views (user_id, node_id, viewed_at) FROM stdin;
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e270-108e-72ff-840c-656306ca0c4e	1794800358141
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e270-1595-7188-855c-5c88f0debcac	1794800358982
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e270-13b2-76e1-bc4b-bfb4fbe20c05	1794800359094
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e270-140d-7712-b2dc-a8cd9b4f279c	1794800359207
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e274-c676-76df-a2de-fe0b98febc5d	1794800666545
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e274-ca4e-73c9-960d-dbd75017caf3	1794800667447
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e274-c868-73f9-b905-7e3b2202f214	1794800667582
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e274-c8f4-734b-8149-82943f779f77	1794800667695
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e274-c73d-70c1-a5ca-fec05dcb8da7	1794800873701
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e277-fd90-7755-b21f-b70400c88a22	1794800877384
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e278-00f1-72b8-b562-5f086143b050	1794800877944
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e278-007c-7631-aa08-79b111b5a20b	1794800878065
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e253-c299-7080-9577-727389e5fb72	1794800933158
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e253-c3cf-7296-889e-69f645b5aa06	1794800941038
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e279-7ff2-7317-bce4-6d7abb37a05a	1794800976205
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e279-82cd-7684-88b0-d6a4122dbac4	1794800976811
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e279-80a4-738c-93a8-1cb388f7f743	1794801082121
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d033-e81d-749a-9b64-09d8ff47ca7b	1794801544628
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e278-f8cc-7284-9d81-1bef05f50d6a	1794808829771
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e278-f4c9-73b4-b522-24582d2093d2	1794808833910
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e23f-bb71-7272-83d8-dd80515be4c5	1794808857480
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e23f-bd27-7695-8361-fea4f5276117	1794808860640
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-d0b2-7357-b726-5fcc0eeb5506	1794808879041
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e256-a143-735b-823d-6023bbee7a57	1794959009175
01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e261-f1ab-7181-857e-3c59d8390447	1794962015798
\.


--
-- Data for Name: nodes; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.nodes (id, user_id, parent_id, name, description, tags, role, color, icon, thumbnail, theme, accessibility, access, display, "order", content, content_compiled, size, metadata, created_timestamp, updated_timestamp, deleted_at) FROM stdin;
01a1e253-9b66-7506-8e6a-f678a4b343a7	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	perm-probe	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794798492518	1794798492518	1794798501368
01a1e253-c299-7080-9577-727389e5fb72	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e253-bf21-720a-90f7-5c50d5115eae	2026	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794798502553	1794798502553	\N
01a1e23f-bd27-7695-8361-fea4f5276117	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e253-c5da-77e2-8724-f1ba8679486e	2026-11-02 09:30:15 <2026-11-02 09:30:15>	\N	\N	3	\N	\N	\N	\N	\N	0	\N	\N	![D3410EAB-2E7A-4CFD-A4AE-745472FBD87D.jpeg](assets/memos/resources/hr27r3oDZSbAWVBFKkrpnk_1793582949.jpeg)	\N	\N	\N	1794797190439	1794798933086	\N
01a1e23f-b26d-7373-9eeb-00c9e99a0368	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	daily note	\N	\N	3	\N	\N	\N	\N	\N	0	\N	\N	‍	\N	\N	\N	1794797187693	1794797187693	1794798933999
01a1e26f-7d02-7377-8957-3c9b987cba73	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e26f-788d-778c-8fe7-686b71a1ef2c	index	\N	\N	3	\N	\N	\N	\N	1	1	\N	-1	## 子目录\n\n```dataview\nLIST FROM "id:01a1e26f-788d-778c-8fe7-686b71a1ef2c" DIRECT\n```\n\n## 最近查看\n\n```dataview\nLIST FROM "id:01a1e26f-788d-778c-8fe7-686b71a1ef2c" WHERE role = 3 SORT viewed_timestamp DESC LIMIT 10\n```\n		226	{"index": true}	1794800319747	1794800319747	1794801111048
01a1e26f-b8bf-770b-a492-362a6fdf8b16	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e26f-b740-7045-af15-4944157f556f	index	\N	\N	3	\N	\N	\N	\N	1	1	\N	-1	## 子目录\n\n```dataview\nLIST FROM "id:01a1e26f-b740-7045-af15-4944157f556f" DIRECT\n```\n\n## 最近查看\n\n```dataview\nLIST FROM "id:01a1e26f-b740-7045-af15-4944157f556f" WHERE role = 3 SORT viewed_timestamp DESC LIMIT 10\n```\n		226	{"index": true}	1794800335039	1794800335039	1794801111636
01a1e270-14ab-758e-9fa3-f0e55ce3c0bf	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e270-108e-72ff-840c-656306ca0c4e	IdxSub	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794800358571	1794800358571	1794801112246
01a1e270-15e9-7132-858e-75341f14b968	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e270-108e-72ff-840c-656306ca0c4e	IdxUnv	\N	\N	3	\N			\N	\N	0	\N	\N	\N		0	\N	1794800358889	1794800358889	1794801112246
01a1e270-1595-7188-855c-5c88f0debcac	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e270-14ab-758e-9fa3-f0e55ce3c0bf	IdxC	\N	\N	3	\N			\N	\N	0	\N	\N	\N		0	\N	1794800358805	1794800358805	1794801112246
01a1e274-c94f-747b-b66c-a6271f85557e	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e274-c676-76df-a2de-fe0b98febc5d	IdxSub2	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794800666959	1794800666959	1794801113199
01a1e274-caae-74d4-8a99-8eb6dee95ee5	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e274-c676-76df-a2de-fe0b98febc5d	IdxUnv2	\N	\N	3	\N			\N	\N	0	\N	\N	\N		0	\N	1794800667310	1794800667310	1794801113199
01a1e274-c73d-70c1-a5ca-fec05dcb8da7	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e274-c676-76df-a2de-fe0b98febc5d	index	\N	\N	3	\N	\N	\N	\N	1	1	\N	-1	## 子目录\n\n```children\n```\n\n## 最近查看\n\n```recent\n```\n		62	{"index": true}	1794800666429	1794800666429	1794801113199
01a1e274-c8f4-734b-8149-82943f779f77	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e274-c676-76df-a2de-fe0b98febc5d	IdxB2	\N	\N	3	\N			\N	\N	0	\N	\N	\N		0	\N	1794800666868	1794800666868	1794801113199
01a1e274-ca4e-73c9-960d-dbd75017caf3	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e274-c94f-747b-b66c-a6271f85557e	IdxC2	\N	\N	3	\N			\N	\N	0	\N	\N	\N		0	\N	1794800667214	1794800667214	1794801113199
01a1e278-00f1-72b8-b562-5f086143b050	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e277-fd90-7755-b21f-b70400c88a22	DirKid2	\N	\N	3	\N			\N	\N	0	\N	\N	\N		0	\N	1794800877809	1794800877809	1794801113603
01a1e279-7ff2-7317-bce4-6d7abb37a05a	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	IdxT4	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794800975858	1794800975858	1794801113955
01a1e278-f4c9-73b4-b522-24582d2093d2	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e253-c299-7080-9577-727389e5fb72	index	\N	\N	3	\N	\N	\N	\N	1	1	\N	-1	## 目录\n\n```children\n```\n\n## 最近查看\n\n```recent\n```\n	\N	59	{"index": true}	1794800940233	1794801651475	1795203427357
01a1d033-ebe6-758b-806e-c4d35684d138	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d033-e81d-749a-9b64-09d8ff47ca7b	zhongqiu-guoqing-fangjia-20260923-20261024010128-e	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	zhongqiu-guoqing-fangjia-20260923-20261024010128-eq81zlt.pdf	01a1d033-ebe6-758b-806e-c4d35684d138.pdf	17044534	{"filetype": "application/pdf", "original_path": "zhongqiu-guoqing-fangjia-20260923-20261024010128-eq81zlt.pdf", "content_sha256": "d400fd68b5418b182ff3c2ab7e2187f3490479b47f7a135d067a27c3b16e6c7d", "transformed_path": "01a1d033-ebe6-758b-806e-c4d35684d138.pdf"}	1794494426105	1794494426105	\N
01a1d836-6bec-763a-b656-915625f6b74a	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	probe-512k.png	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	probe-512k.png	01a1d836-6bec-763a-b656-915625f6b74a.png	524296	{"filetype": "image/png", "original_path": "probe-512k.png", "content_sha256": "ee743c34ba9fde989a280f8c99a07900017c8edd683b516cec4c75b8a5d3195b", "transformed_path": "01a1d836-6bec-763a-b656-915625f6b74a.png"}	1794628808088	1794628808088	\N
01a1d837-5b63-700a-a524-02aa4eb69ea2	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	probe-8m.png	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	probe-8m.png	01a1d837-5b63-700a-a524-02aa4eb69ea2.png	8388616	{"filetype": "image/png", "original_path": "probe-8m.png", "content_sha256": "59fdc92de01f951d2bc437bfd96416ed1269b1239e87f86c431848cd9688ec47", "transformed_path": "01a1d837-5b63-700a-a524-02aa4eb69ea2.png"}	1794628870513	1794628870513	\N
01a1d033-e81d-749a-9b64-09d8ff47ca7b	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	关于中秋和国庆放假安排 20260923	\N	\N	3	\N	\N	\N	\N	\N	0	\N	\N	# 关于中秋和国庆放假安排 20260923\n\n## 元信息\n\n|项|值|\n| ----------| -------------------------------|\n|来源|用户直传归档（微信 PDF）|\n|内容|PDF 1 册（51 页，纯图无文本）|\n|大小|约 16.3 MB|\n|归档时间|2026-09-23 09:47:55Z|\n\n## 文件\n\n![](/api/resources/01a1d033-ebe6-758b-806e-c4d35684d138/file/zhongqiu-guoqing-fangjia-20260923-20261024010128-eq81zlt.pdf)\n\n## 备注\n\n- 微信源 PDF，无加密，直存；本地 ​`assets/public/`，非敏感。	\N	\N	\N	1794494425117	1794494429743	\N
01a1e253-bf21-720a-90f7-5c50d5115eae	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	daily note	\N	\N	1	\N			\N	\N	0	\N	\N	\N		0	\N	1794798501665	1794798501665	\N
01a1e23f-bb71-7272-83d8-dd80515be4c5	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e253-c5da-77e2-8724-f1ba8679486e	2026-11-02 07:17:20 <2026-11-02 07:17:20>	\N	\N	3	\N	\N	\N	\N	\N	0	\N	\N	[https://www.zhihu.com/question/715577762/answer/2095239383058329672?share_code=1aG1aX48DZmma&utm_psn=2100486577877268407](https://www.zhihu.com/question/715577762/answer/2095239383058329672?share_code=1aG1aX48DZmma&utm_psn=2100486577877268407)\n\n[https://zhuanlan.zhihu.com/p/2000126377677054450?share_code=1fE0WJhBKSObA&utm_psn=2100486622118790035](https://zhuanlan.zhihu.com/p/2000126377677054450?share_code=1fE0WJhBKSObA&utm_psn=2100486622118790035)	\N	\N	\N	1794797190001	1794798931931	\N
01a1e23f-b6bb-76bf-8a63-82e916ad55e4	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e23f-b26d-7373-9eeb-00c9e99a0368	2026	\N	\N	3	\N	\N	\N	\N	\N	0	\N	\N	‍	\N	\N	\N	1794797188795	1794797188795	1794798933999
01a1e26f-788d-778c-8fe7-686b71a1ef2c	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	IdxT	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794800318605	1794800318605	1794801111048
01a1e274-c868-73f9-b905-7e3b2202f214	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e274-c676-76df-a2de-fe0b98febc5d	IdxA2	\N	\N	3	\N			\N	\N	0	\N	\N	\N		0	\N	1794800666728	1794800666728	1794801113199
01a1d02b-d196-7245-ab98-46ac112d3704	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-d0b2-7357-b726-5fcc0eeb5506	955ef3bece80a43e9bbf6ccc7272ce00-20261108125259-dx	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	955ef3bece80a43e9bbf6ccc7272ce00-20261108125259-dxxunvg.mp4	01a1d02b-d196-7245-ab98-46ac112d3704.mp4	1743217	{"filetype": "video/mp4", "original_path": "955ef3bece80a43e9bbf6ccc7272ce00-20261108125259-dxxunvg.mp4", "content_sha256": "58235c016ed6184b404bdffa5b52dc34bc10af151f5ecc23c04f59a0443aef63", "transformed_path": "01a1d02b-d196-7245-ab98-46ac112d3704.mp4"}	1794493895069	1794493895069	\N
01a1d02b-d389-7692-a548-844e045cd31f	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-d0b2-7357-b726-5fcc0eeb5506	a9c705dbef0475f04a2775725c82c302-20261108125259-bp	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	a9c705dbef0475f04a2775725c82c302-20261108125259-bpsc9dg.mp4	01a1d02b-d389-7692-a548-844e045cd31f.mp4	2082655	{"filetype": "video/mp4", "original_path": "a9c705dbef0475f04a2775725c82c302-20261108125259-bpsc9dg.mp4", "content_sha256": "db70a6880d780fc0ce8de9e49070ee49951569fbe589cf6ebf309a028229e91c", "transformed_path": "01a1d02b-d389-7692-a548-844e045cd31f.mp4"}	1794493895569	1794493895569	\N
01a1d02b-d65e-7656-9f6f-1791a88616f2	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-d0b2-7357-b726-5fcc0eeb5506	ce2a02647322f36b98e53af11bb493a3-20261108125259-8c	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	ce2a02647322f36b98e53af11bb493a3-20261108125259-8cuwv01.mp4	01a1d02b-d65e-7656-9f6f-1791a88616f2.mp4	1541699	{"filetype": "video/mp4", "original_path": "ce2a02647322f36b98e53af11bb493a3-20261108125259-8cuwv01.mp4", "content_sha256": "752df2f8fba4c27d01264dbdbec01e9f9124e1ffafc09e3c143ecc14616f984c", "transformed_path": "01a1d02b-d65e-7656-9f6f-1791a88616f2.mp4"}	1794493896294	1794493896294	\N
01a1d02b-b95a-7541-940f-4731b4e52a6c	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0225qnzgp7nk-20261103101506-z	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0225qnzgp7nk-20261103101506-z2rmhc5.jpeg	01a1d02b-b95a-7541-940f-4731b4e52a6c.jpeg	77798	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0225qnzgp7nk-20261103101506-z2rmhc5.jpeg", "content_sha256": "53a35c99ec4c4a0644d8ce32fd40dd03be1ee91614b6709ef304d184aed0004d", "transformed_path": "01a1d02b-b95a-7541-940f-4731b4e52a6c.jpeg"}	1794493888876	1794493888876	\N
01a1e253-c3cf-7296-889e-69f645b5aa06	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e253-c299-7080-9577-727389e5fb72	11	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794798502863	1794798502863	\N
01a1d40d-61e6-70e9-a97d-e4fecce4368f	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d40c-272a-728a-9844-4164de145220	DQL-Render-Probe	\N	\N	3	\N			\N	\N	0	\N	\N	# Probe\n\n```dataview\nLIST FROM "DQL-Probe" LIMIT 10\n```\n\n```dataview\nTABLE name, role WHERE contains(name, "Probe")\n```\n\n```index\n```\n		134	\N	1794559009254	1794559009254	1794559184338
01a1d40c-272a-728a-9844-4164de145220	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	DQL-Probe	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794558928682	1794558928682	1794559184554
01a1e253-c5da-77e2-8724-f1ba8679486e	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e253-c3cf-7296-889e-69f645b5aa06	2026-11-02	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794798503386	1794798503386	\N
01a1e23f-b83b-7385-87a5-f3221168c5f8	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e23f-b6bb-76bf-8a63-82e916ad55e4	11	\N	\N	3	\N	\N	\N	\N	\N	0	\N	\N	‍	\N	\N	\N	1794797189179	1794797189179	1794798933999
01a1e253-8819-7120-a32d-3a3e01fd6cf5	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	perm-probe	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794798487577	1794798487577	1794798937356
01a1e270-0efd-756a-a635-d3a2ea2df743	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e270-0e32-71e9-9e5f-88581359365f	index	\N	\N	3	\N	\N	\N	\N	1	1	\N	-1	## 子目录\n\n```dataview\nLIST FROM "id:01a1e270-0e32-71e9-9e5f-88581359365f" DIRECT\n```\n\n## 最近查看\n\n```dataview\nLIST FROM "id:01a1e270-0e32-71e9-9e5f-88581359365f" WHERE role = 3 SORT viewed_timestamp DESC LIMIT 10\n```\n		226	{"index": true}	1794800357117	1794800357117	1794800357249
01a1e270-1178-763d-ba83-76b5065d6cd6	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e270-108e-72ff-840c-656306ca0c4e	index	\N	\N	3	\N	\N	\N	\N	1	1	\N	-1	## 子目录\n\n```dataview\nLIST FROM "id:01a1e270-108e-72ff-840c-656306ca0c4e" DIRECT\n```\n\n## 最近查看\n\n```dataview\nLIST FROM "id:01a1e270-108e-72ff-840c-656306ca0c4e" WHERE role = 3 SORT viewed_timestamp DESC LIMIT 10\n```\n		226	{"index": true}	1794800357752	1794800357752	1794801112246
01a1e270-140d-7712-b2dc-a8cd9b4f279c	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e270-108e-72ff-840c-656306ca0c4e	IdxB	\N	\N	3	\N			\N	\N	0	\N	\N	\N		0	\N	1794800358413	1794800358413	1794801112246
01a1e270-1534-72e3-bc40-5028c6d3d7d6	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e270-14ab-758e-9fa3-f0e55ce3c0bf	index	\N	\N	3	\N	\N	\N	\N	1	1	\N	-1	## 子目录\n\n```dataview\nLIST FROM "id:01a1e270-14ab-758e-9fa3-f0e55ce3c0bf" DIRECT\n```\n\n## 最近查看\n\n```dataview\nLIST FROM "id:01a1e270-14ab-758e-9fa3-f0e55ce3c0bf" WHERE role = 3 SORT viewed_timestamp DESC LIMIT 10\n```\n		226	{"index": true}	1794800358708	1794800358708	1794801112246
01a1e274-c9ee-762d-ba35-3df35742c25f	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e274-c94f-747b-b66c-a6271f85557e	index	\N	\N	3	\N	\N	\N	\N	1	1	\N	-1	## 子目录\n\n```children\n```\n\n## 最近查看\n\n```recent\n```\n		62	{"index": true}	1794800667118	1794800667118	1794801113199
01a1e26f-db2c-73ce-809c-abdaed4866dd	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	DbgProbe	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794800343852	1794800343852	1794801131983
01a1dddf-5d33-7138-8a35-291084a90501	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	Pergame 精简部署(复用已有 pg + S3)	\N	\N	3	\N	\N	\N	\N	\N	0	\N	\N	> 适用场景:**NAS 上已有 PostgreSQL 与 S3 对象存储**(宿主机原生部署,非容器),只想跑 Pergame 本身。  \n> 只启动三个容器:backend / frontend / dr-backup,不再打包 pg 和 RustFS。\n>\n> **配置全部写在 compose 文件里,不需要 .env**。改配置 = 编辑 compose 后 ​`up -d`。\n>\n> 完整版(自带 pg + RustFS,配 .env)见同目录另一篇笔记。\n\n## 1 前置检查\n\n```bash\n# 1) 外部网络是否存在\ndocker network ls | grep postgres\n\n# 2) 哪些 pg 容器在该网络里 → 记下容器名,填进 DATABASE_HOST\ndocker network inspect postgres --format '{{range .Containers}}{{.Name}}{{"\\n"}}{{end}}'\n\n# 3) 该 pg 的库列表(pergame 库需事先建好)\ndocker exec -it <pg容器名> psql -U postgres -c '\\l'\n```\n\n> 若 ​`postgres`​ 网络不存在,先创建 ​`docker network create postgres`​,并把 pg 容器接进去。  \n> 网络名必须与 compose 里 ​`networks.postgres` 一致,否则 backend 起不来。\n>\n> **backend 只跑 migration,不建库**。首次部署前先执行:\n>\n> ```bash\n> docker exec -it <pg容器名> psql -U postgres -c 'CREATE DATABASE pergame;'\n> ```\n\n## 2 与完整版的差异\n\n|项|完整版|精简版|\n| ------------| -----------------------------------------------| ------------------------------|\n|服务数|5(postgres/rustfs/backend/frontend/dr-backup)|**3**(backend/frontend/dr-backup)|\n|配置文件|compose + ​`.env`|**只有 compose**(配置内联)|\n|PostgreSQL|自带容器|**复用外部**,经 ​`postgres` 外部网络|\n|对象存储|自带 RustFS|**复用外部 S3**(宿主机原生部署)|\n|数据卷|5 个|**2 个**(​`pergame-dr-conf`​/​`pergame-dr-jobs`,仅 DR 用)|\n|`down -v` 影响|删库删桶|**只删 DR 配置**|\n\n## 3 部署\n\n把 §8 的 compose 全文保存为 ​`docker-compose.external.yml`​,先改掉 4 处 ​`CHANGE-ME`,然后:\n\n```bash\ndocker compose -f docker-compose.external.yml up -d\ndocker compose -f docker-compose.external.yml logs -f backend\n```\n\n期望日志顺序:​`migrations applied`​ → ​`database ready`​ → ​`S3 storage configured`​ → ​`Successfully created pergame`​ → ​`pergame-backups`​ → ​`listening addr=0.0.0.0:8201`。\n\n健康检查(**经前端代理**,backend 不对外发布端口):\n\n```bash\ncurl http://<NAS-IP>:8200/api/health   # → {"database":true,"status":"ok"}\n```\n\n## 4 必须改的配置(在 compose 内直接改)\n\n|位置|字段|说明|\n| ---------| --------------------| -------------------------------------------------------------|\n|backend|`JWT_SECRET`|`openssl rand -hex 32`。改它 = 所有会话失效|\n|backend|`DATABASE_HOST`​ / ​`DATABASE_USER`​ / ​`DATABASE_PASSWORD`|pg 容器名、用户、密码(按你 pg 实例改)|\n|backend|`S3_ENDPOINT`​ / ​`S3_PUBLIC_URL`|内网经 host.docker.internal;对外用浏览器可达地址(示例 ​`qnap.elf-balance.ts.net:8010`)|\n|backend|`S3_ACCESSKEY`​ / ​`S3_SECRETKEY`|对象存储凭据|\n|其余|—|全部有安全默认值,可不动;全量参数表见仓库 ​`docs/content/6.deploy/6.configurations-list.md`|\n\n另按实际情况调整:​`DATABASE_HOST`​(pg 容器名)、​`NUXT_PUBLIC_CDN_PROXY_TARGET`。\n\n## 5 架构:同源反代,只有一个对外端口\n\n浏览器只访问前端 ​`:8200`;frontend 容器内的 Nitro 中间件把两类路径转发给内网服务:\n\n```\n浏览器 → :8200\n            ├── /api/**      → backend:8201   (NUXT_PUBLIC_API_PROXY_TARGET)\n            └── /pergame/**  → S3 (NUXT_PUBLIC_CDN_PROXY_TARGET)\n```\n\n- **backend 不对外发布端口**(compose 无 ​`ports`​;同网络容器本就能互访全部端口,无需任何声明);调试需直连时临时加回 ​`ports: ["8201:8201"]`\n- 同源请求,**CORS 问题不存在**\n- Cookie 由代理原样转发(已实测 Set-Cookie 逐字节一致)\n\n### S3 是宿主机原生服务 → 必须加 extra_hosts\n\nS3(QuObjects 等)跑在**宿主机**而非容器里。容器内的 ​`localhost`​ 指向容器自身,必然失败;Linux 上 ​`host.docker.internal`​ 也**默认不可解析**,必须显式映射。compose 里三个服务都加了:\n\n```yaml\n# ==============================================================================\n# Pergame — 精简版(复用已有 PostgreSQL + S3)\n#\n# 只跑三个服务:backend / frontend / dr-backup。\n# pg 与对象存储用外部已有的,通过名为 postgres 的外部 Docker 网络接入。\n#\n# 所有配置直接写在本文件里,**不需要 .env**。改配置 = 编辑本文件后\n#   docker compose -f docker-compose.external.yml up -d\n#\n# 与 docker-compose.yml 的区别:那个是自包含版(pg + RustFS 一起起,5 个服务\n# 5 个卷,配 .env 使用);这个只跑应用本身(3 个服务 2 个卷)。精简版的好处:\n# down -v 只删 DR 配置,不会碰 pg 数据或 S3 对象。\n#\n# 前置:\n#   1) 确认外部网络存在:docker network inspect postgres\n#   2) 确认 pg 容器在其中,记下容器名 → 填 DATABASE_HOST\n#   3) 手工建库(backend 只跑 migration,不建库):\n#        docker exec -it <pg容器> psql -U postgres -c 'CREATE DATABASE pergame;'\n#   4) 确认 S3 桶存在(backend 会自动建,但不会改已有桶的归属)\n#\n# 启动:\n#   docker compose -f docker-compose.external.yml up -d\n# ==============================================================================\n# 镜像源说明:用 ghcr.nju.edu.cn(南京大学 GHCR 反代)代替 ghcr.io。\n# 本机直连 ghcr.io 拉取经常超时(实测 36MB 层 30 分钟未完成),镜像站 2 秒完成。\n# 若该站不可用,把 ghcr.nju.edu.cn 换回 ghcr.io 即可(内容一致,同一 digest)。\n# 备选:ghcr.1ms.run(Portainer > Registries 里已配置,同样可用)。\n\nservices:\n  # ---------------------------------------------------------------- backend\n  backend:\n    image: ghcr.nju.edu.cn/kamasylvia/pergame-backend:v0.0.3\n    container_name: pergame-backend\n    restart: unless-stopped\n    environment:\n      # 只列无默认值或与环境强相关的变量;其余全有安全默认\n      # (快照/回收站/SMTP/注册开关等),全量参考:\n      # docs/content/6.deploy/6.configurations-list.md\n      # 改 JWT_SECRET = 所有会话失效。生成:openssl rand -hex 32\n      JWT_SECRET: "CHANGE-ME-openssl-rand-hex-32"\n      # 无 HTTPS 时必须显式 true:默认走 secure cookie,HTTP 下浏览器拒收\n      ALLOW_UNSECURE: "true"\n\n      # ---------- 外部 PostgreSQL ----------\n      # pg 容器在 postgres 网络里的名字(不是 IP、不是域名)\n      DATABASE_HOST: "pg18"\n      DATABASE_USER: "postgres"\n      DATABASE_PASSWORD: "CHANGE-ME-pg-password"\n\n      # ---------- 外部对象存储 ----------\n      # S3 是宿主机上的原生服务(非容器),经 host.docker.internal 访问\n      # (由下方 extra_hosts 映射到宿主机网关,Linux 上必须显式声明)。\n      S3_ENDPOINT: "http://host.docker.internal:8010"\n      # 浏览器能访问到的地址(附件直链走它)\n      S3_PUBLIC_URL: "http://qnap.elf-balance.ts.net:8010"\n      S3_ACCESSKEY: "CHANGE-ME-s3-access-key"\n      S3_SECRETKEY: "CHANGE-ME-s3-secret-key"\n\n      # ---------- DR sidecar 共享卷 ----------\n      # 与下方 dr_conf/dr_jobs 挂载对应;不部署 DR 就连这两行带卷一起删\n      DR_CONF_DIR: "/conf"\n      DR_JOBS_DIR: "/jobs"\n      # 时区:影响 target cron 调度与日志墙钟(镜像含 tzdata)。\n      # 不设 = UTC;要按本地墙钟跑 `0 2 * * *` 这类调度就解开注释\n      # TZ: "Asia/Shanghai"\n\n    # No published port: the browser reaches the API through the frontend's\n    # same-origin proxy (NUXT_PUBLIC_API_PROXY_TARGET below). `expose` would\n    # be pure documentation here (same-network containers can reach every\n    # port regardless), so it is omitted. For direct access during debugging,\n    # temporarily re-add: ports: ["8201:8201"]\n    # S3 是宿主机原生服务:把 host.docker.internal 指到宿主机网关\n    # (Linux 必需,不加此映射 host.docker.internal 无法解析)\n    extra_hosts:\n      - "host.docker.internal:host-gateway"\n    volumes:\n      - dr_conf:/conf\n      - dr_jobs:/jobs\n    networks:\n      - default   # 应用网络(top-level 里改名为 pergame)\n      - postgres\n\n  # --------------------------------------------------------------- frontend\n  frontend:\n    image: ghcr.nju.edu.cn/kamasylvia/pergame-frontend:v0.0.3\n    container_name: pergame-frontend\n    restart: unless-stopped\n    environment:\n      PORT: 8200\n      # ---- 同源反代模式(推荐):浏览器只访问前端端口 ----\n      # BASE_API/BASE_CDN 留空(默认)走相对路径,由 Nitro 中间件转发:\n      #   /api/**      → NUXT_PUBLIC_API_PROXY_TARGET(backend 容器)\n      #   /pergame/**  → NUXT_PUBLIC_CDN_PROXY_TARGET(对象存储)\n      # backend 服务在本 compose 的 pergame 网络内,直接用服务名\n      NUXT_PUBLIC_API_PROXY_TARGET: "http://backend:8201"\n      # S3 内网地址:宿主机原生服务,经 host.docker.internal 访问(见 backend extra_hosts)\n      NUXT_PUBLIC_CDN_PROXY_TARGET: "http://host.docker.internal:8010"\n    ports:\n      - "8200:8200"\n    # frontend 的 CDN 代理同样要访问宿主机上的原生 S3\n    extra_hosts:\n      - "host.docker.internal:host-gateway"\n    depends_on:\n      - backend\n\n  # --------------------------------------------------------------- dr-backup\n  # 实例 DR sidecar:执行 backend 通过共享卷交付的 pg_dump 任务。\n  # 不需要实例级备份就删掉本服务 + backend 的 /conf,/jobs 挂载。\n  dr-backup:\n    # 版本钉死,不用浮动 tag:pg_dump 行为必须按 release 可复现。\n    # 当前钉 pg18-v8.15.0(基底 postgres:18.2-alpine,匹配活库 PostgreSQL\n    # 18.2)。pg 升级或 fork 升版时手动改这里的 tag。\n    image: ghcr.nju.edu.cn/kamasylvia/pergame-dr-backup:v0.0.3\n    container_name: pergame-dr-backup\n    restart: unless-stopped\n    # 镜像自带全部默认值(SCHEDULE_HOURS=0 仅手动 / SCAN_INTERVAL_SECONDS=2 /\n    # JOBS_DIR=/jobs / CONF_DIR=/conf);要定时 dump 或改路径时才需要显式 env\n    # (见 docs/content/6.deploy/6.configurations-list.md 的 DR sidecar 节)。\n    environment:\n      # 时区:备份 stamp 目录(如 2026-11-11_020000)与 job 时间戳跟它走。\n      # 与 backend 的 TZ 保持一致,人工核对异地目录时不用换算时差\n      TZ: "Asia/Shanghai"\n    # rclone 目的地若指向宿主机上的 S3,同样需要 host.docker.internal\n    extra_hosts:\n      - "host.docker.internal:host-gateway"\n    volumes:\n      - dr_conf:/conf\n      - dr_jobs:/jobs\n    networks:\n      - default\n      - postgres   # sidecar 要连 pg 做 dump\n\nnetworks:\n  # 应用内部网络 = compose 的 default 网络,显式改名为 pergame。\n  # 未在服务里声明 networks 的服务自动加入它;声明了的要显式带上 default。\n  default:\n    name: pergame\n  # 已有的外部网络,pg 容器在其中;compose 不创建也不删除它\n  postgres:\n    external: true\n\n# 卷名显式固定:compose 默认会加「目录名_」前缀,目录改名/移动后\n# 会生成一套全新的空卷(旧数据还在但容器挂的是新卷)。显式命名避免这种漂移。\nvolumes:\n  dr_conf:\n    name: pergame-dr-conf\n  dr_jobs:\n    name: pergame-dr-jobs\n\n```\n\n加映射后,S3 地址统一用 ​`http://host.docker.internal:8010`:\n\n|变量|谁在用|填什么|\n| ----------------------------| ---------------------| ----------------------------|\n|`S3_ENDPOINT`(backend)|backend 容器 → S3|`http://host.docker.internal:8010`|\n|`NUXT_PUBLIC_CDN_PROXY_TARGET`(frontend)|frontend 容器 → S3|同上|\n|dr-backup 的 rclone 目的地|sidecar → S3|同上(若目的地是这台宿主机)|\n|(浏览器侧)|无需配置|走前端同源 ​`/pergame/**`|\n\n> 旧写法 ​`172.17.0.1`​(docker0 网关)也能通,但网段可能因 docker 配置而变;​`host.docker.internal`​ + ​`host-gateway` 是 Docker 20.10+ 的正式做法,更稳。\n\n### 别复用有其他数据的桶\n\nbackend 启动时会给**整个桶**设公共读策略,影响桶内所有对象。​`S3_BUCKET` 指向独立空桶最安全。\n\n## 6 数据卷:显式命名\n\ncompose 的匿名命名规则是「**目录名_卷名**」——目录改名/移动后,​`up -d`​ 会生成一套**全新的空卷**(旧数据还在,但容器挂的是新卷),表现为配置丢失。所以两个卷都显式固定了名字:\n\n```yaml\nvolumes:\n  dr_conf:\n    name: pergame-dr-conf\n  dr_jobs:\n    name: pergame-dr-jobs\n```\n\n> 严格说 ​`name:`​ **不是必需**的;但对外置基础设施的部署(文件可能被挪动、目录名可能不固定),显式命名能避免这类静默漂移,​`docker volume ls` 也一眼可辨。\n\n## 7 Smoke 检查单\n\n1. **健康检查**(经代理):​`curl http://<NAS-IP>:8200/api/health`​ 返回 ​`status: ok`\n2. **注册登录**:打开 ​`http://<NAS-IP>:8200`​。⚠️ 能登录但操作全 401 → 查 ​`COOKIE_DOMAIN` 是否带了端口\n3. **建库写文档**:新建 workspace → 新建文档 → 保存 → 刷新还在\n4. **附件上传**:粘贴图片能显示(验 CDN 代理链路)\n5. **回收站**:删文档 → ​`/dashboard/trash` → 恢复\n6. **快照**:设置 → Backup → Create backup\n7. **快照下载**:得到 zip(​`meta.json`​ + ​`manifest.json`​ + ​`blobs/…`)\n8. **快照还原**:改数据 → 还原 → 二次确认 → 回滚;期间顶部显示只读提示\n9. **异地目标**(可选):Instance DR → Offsite snapshot targets → **先测试连接** → Save → Sync now\n10. **实例 DR**(可选):Instance DR → 配目的地 → Back up now\n\n## 8 升级\n\n```bash\ndocker compose -f docker-compose.external.yml pull\ndocker compose -f docker-compose.external.yml up -d\n```\n\n> `down -v`​ 只会删 ​`pergame-dr-conf`​/​`pergame-dr-jobs`​,**不会**删 pg 数据或 S3 对象。\n\n> ⚠️ **pg 主版本耦合**:dr-backup 镜像基于 ​`postgres:18-alpine`,pg_dump 不能导出比它新的服务器。pg 升级大版本时必须同步更新 sidecar 镜像。\n\n## 9 docker-compose.external.yml 全文\n\n```yaml\n# ==============================================================================\n# Pergame — 精简版(复用已有 PostgreSQL + S3)\n#\n# 只跑三个服务:backend / frontend / dr-backup。\n# pg 与对象存储用外部已有的,通过名为 postgres 的外部 Docker 网络接入。\n#\n# 所有配置直接写在本文件里,**不需要 .env**。改配置 = 编辑本文件后\n#   docker compose -f docker-compose.external.yml up -d\n#\n# 与 docker-compose.yml 的区别:那个是自包含版(pg + RustFS 一起起,5 个服务\n# 5 个卷,配 .env 使用);这个只跑应用本身(3 个服务 2 个卷)。精简版的好处:\n# down -v 只删 DR 配置,不会碰 pg 数据或 S3 对象。\n#\n# 前置:\n#   1) 确认外部网络存在:docker network inspect postgres\n#   2) 确认 pg 容器在其中,记下容器名 → 填 DATABASE_HOST\n#   3) 手工建库(backend 只跑 migration,不建库):\n#        docker exec -it <pg容器> psql -U postgres -c 'CREATE DATABASE pergame;'\n#   4) 确认 S3 桶存在(backend 会自动建,但不会改已有桶的归属)\n#\n# 启动:\n#   docker compose -f docker-compose.external.yml up -d\n# ==============================================================================\nservices:\n  # ---------------------------------------------------------------- backend\n  backend:\n    image: ghcr.io/kamasylvia/pergame-backend:v0.0.1\n    container_name: pergame-backend\n    restart: unless-stopped\n    environment:\n      # 只列无默认值或与环境强相关的变量;其余全有安全默认\n      # (快照/回收站/SMTP/注册开关等),全量参考:\n      # docs/content/6.deploy/6.configurations-list.md\n      # 改 JWT_SECRET = 所有会话失效。生成:openssl rand -hex 32\n      JWT_SECRET: "26eb67dc0d424b8791f40ffd5d37f898acd0af8ef6ee949ca41d862d7c69253e"\n      # 无 HTTPS 时必须显式 true:默认走 secure cookie,HTTP 下浏览器拒收\n      ALLOW_UNSECURE: "true"\n\n      # ---------- 外部 PostgreSQL ----------\n      # pg 容器在 postgres 网络里的名字(不是 IP、不是域名)\n      DATABASE_HOST: "pg18"\n      DATABASE_USER: "postgres"\n      DATABASE_PASSWORD: "postgres"\n\n      # ---------- 外部对象存储 ----------\n      # S3 是宿主机上的原生服务(非容器),经 host.docker.internal 访问\n      # (由下方 extra_hosts 映射到宿主机网关,Linux 上必须显式声明)。\n      S3_ENDPOINT: "http://host.docker.internal:8010"\n      # 浏览器能访问到的地址(附件直链走它)\n      S3_PUBLIC_URL: "http://qnap.elf-balance.ts.net:8010"\n      S3_ACCESSKEY: "QuObjects:LggrAwIPzTkDA31KX085"\n      S3_SECRETKEY: "ZrP6OfOjnLFxUhqmrtMDUsMa5aSXWUlk"\n\n      # ---------- DR sidecar 共享卷 ----------\n      # 与下方 dr_conf/dr_jobs 挂载对应;不部署 DR 就连这两行带卷一起删\n      DR_CONF_DIR: "/conf"\n      DR_JOBS_DIR: "/jobs"\n      # 时区:影响 target cron 调度与日志墙钟(镜像含 tzdata)。\n      # 不设 = UTC;要按本地墙钟跑 `0 2 * * *` 这类调度就解开注释\n      # TZ: "Asia/Shanghai"\n\n    # No published port: the browser reaches the API through the frontend's\n    # same-origin proxy (NUXT_PUBLIC_API_PROXY_TARGET below). `expose` would\n    # be pure documentation here (same-network containers can reach every\n    # port regardless), so it is omitted. For direct access during debugging,\n    # temporarily re-add: ports: ["8201:8201"]\n    # S3 是宿主机原生服务:把 host.docker.internal 指到宿主机网关\n    # (Linux 必需,不加此映射 host.docker.internal 无法解析)\n    extra_hosts:\n      - "host.docker.internal:host-gateway"\n    volumes:\n      - dr_conf:/conf\n      - dr_jobs:/jobs\n    networks:\n      - default   # 应用网络(top-level 里改名为 pergame)\n      - postgres\n\n  # --------------------------------------------------------------- frontend\n  frontend:\n    image: ghcr.io/kamasylvia/pergame-frontend:v0.0.1\n    container_name: pergame-frontend\n    restart: unless-stopped\n    environment:\n      PORT: 8200\n      # ---- 同源反代模式(推荐):浏览器只访问前端端口 ----\n      # BASE_API/BASE_CDN 留空(默认)走相对路径,由 Nitro 中间件转发:\n      #   /api/**      → NUXT_PUBLIC_API_PROXY_TARGET(backend 容器)\n      #   /pergame/**  → NUXT_PUBLIC_CDN_PROXY_TARGET(对象存储)\n      # backend 服务在本 compose 的 pergame 网络内,直接用服务名\n      NUXT_PUBLIC_API_PROXY_TARGET: "http://backend:8201"\n      # S3 内网地址:宿主机原生服务,经 host.docker.internal 访问(见 backend extra_hosts)\n      NUXT_PUBLIC_CDN_PROXY_TARGET: "http://host.docker.internal:8010"\n    ports:\n      - "8200:8200"\n    # frontend 的 CDN 代理同样要访问宿主机上的原生 S3\n    extra_hosts:\n      - "host.docker.internal:host-gateway"\n    depends_on:\n      - backend\n\n  # --------------------------------------------------------------- dr-backup\n  # 实例 DR sidecar:执行 backend 通过共享卷交付的 pg_dump 任务。\n  # 不需要实例级备份就删掉本服务 + backend 的 /conf,/jobs 挂载。\n  dr-backup:\n    # 版本钉死,不用浮动 tag:pg_dump 行为必须按 release 可复现。\n    # 当前钉 pg18-v0.0.1(基底 postgres:18-alpine,匹配活库 PostgreSQL 18)。\n    # pg 升级或 fork 升版时手动改这里的 tag;tag 格式为 pg18-<版本号>。\n    image: ghcr.io/kamasylvia/pergame-dr-backup:pg18-v0.0.1\n    container_name: pergame-dr-backup\n    restart: unless-stopped\n    # 镜像自带全部默认值(SCHEDULE_HOURS=0 仅手动 / SCAN_INTERVAL_SECONDS=2 /\n    # JOBS_DIR=/jobs / CONF_DIR=/conf);要定时 dump 或改路径时才需要显式 env\n    # (见 docs/content/6.deploy/6.configurations-list.md 的 DR sidecar 节)。\n    environment:\n      # 时区:备份 stamp 目录(如 2026-11-11_020000)与 job 时间戳跟它走。\n      # 与 backend 的 TZ 保持一致,人工核对异地目录时不用换算时差\n      TZ: "Asia/Shanghai"\n    # rclone 目的地若指向宿主机上的 S3,同样需要 host.docker.internal\n    extra_hosts:\n      - "host.docker.internal:host-gateway"\n    volumes:\n      - dr_conf:/conf\n      - dr_jobs:/jobs\n    networks:\n      - default\n      - postgres   # sidecar 要连 pg 做 dump\n\nnetworks:\n  # 应用内部网络 = compose 的 default 网络,显式改名为 pergame。\n  # 未在服务里声明 networks 的服务自动加入它;声明了的要显式带上 default。\n  default:\n    name: pergame\n  # 已有的外部网络,pg 容器在其中;compose 不创建也不删除它\n  postgres:\n    external: true\n\n# 卷名显式固定:compose 默认会加「目录名_」前缀,目录改名/移动后\n# 会生成一套全新的空卷(旧数据还在但容器挂的是新卷)。显式命名避免这种漂移。\nvolumes:\n  dr_conf:\n    name: pergame-dr-conf\n  dr_jobs:\n    name: pergame-dr-jobs\n```\n	\N	\N	\N	1794723765582	1794723765582	\N
01a1e26f-dbb4-7173-9d03-1b1aed40b954	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e26f-db2c-73ce-809c-abdaed4866dd	index	\N	\N	3	\N	\N	\N	\N	1	1	\N	-1	## 子目录\n\n```dataview\nLIST FROM "id:01a1e26f-db2c-73ce-809c-abdaed4866dd" DIRECT\n```\n\n## 最近查看\n\n```dataview\nLIST FROM "id:01a1e26f-db2c-73ce-809c-abdaed4866dd" WHERE role = 3 SORT viewed_timestamp DESC LIMIT 10\n```\n		226	{"index": true}	1794800343988	1794800343988	1794801131983
01a1e256-a143-735b-823d-6023bbee7a57	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e253-c5da-77e2-8724-f1ba8679486e	index	\N	\N	3	\N	\N	\N	\N	1	1	\N	-1	## 目录\n\n```children\n```\n\n## 最近查看\n\n```recent\n```\n	\N	13	{"index": true}	1794798690627	1794801659813	1795203427357
01a1d02b-aed8-74bb-925b-50c9bd4d6881	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	Zhihu - 国家为何还不修清史？	\N	\N	3	\N			\N	\N	0	\N	\N	> [国家为何还不修清史？](https://www.zhihu.com/question/715577762/answer/2095239383058329672?share_code=HQMHsGghQlGW&utm_psn=2100608642760303847)\n\n关于国内清史修不修得出来，我觉得各位不用特别在意。因为关于清史这事儿，美国人已经给咱们修得很详实了。今天我想通过引用美国历史学家欧立德(Mark C. Elliott) 的代表作《满洲之道: 中国晚期帝制八旗与族群认同》，系统性地向大家介绍一下清朝八旗“寄生虫”与“特权层”的成分和性质。并以此请各位自行斟酌为什么国内清史修不出来，以及为什么美国人就能写出跟咱们版本完全相反的“清史”。\n\n注意，我这里说“寄生虫”，不是故意要抹黑咱们八旗。是因为“寄生”和“特权”这些事在清朝是公知----是八旗旗人自己、本土汉人普通民众以及外国学者观察者等, 一切国内外人士都心知肚明的事儿。\n\n![](/api/resources/01a1d02b-b1a3-7380-8042-5d0e52b57151/file)\n\n\n\\[美\\]欧立德(Mark C. Elliott) 《满洲之道: 中国晚期帝制八旗与族群认同》\n\n![](/api/resources/01a1d02b-b039-7668-ac08-a8f0b48de6b5/file)\n\n![]()\n\n清朝八旗不仅在财政收入方面享有特权，旗人还享有诸多法律与职业上的特殊权利，而这些“特权”进一步凸显了本土汉人与满洲人在社会--军事阶层之间的各项隔离。德国历史学家弗兰克(Franke) 指出，征服政权往往会持续保留各种区分机制；有学者近期亦评论道：“尽管汉人与满洲人之间关系错综复杂，但他们始终属于两个不同的世界。在制度、法律和财政层面，满汉之间彼此独立，各自拥有独立的法规体系。” 本文将讲述“八旗旗人”在制度、法律和经济方面的各类特权。\n\n---\n\n## 《寄生篇》\n\n![](/api/resources/01a1d02b-b0ae-77b4-9bd9-5e7844efb76f/file)\n\n![]()\n\n![](/api/resources/01a1d02b-af51-76e1-8cbf-4d6243fd6237/file)\n\n![]()\n\n![](/api/resources/01a1d02b-b121-71d0-adec-3a1263ed50d9/file)\n\n![]()\n\n### **1.**  **收入“寄生”**\n\n旗人生活需要钱；清廷会以白银的形式发放给他们，或从国库中借款给他们。在衣食住行方面，清廷通过发放定额粮食来满足旗人的日常需求，粮食通常来自北京周边的粮仓，或是(对于驻防八旗而言) 来自各省的征粮配额；此外，清廷还在北京及各省的满城中为旗人修建并维护住所。清廷还负责为八旗配备武器，每个旗人的装备均由官府出资购置。部分旗人被要求配马；清廷会按规定数量提供马匹，但此仅限满洲与蒙古八旗，并额外发放购买草料(如干草与豆类) 的补贴。如果旗人因出行、婚丧嫁娶等事宜产生额外的开销，清廷也会提供专项补助。而当旗人因故深陷债务时，即便已有朝廷的各项经济援助，官府仍会提供必要资金，以助八旗偿还债权人。由于清朝为八旗提供了从生到死的全方位供养，套用当时形容所有领取官俸者的一种形象说法，即八旗中人皆是“吃皇家俸禄”的。到了清后期，旗人也同样自我定位，他们自称是“吃皇粮”的人。\n\n清朝八旗所获得的财政支持，从理论上讲主要有两种形式：一是银饷与粮饷，二是财产赏赐。银饷(军官所得者称为“俸银”，士兵所得者称为“兵饷”) 按月发放。旗人所领银两金额取决于其品级及驻防地点；通常京师旗人每月可领银四两，而其它中国城市的驻防旗人则领银三两。粮饷数额亦因品级而异，通常于春秋两季分两次发放。在京师，侍卫亲军、领催及八旗骑兵的标准年粮额为22石2斗。驻防各省的旗人，其粮饷数额虽因地而异，但通常约为每年30石，即每月2.5石。在粮饷中，一部分以实物粮食的形式发放，其余部分则折算为等值的银两进行支付。\n\n![](/api/resources/01a1d02b-b2ba-71e7-85f3-a352eacbaf97/file)\n\n![]()\n\n![](/api/resources/01a1d02b-b386-7363-b51d-f9a59afedc5f/file)\n\n![]()\n\n![](/api/resources/01a1d02b-b241-7208-ab62-1f34e5a3b125/file)\n\n![]()\n\n### 2\\. 土地“寄生”\n\n事实上，清朝最初设想以“圈地”(即土地赏赐) 而非银饷或粮饷，作为支撑八旗的主要经济来源。在满清征服过程中，北京周边被圈占的土地多达1400多万亩，其中大部分被分给了满洲贵族，也有大片土地分拨给八旗各旗，用于供养普通旗户。在清初，凡占领北京且在旗籍册上的男八旗，均可根据其品级的高低，被清廷赏赐30亩或更多的土地。这些旗地享有免税特权，且因其不可转让(仅限在旗人内部买卖)，成为旗人永久的收入来源。土地耕作多由汉人农奴或佃户承担，而八旗庄园的管理事务通常由一名同样是汉人的“庄头”负责。\n\n然而，八旗的经济支柱在清朝成立之初的二十年后便开始瓦解。首先，“诚实”与“公允”也许并非是当时八旗旗人所具备的品质；这导致了无数针对汉人农奴的残酷虐待，这些汉人在极其悲惨的劳动条件下，最终选择了逃离被八旗圈占的土地。与此同时，这些八旗管理者也并不介意欺瞒他们的满洲主子。因此，只有那些有充裕时间和精力密切监管自家田产的旗人，才能真正得站稳地主之位。然而，当时大多数旗人都奔赴在前线忙着征服明朝。于是没过多久，地租收入便断绝，旗人地主要么失去了对土地的掌控，要么干脆任由土地荒废；最终，这些土地流回市场，重新落入汉人手中。\n\n清廷曾多次颁布禁令，禁止买卖旗地，这恰恰证明了清朝根本无法阻止旗地的流失。尽管部分非法出售的“旗地”后来被收回，但对于普通旗人而言，旗地制度(这与日本江户时期针对武士阶层所设的类似制度形成鲜明对比) 始终未能如清朝统治者所愿，成为解决他们生计问题的长久之计。八旗旗人对清朝持续提供的钱粮依赖程度反而日益加深。到了18世纪中叶，八旗已因其游手好闲、寄生虫式的生活方式而“声名狼藉”；他们日益奢靡的生活习惯，与他们愈发贫困的境况之间显得愈发格格不入。\n\n![](/api/resources/01a1d02b-b579-7150-b852-66d48004cfe9/file)\n\n![]()\n\n![](/api/resources/01a1d02b-b31f-7505-8247-05827faea653/file)\n\n![]()\n\n### 3\\. 粮食“寄生”\n\n为了在一定程度上平衡京师八旗与各地驻防旗人之间的差异，对于那些未被授予圈占土地的旗人，清廷又发放了一项额外的补助----旨在补偿这些旗人因无地而损失的收入。此补助被称为“口粮”，其标准统一设定为每人每月1/4斛。领取“口粮”的旗人包括1647年(明永历元年) 后占领明朝首都北京的八旗兵，以及几乎所有驻防在中国各地的八旗兵。若按每户10口人计算，这项“口粮”补助相当可观，每月达2.5“石”，全年共计30“石”米，相当于又一份全年的基本粮饷。因此，我们可以总结----在北京，一名普通旗兵的收入构成包括：土地收益(仅此一项便足以优渥地供养全家)、银饷，以及按其银饷数额比例发放的“饷米”补助。驻防旗人若未被分配土地，境况或许稍逊，但仍能领取银饷、粮饷以及“口粮”补助。\n\n![](/api/resources/01a1d02b-b4ff-7248-962b-a160331803a9/file)\n\n![]()\n\n### 4\\. 补贴“寄生”\n\n除了上述款项外，旗人还享有其他福利。满洲和蒙古八旗每年可获两次马匹饲料配给，以实物或银两形式发放。自1724年(清雍正二年)起，各省则自1729年起，满洲和蒙古旗人还有资格领取用于“红事”(即婚事)和“白事”(即丧事)的专项补助。1735年(雍正十三年)，汉军旗才被纳入此项补助范围，其补助金额依品级而定：骑兵婚事约可获银10两，丧事约20两。并且北京地区的补助标准通常高于其他中国城市。清朝对于需要自行筹备军需装备以参与军事行动的八旗兵还会发放额外补贴。\n\n虽然相关标准直到1780年代才趋于统一，但京旗旗人依然获益更多：他们可一次性领取40两银子，而满洲(今东三省)的驻防旗人可获30两，各省驻防旗人则为20两。由于八旗兵在出征期间仍可照常领取薪饷，由其留守家属代领，他们通常还能获得一笔按月发放的“盐菜银”作为生活补助，外加额外的口粮补贴，类似于清朝入关前的“行粮”制度。此外，出征的八旗军队还会获得额外的战马，以及马匹和骆驼的饲料补贴。这还不包括战功奖励制度，由于因作战英勇而获得清廷的赏赐，金额可达数百甚至数千两白银；同样这也不包括发给伤残士兵或阵亡士兵家属的抚恤金。\n\n![](/api/resources/01a1d02b-b5f2-7284-8ed5-105f7f5c494e/file)\n\n![]()\n\n### 5\\. 高官“寄生”\n\n对于旗人而言，清朝对自己八旗极为优厚的待遇，在旗籍官员身上体现得尤为明显：他们不仅享有更高的俸禄及其他福利，有时还能通过出租圈占土地的房产获得数百两银子的租金收入。满城驻防将军的正式俸禄虽有差异，但通常为180两，与内阁大学士或清朝总督的俸禄相当。此外，他们大多还能领到一笔可观的“印纸银”(最高可达200​​两)，用于支付办公所需的印泥、纸张等费用。旗籍官员往往拥有规模更大的马厩，并因此领取了相应的额外津贴。自雍正朝起，清廷开始向文官发放“养廉银”；这一制度同样适用于旗籍官员，满城驻防将军可领到1000至2500两，副都统则可领到600至800两。\n\n![](/api/resources/01a1d02b-b675-7449-b80f-92a5e4391c8f/file)\n\n![]()\n\n### 6.“寄生”后果\n\n在普通人看来，八旗旗人的生活无疑极具吸引力，简直是令人艳羡的优渥日子。对于大多数旗户而言，其既是清朝统治者的“奴才”，也同样受清朝国家供养，经济上的考量在于按时领取他们的银两和米粮；因为即便他们终日无所事事，毫无生产，也照样能领到国家发的这些钱粮。在清朝统治中国的前100年左右，旗人的生活水平远高于汉人普通家庭；也许这同样能解释，为何在此期间，许多汉人平民希望能被“抬旗”。然而，八旗旗人生活所享有的相对安稳与富足，并未持续太久。\n\n自1680年代(康熙前二十年) 确立了八旗的月俸制度后，旗人所获的固定供养标准, 正如清朝的税额一样, 便未再作过调整。由于物价水平并非一成不变，随着时间推移，这些俸禄所能覆盖的旗户家庭实际生活开支便愈发有限。18世纪清朝通货膨胀加剧，加之旗人俸禄中的“粮”被折算为“银”发放（即“折银”），使得旗人的经济困境雪上加霜。此外，随着旗人人口增长，能谋得实职并领取薪俸的男八旗愈来愈少，导致出现了一个庞大的“闲散”群体(即“苏拉”），他们只能依靠家中其他成员的收入来维持生计。到了18世纪中期，旗人的经济生活已不再稳固，许多旗人家庭陷入了困顿，甚至生活在贫困边缘。这一状况引发了清廷官员所称的“八旗生计问题”，成为清朝中期国家治理中的一大难题。\n\n---\n\n## 《特权篇》\n\n![](/api/resources/01a1d02b-b40d-74cc-8dbe-e8cc31390152/file)\n\n![]()\n\n![](/api/resources/01a1d02b-bb26-761c-a48a-f7469459bb12/file)\n\n![]()\n\n![](/api/resources/01a1d02b-b491-770d-b607-7880ee2481c4/file)\n\n![]()\n\n### 1\\. 法律“特权”\n\n由于“八旗旗人”既是征服者又是军事阶层，管理汉人官僚体系的地方普通官员(如知县、知府等) 对他们并无管辖权，尽管从理论上讲，旗人同样受制于同一套民事法规。正因如此，凡涉及旗人的犯罪或纠纷，在处理方式上通常与仅涉汉人平民的案件有所不同。\n\n由于八旗通常享有免受常规司法起诉的特权，汉人平民在面对可能曾侵害过他们的满人时，很难通过正当途径寻求申诉；而旗人的法律特权也理所当然地助长了他们傲慢自大的态度。有学者指出，八旗驻防城市满城成为了旗人中罪犯和不法之徒的庇护所。在1737年(乾隆二年) 发生于西安满城的一起汉人被杀案件中，据记载，当时的汉人老百姓不敢过问案情，普通士兵也不敢插手干预，以至于“满城之内，竟无法执行法律”。一个世纪后，太原满城里的旗人已到了无法无天的地步，以至于当地汉人居民将满城称为“梁山泊”，这一称呼借用了小说《水浒传》中绿林好汉的藏身之所，意指满城里的旗人始终游离于法律权威的管辖之外。\n\n八旗旗人“免受鞭笞”以及“免受刺字”等特权表明，满洲人的自大与清朝对国家安全的考量，共同塑造了“八旗旗人”的法律特权体系。这也解释了为何在1726年(雍正四年) 取消汉军旗所享有的部分法律豁免时，满洲八旗和蒙古八旗所享有的相关豁免权却得以保留。\n\n![](/api/resources/01a1d02b-b770-72ea-9b8f-549ce4ac54d4/file)\n\n![]()\n\n![](/api/resources/01a1d02b-b800-7697-9331-1a82f43dcf90/file)\n\n![]()\n\n![](/api/resources/01a1d02b-b6e8-72f0-b85f-f1a3558ffebb/file)\n\n![]()\n\n![](/api/resources/01a1d02b-b95a-7541-940f-4731b4e52a6c/file)\n\n![]()\n\n![](/api/resources/01a1d02b-b9f5-76ae-a480-dd0c214f88ed/file)\n\n![]()\n\n### 2\\. 官职“特权”\n\n旗人的职业特权犹如一把双刃剑。有利面在于，他们在获取清廷职位(包括军职与文职)时，所占份额远超其人口比例；与汉人相比，他们获得显赫且握有实权要职的机会也大得多。不利面则在于，他们被禁止在八旗体制或官僚机构之外寻求其它职业。然而在清朝统治长时间段里，旗人所享有的职业红利远胜于身处八旗体制所带来的弊端。虽然清朝并未保证每一个男旗人一份工作，但却承诺保障所有旗人, 无论男女老少、体格强弱的基本生活，即为他们提供衣食住行。1687年(康熙二十六年) 后，满清王朝进一步简化了驻防旗人职位由子嗣承袭的程序，旗人普遍认识到，每个旗户至少需要一个有薪职位，才能维持家庭生计。既然清政府承担了供养所有旗人的责任，那么尽可能地让更多旗人担任实职，显然最符合清朝的国家利益。\n\n清廷曾多次为旗人增设职位、创设新职，并扩大其他岗位名额。换言之，清政府为旗人“创造”了工作，即便这仅仅是为他们发放俸禄的一种“借口”。然而，八旗的失业问题依然存在，且影响远不止个人层面。由于满洲人失业问题日益严重，清廷在18世纪中期采取了清理整顿措施：裁撤了八旗队伍中被认为“不配”任职的人员，大部分被清理出旗的是汉军旗人，转而将职位留给那些被视为“更合格”的旗人，即所谓“真正的”满洲人。直到1860年代，清廷才不再承担满洲人生计的全部责任，正式允许旗人自行寻求其它职业。然而，极少有旗人会自行“出旗”，这恰恰反映了当时清朝八旗“寄生性质”的生活方式已根深蒂固到了何种程度。\n\n八旗的“寄生虫”性质在清朝早已有广泛的共识。一位19世纪的评论家曾精辟地指出：“满洲官员的升迁途径远比汉人官员更快捷，原因在于满洲人数较少，而为他们专设的职位却更多。”此外，还有大量仅限满人担任的职位，这些职位游离于严格的“满汉双轨制”之外，例如内务府、宗人府、理藩院以及盛京(今沈阳) 的官僚机构中的职位，进一步向满洲人倾斜权力天平。另有一些职位，其名额在满汉之间分配不均(如书吏、翻译等职)，或是出于其他(未必总是明言的) 原因，被专门留给了满洲人和蒙古人。\n\n将此类职位称为“满缺”(Manchu slots)，并不意味着它们仅供满洲旗人担任，而是面向满洲和蒙古旗人(尽管在某些情况下，蒙古旗人另有专门配额)。虽然有部分职位专门留给了汉军旗人，但汉军旗在争取常规官职时，通常仍需与旗人体制外的汉人官员展开竞争。此外，在清朝前中期，许多重要的地方官职(亦即“双轨制”行政体系之外) 在正式或非正式层面均由满洲旗人把持。例如，在1667年(康熙六年)之前，汉人被明确禁止担任巡抚和总督等地方高级官职。1680年代以后，尽管汉军旗出任地方官的情况有所减少，这些职位仍主要由满洲人担任；此类情况在西北地区尤为明显，自1666年起，陕西、甘肃及山西的巡抚等职便被规定仅限满洲人担任。此外，在盐政、内务府所属官营工场以及海关等部门，旗人(尤其是“包衣”) 也常被委以重任，而汉人则往往被排斥在外。\n\n在清朝对特权的分配中，可以清楚地看到族群范畴所起的作用：满洲人和蒙古人始终享有优于汉人以及“汉军旗”的待遇。尽管此类种族歧视的言论是在私下场合发表的，但当时对满蒙优待的政策并非秘密，它们不仅会被写入清朝的典章制度，也曾是时人热议的话题。尽管到1800年左右，此类种族偏向似乎有所缓和，但针对众多官职设置“仅限满洲人/首崇满洲”配额的做法，可以被视为满洲统治阶层内心深处始终“防汉”的体现，即满洲统治者对汉人的忠诚度和能力始终心存疑虑。\n\n![](/api/resources/01a1d02b-b8b5-7708-9814-210ed6c7fad8/file)\n\n![]()\n\n![](/api/resources/01a1d02b-ba7e-76e4-aa5c-3a0651b3730c/file)\n\n![]()\n\n![](/api/resources/01a1d02b-bba5-73fc-95d3-08d0beb2e201/file)\n\n![]()\n\n### 3.“寄生”+“特权”汇总\n\n在清朝统治中国的前100年左右，旗人生活的显著特征在于八旗浓厚的军事色彩，以及清朝提供相当程度的经济支持。直到19世纪，旗人每月都能领到银饷；只要善于持家，他们所获的粮银便足以维持生计。八旗确实没什么可抱怨的，也很少听到他们有怨言。即便清后期他们的俸饷已不足以覆盖他们日常的开销，以至于旗人群体在经济上已不再具有优势(事实上，许多旗人陷入了贫困)，但这种“依附”于国家的状态，依然是界定八旗群体的核心特征。借用德国历史学家梅谷(Franz Michael) 的话来说：即便旗人连箭都拉不动，他们也总能有一碗饭吃。虽然后来这碗也确实成了他们讨饭的碗，但“铁杆庄稼”依然是旗人身份的明确标志。\n\n到了清末，正如阿拉伯哲学家伊本·赫勒敦(Ibn Khaldun) 那充满讽刺意味的预言，那些被视为满人衰落标志的特征，恰恰也成了他们身份的标签。当时人们眼中的“旗人”(尽管满洲统治阶层依然相当富裕), 不仅未因其优渥的生活而令人艳羡，反而被普遍视为“既穷又横”。一位19世纪末的西方人士曾对八旗因“特权心态”而滋生的傲慢与懒惰，作出了令人印象深刻的描述：\n\n> 谈及满人(其与汉人完全隔离)，我能就居住在北京、广州、福州、南京和镇江这几处的满洲人发表看法。除北京外，各地的满人皆属“旗人”；即一个享有特权的军事阶层，他们携家眷居住在驻防满城内，周遭民众所操的方言与他们说的话也截然不同。旗人的生活方式傲慢且封闭；其本性难免受到周边环境的影响，正如驻扎在石勒苏益格的普鲁士驻军，或是驻扎在波兰的俄国驻军，即便他们军事纪律严明，也总是表露出征服者的姿态。若要概括满人的主要特征，我认为是懒惰与傲慢的结合；然而，在与洋人建立“师生”关系时，比起汉人(其不跪舔洋人)，满人的举止显然又少了几分自命不凡之感，反而倒展现出一种谦卑之姿。\n\n如果旗人“寄生虫”与“特权层”性质的生活对于局外人而言如此显而易见，那么这些恐怕也难逃本土汉人的意识，尤其考虑到在1860年代爆发的“太平天国”运动中，八旗驻防满城曾是太平军血腥屠戮的目标地。也有证据表明，英国人倾向于将满人在中国的地位，与他们自己在印度的情况相类比。诚然，在清末之前，史料中几乎找不到中国人指责满人“懒惰”或“傲慢”的言论；因为对于汉人平民而言，若将此类观点诉诸笔端，无异于自决死刑。当时汉人百姓的“反man”言论，大多以隐晦的方式表达；唯有像太平天国那样英勇的起义者，他们为了民族光复无畏生死，才敢公然发表此类言论。	<blockquote data-source-line="0">\n<p data-source-line="0"><a href="https://www.zhihu.com/question/715577762/answer/2095239383058329672?share_code=HQMHsGghQlGW&amp;utm_psn=2100608642760303847">国家为何还不修清史？</a></p>\n</blockquote>\n<p data-source-line="2">关于国内清史修不修得出来，我觉得各位不用特别在意。因为关于清史这事儿，美国人已经给咱们修得很详实了。今天我想通过引用美国历史学家欧立德(Mark C. Elliott) 的代表作《满洲之道: 中国晚期帝制八旗与族群认同》，系统性地向大家介绍一下清朝八旗“寄生虫”与“特权层”的成分和性质。并以此请各位自行斟酌为什么国内清史修不出来，以及为什么美国人就能写出跟咱们版本完全相反的“清史”。</p>\n<p data-source-line="4">注意，我这里说“寄生虫”，不是故意要抹黑咱们八旗。是因为“寄生”和“特权”这些事在清朝是公知----是八旗旗人自己、本土汉人普通民众以及外国学者观察者等, 一切国内外人士都心知肚明的事儿。</p>\n<p data-source-line="6"><img src="/api/resources/01a1d02b-b1a3-7380-8042-5d0e52b57151/file" alt=""></p>\n<p data-source-line="9">[美]欧立德(Mark C. Elliott) 《满洲之道: 中国晚期帝制八旗与族群认同》</p>\n<p data-source-line="11"><img src="/api/resources/01a1d02b-b039-7668-ac08-a8f0b48de6b5/file" alt=""></p>\n<p data-source-line="13"><img src="" alt=""></p>\n<p data-source-line="15">清朝八旗不仅在财政收入方面享有特权，旗人还享有诸多法律与职业上的特殊权利，而这些“特权”进一步凸显了本土汉人与满洲人在社会--军事阶层之间的各项隔离。德国历史学家弗兰克(Franke) 指出，征服政权往往会持续保留各种区分机制；有学者近期亦评论道：“尽管汉人与满洲人之间关系错综复杂，但他们始终属于两个不同的世界。在制度、法律和财政层面，满汉之间彼此独立，各自拥有独立的法规体系。” 本文将讲述“八旗旗人”在制度、法律和经济方面的各类特权。</p>\n<hr>\n<h2 id="%E3%80%8A%E5%AF%84%E7%94%9F%E7%AF%87%E3%80%8B" tabindex="-1" data-source-line="19"><a class="header-anchor" href="#%E3%80%8A%E5%AF%84%E7%94%9F%E7%AF%87%E3%80%8B" aria-hidden="true">#</a> 《寄生篇》</h2>\n<p data-source-line="21"><img src="/api/resources/01a1d02b-b0ae-77b4-9bd9-5e7844efb76f/file" alt=""></p>\n<p data-source-line="23"><img src="" alt=""></p>\n<p data-source-line="25"><img src="/api/resources/01a1d02b-af51-76e1-8cbf-4d6243fd6237/file" alt=""></p>\n<p data-source-line="27"><img src="" alt=""></p>\n<p data-source-line="29"><img src="/api/resources/01a1d02b-b121-71d0-adec-3a1263ed50d9/file" alt=""></p>\n<p data-source-line="31"><img src="" alt=""></p>\n<h3 id="1.-%E6%94%B6%E5%85%A5%E2%80%9C%E5%AF%84%E7%94%9F%E2%80%9D" tabindex="-1" data-source-line="33"><a class="header-anchor" href="#1.-%E6%94%B6%E5%85%A5%E2%80%9C%E5%AF%84%E7%94%9F%E2%80%9D" aria-hidden="true">#</a> <strong>1.</strong>  <strong>收入“寄生”</strong></h3>\n<p data-source-line="35">旗人生活需要钱；清廷会以白银的形式发放给他们，或从国库中借款给他们。在衣食住行方面，清廷通过发放定额粮食来满足旗人的日常需求，粮食通常来自北京周边的粮仓，或是(对于驻防八旗而言) 来自各省的征粮配额；此外，清廷还在北京及各省的满城中为旗人修建并维护住所。清廷还负责为八旗配备武器，每个旗人的装备均由官府出资购置。部分旗人被要求配马；清廷会按规定数量提供马匹，但此仅限满洲与蒙古八旗，并额外发放购买草料(如干草与豆类) 的补贴。如果旗人因出行、婚丧嫁娶等事宜产生额外的开销，清廷也会提供专项补助。而当旗人因故深陷债务时，即便已有朝廷的各项经济援助，官府仍会提供必要资金，以助八旗偿还债权人。由于清朝为八旗提供了从生到死的全方位供养，套用当时形容所有领取官俸者的一种形象说法，即八旗中人皆是“吃皇家俸禄”的。到了清后期，旗人也同样自我定位，他们自称是“吃皇粮”的人。</p>\n<p data-source-line="37">清朝八旗所获得的财政支持，从理论上讲主要有两种形式：一是银饷与粮饷，二是财产赏赐。银饷(军官所得者称为“俸银”，士兵所得者称为“兵饷”) 按月发放。旗人所领银两金额取决于其品级及驻防地点；通常京师旗人每月可领银四两，而其它中国城市的驻防旗人则领银三两。粮饷数额亦因品级而异，通常于春秋两季分两次发放。在京师，侍卫亲军、领催及八旗骑兵的标准年粮额为22石2斗。驻防各省的旗人，其粮饷数额虽因地而异，但通常约为每年30石，即每月2.5石。在粮饷中，一部分以实物粮食的形式发放，其余部分则折算为等值的银两进行支付。</p>\n<p data-source-line="39"><img src="/api/resources/01a1d02b-b2ba-71e7-85f3-a352eacbaf97/file" alt=""></p>\n<p data-source-line="41"><img src="" alt=""></p>\n<p data-source-line="43"><img src="/api/resources/01a1d02b-b386-7363-b51d-f9a59afedc5f/file" alt=""></p>\n<p data-source-line="45"><img src="" alt=""></p>\n<p data-source-line="47"><img src="/api/resources/01a1d02b-b241-7208-ab62-1f34e5a3b125/file" alt=""></p>\n<p data-source-line="49"><img src="" alt=""></p>\n<h3 id="2.-%E5%9C%9F%E5%9C%B0%E2%80%9C%E5%AF%84%E7%94%9F%E2%80%9D" tabindex="-1" data-source-line="51"><a class="header-anchor" href="#2.-%E5%9C%9F%E5%9C%B0%E2%80%9C%E5%AF%84%E7%94%9F%E2%80%9D" aria-hidden="true">#</a> 2. 土地“寄生”</h3>\n<p data-source-line="53">事实上，清朝最初设想以“圈地”(即土地赏赐) 而非银饷或粮饷，作为支撑八旗的主要经济来源。在满清征服过程中，北京周边被圈占的土地多达1400多万亩，其中大部分被分给了满洲贵族，也有大片土地分拨给八旗各旗，用于供养普通旗户。在清初，凡占领北京且在旗籍册上的男八旗，均可根据其品级的高低，被清廷赏赐30亩或更多的土地。这些旗地享有免税特权，且因其不可转让(仅限在旗人内部买卖)，成为旗人永久的收入来源。土地耕作多由汉人农奴或佃户承担，而八旗庄园的管理事务通常由一名同样是汉人的“庄头”负责。</p>\n<p data-source-line="55">然而，八旗的经济支柱在清朝成立之初的二十年后便开始瓦解。首先，“诚实”与“公允”也许并非是当时八旗旗人所具备的品质；这导致了无数针对汉人农奴的残酷虐待，这些汉人在极其悲惨的劳动条件下，最终选择了逃离被八旗圈占的土地。与此同时，这些八旗管理者也并不介意欺瞒他们的满洲主子。因此，只有那些有充裕时间和精力密切监管自家田产的旗人，才能真正得站稳地主之位。然而，当时大多数旗人都奔赴在前线忙着征服明朝。于是没过多久，地租收入便断绝，旗人地主要么失去了对土地的掌控，要么干脆任由土地荒废；最终，这些土地流回市场，重新落入汉人手中。</p>\n<p data-source-line="57">清廷曾多次颁布禁令，禁止买卖旗地，这恰恰证明了清朝根本无法阻止旗地的流失。尽管部分非法出售的“旗地”后来被收回，但对于普通旗人而言，旗地制度(这与日本江户时期针对武士阶层所设的类似制度形成鲜明对比) 始终未能如清朝统治者所愿，成为解决他们生计问题的长久之计。八旗旗人对清朝持续提供的钱粮依赖程度反而日益加深。到了18世纪中叶，八旗已因其游手好闲、寄生虫式的生活方式而“声名狼藉”；他们日益奢靡的生活习惯，与他们愈发贫困的境况之间显得愈发格格不入。</p>\n<p data-source-line="59"><img src="/api/resources/01a1d02b-b579-7150-b852-66d48004cfe9/file" alt=""></p>\n<p data-source-line="61"><img src="" alt=""></p>\n<p data-source-line="63"><img src="/api/resources/01a1d02b-b31f-7505-8247-05827faea653/file" alt=""></p>\n<p data-source-line="65"><img src="" alt=""></p>\n<h3 id="3.-%E7%B2%AE%E9%A3%9F%E2%80%9C%E5%AF%84%E7%94%9F%E2%80%9D" tabindex="-1" data-source-line="67"><a class="header-anchor" href="#3.-%E7%B2%AE%E9%A3%9F%E2%80%9C%E5%AF%84%E7%94%9F%E2%80%9D" aria-hidden="true">#</a> 3. 粮食“寄生”</h3>\n<p data-source-line="69">为了在一定程度上平衡京师八旗与各地驻防旗人之间的差异，对于那些未被授予圈占土地的旗人，清廷又发放了一项额外的补助----旨在补偿这些旗人因无地而损失的收入。此补助被称为“口粮”，其标准统一设定为每人每月1/4斛。领取“口粮”的旗人包括1647年(明永历元年) 后占领明朝首都北京的八旗兵，以及几乎所有驻防在中国各地的八旗兵。若按每户10口人计算，这项“口粮”补助相当可观，每月达2.5“石”，全年共计30“石”米，相当于又一份全年的基本粮饷。因此，我们可以总结----在北京，一名普通旗兵的收入构成包括：土地收益(仅此一项便足以优渥地供养全家)、银饷，以及按其银饷数额比例发放的“饷米”补助。驻防旗人若未被分配土地，境况或许稍逊，但仍能领取银饷、粮饷以及“口粮”补助。</p>\n<p data-source-line="71"><img src="/api/resources/01a1d02b-b4ff-7248-962b-a160331803a9/file" alt=""></p>\n<p data-source-line="73"><img src="" alt=""></p>\n<h3 id="4.-%E8%A1%A5%E8%B4%B4%E2%80%9C%E5%AF%84%E7%94%9F%E2%80%9D" tabindex="-1" data-source-line="75"><a class="header-anchor" href="#4.-%E8%A1%A5%E8%B4%B4%E2%80%9C%E5%AF%84%E7%94%9F%E2%80%9D" aria-hidden="true">#</a> 4. 补贴“寄生”</h3>\n<p data-source-line="77">除了上述款项外，旗人还享有其他福利。满洲和蒙古八旗每年可获两次马匹饲料配给，以实物或银两形式发放。自1724年(清雍正二年)起，各省则自1729年起，满洲和蒙古旗人还有资格领取用于“红事”(即婚事)和“白事”(即丧事)的专项补助。1735年(雍正十三年)，汉军旗才被纳入此项补助范围，其补助金额依品级而定：骑兵婚事约可获银10两，丧事约20两。并且北京地区的补助标准通常高于其他中国城市。清朝对于需要自行筹备军需装备以参与军事行动的八旗兵还会发放额外补贴。</p>\n<p data-source-line="79">虽然相关标准直到1780年代才趋于统一，但京旗旗人依然获益更多：他们可一次性领取40两银子，而满洲(今东三省)的驻防旗人可获30两，各省驻防旗人则为20两。由于八旗兵在出征期间仍可照常领取薪饷，由其留守家属代领，他们通常还能获得一笔按月发放的“盐菜银”作为生活补助，外加额外的口粮补贴，类似于清朝入关前的“行粮”制度。此外，出征的八旗军队还会获得额外的战马，以及马匹和骆驼的饲料补贴。这还不包括战功奖励制度，由于因作战英勇而获得清廷的赏赐，金额可达数百甚至数千两白银；同样这也不包括发给伤残士兵或阵亡士兵家属的抚恤金。</p>\n<p data-source-line="81"><img src="/api/resources/01a1d02b-b5f2-7284-8ed5-105f7f5c494e/file" alt=""></p>\n<p data-source-line="83"><img src="" alt=""></p>\n<h3 id="5.-%E9%AB%98%E5%AE%98%E2%80%9C%E5%AF%84%E7%94%9F%E2%80%9D" tabindex="-1" data-source-line="85"><a class="header-anchor" href="#5.-%E9%AB%98%E5%AE%98%E2%80%9C%E5%AF%84%E7%94%9F%E2%80%9D" aria-hidden="true">#</a> 5. 高官“寄生”</h3>\n<p data-source-line="87">对于旗人而言，清朝对自己八旗极为优厚的待遇，在旗籍官员身上体现得尤为明显：他们不仅享有更高的俸禄及其他福利，有时还能通过出租圈占土地的房产获得数百两银子的租金收入。满城驻防将军的正式俸禄虽有差异，但通常为180两，与内阁大学士或清朝总督的俸禄相当。此外，他们大多还能领到一笔可观的“印纸银”(最高可达200​​两)，用于支付办公所需的印泥、纸张等费用。旗籍官员往往拥有规模更大的马厩，并因此领取了相应的额外津贴。自雍正朝起，清廷开始向文官发放“养廉银”；这一制度同样适用于旗籍官员，满城驻防将军可领到1000至2500两，副都统则可领到600至800两。</p>\n<p data-source-line="89"><img src="/api/resources/01a1d02b-b675-7449-b80f-92a5e4391c8f/file" alt=""></p>\n<p data-source-line="91"><img src="" alt=""></p>\n<h3 id="6.%E2%80%9C%E5%AF%84%E7%94%9F%E2%80%9D%E5%90%8E%E6%9E%9C" tabindex="-1" data-source-line="93"><a class="header-anchor" href="#6.%E2%80%9C%E5%AF%84%E7%94%9F%E2%80%9D%E5%90%8E%E6%9E%9C" aria-hidden="true">#</a> 6.“寄生”后果</h3>\n<p data-source-line="95">在普通人看来，八旗旗人的生活无疑极具吸引力，简直是令人艳羡的优渥日子。对于大多数旗户而言，其既是清朝统治者的“奴才”，也同样受清朝国家供养，经济上的考量在于按时领取他们的银两和米粮；因为即便他们终日无所事事，毫无生产，也照样能领到国家发的这些钱粮。在清朝统治中国的前100年左右，旗人的生活水平远高于汉人普通家庭；也许这同样能解释，为何在此期间，许多汉人平民希望能被“抬旗”。然而，八旗旗人生活所享有的相对安稳与富足，并未持续太久。</p>\n<p data-source-line="97">自1680年代(康熙前二十年) 确立了八旗的月俸制度后，旗人所获的固定供养标准, 正如清朝的税额一样, 便未再作过调整。由于物价水平并非一成不变，随着时间推移，这些俸禄所能覆盖的旗户家庭实际生活开支便愈发有限。18世纪清朝通货膨胀加剧，加之旗人俸禄中的“粮”被折算为“银”发放（即“折银”），使得旗人的经济困境雪上加霜。此外，随着旗人人口增长，能谋得实职并领取薪俸的男八旗愈来愈少，导致出现了一个庞大的“闲散”群体(即“苏拉”），他们只能依靠家中其他成员的收入来维持生计。到了18世纪中期，旗人的经济生活已不再稳固，许多旗人家庭陷入了困顿，甚至生活在贫困边缘。这一状况引发了清廷官员所称的“八旗生计问题”，成为清朝中期国家治理中的一大难题。</p>\n<hr>\n<h2 id="%E3%80%8A%E7%89%B9%E6%9D%83%E7%AF%87%E3%80%8B" tabindex="-1" data-source-line="101"><a class="header-anchor" href="#%E3%80%8A%E7%89%B9%E6%9D%83%E7%AF%87%E3%80%8B" aria-hidden="true">#</a> 《特权篇》</h2>\n<p data-source-line="103"><img src="/api/resources/01a1d02b-b40d-74cc-8dbe-e8cc31390152/file" alt=""></p>\n<p data-source-line="105"><img src="" alt=""></p>\n<p data-source-line="107"><img src="/api/resources/01a1d02b-bb26-761c-a48a-f7469459bb12/file" alt=""></p>\n<p data-source-line="109"><img src="" alt=""></p>\n<p data-source-line="111"><img src="/api/resources/01a1d02b-b491-770d-b607-7880ee2481c4/file" alt=""></p>\n<p data-source-line="113"><img src="" alt=""></p>\n<h3 id="1.-%E6%B3%95%E5%BE%8B%E2%80%9C%E7%89%B9%E6%9D%83%E2%80%9D" tabindex="-1" data-source-line="115"><a class="header-anchor" href="#1.-%E6%B3%95%E5%BE%8B%E2%80%9C%E7%89%B9%E6%9D%83%E2%80%9D" aria-hidden="true">#</a> 1. 法律“特权”</h3>\n<p data-source-line="117">由于“八旗旗人”既是征服者又是军事阶层，管理汉人官僚体系的地方普通官员(如知县、知府等) 对他们并无管辖权，尽管从理论上讲，旗人同样受制于同一套民事法规。正因如此，凡涉及旗人的犯罪或纠纷，在处理方式上通常与仅涉汉人平民的案件有所不同。</p>\n<p data-source-line="119">由于八旗通常享有免受常规司法起诉的特权，汉人平民在面对可能曾侵害过他们的满人时，很难通过正当途径寻求申诉；而旗人的法律特权也理所当然地助长了他们傲慢自大的态度。有学者指出，八旗驻防城市满城成为了旗人中罪犯和不法之徒的庇护所。在1737年(乾隆二年) 发生于西安满城的一起汉人被杀案件中，据记载，当时的汉人老百姓不敢过问案情，普通士兵也不敢插手干预，以至于“满城之内，竟无法执行法律”。一个世纪后，太原满城里的旗人已到了无法无天的地步，以至于当地汉人居民将满城称为“梁山泊”，这一称呼借用了小说《水浒传》中绿林好汉的藏身之所，意指满城里的旗人始终游离于法律权威的管辖之外。</p>\n<p data-source-line="121">八旗旗人“免受鞭笞”以及“免受刺字”等特权表明，满洲人的自大与清朝对国家安全的考量，共同塑造了“八旗旗人”的法律特权体系。这也解释了为何在1726年(雍正四年) 取消汉军旗所享有的部分法律豁免时，满洲八旗和蒙古八旗所享有的相关豁免权却得以保留。</p>\n<p data-source-line="123"><img src="/api/resources/01a1d02b-b770-72ea-9b8f-549ce4ac54d4/file" alt=""></p>\n<p data-source-line="125"><img src="" alt=""></p>\n<p data-source-line="127"><img src="/api/resources/01a1d02b-b800-7697-9331-1a82f43dcf90/file" alt=""></p>\n<p data-source-line="129"><img src="" alt=""></p>\n<p data-source-line="131"><img src="/api/resources/01a1d02b-b6e8-72f0-b85f-f1a3558ffebb/file" alt=""></p>\n<p data-source-line="133"><img src="" alt=""></p>\n<p data-source-line="135"><img src="/api/resources/01a1d02b-b95a-7541-940f-4731b4e52a6c/file" alt=""></p>\n<p data-source-line="137"><img src="" alt=""></p>\n<p data-source-line="139"><img src="/api/resources/01a1d02b-b9f5-76ae-a480-dd0c214f88ed/file" alt=""></p>\n<p data-source-line="141"><img src="" alt=""></p>\n<h3 id="2.-%E5%AE%98%E8%81%8C%E2%80%9C%E7%89%B9%E6%9D%83%E2%80%9D" tabindex="-1" data-source-line="143"><a class="header-anchor" href="#2.-%E5%AE%98%E8%81%8C%E2%80%9C%E7%89%B9%E6%9D%83%E2%80%9D" aria-hidden="true">#</a> 2. 官职“特权”</h3>\n<p data-source-line="145">旗人的职业特权犹如一把双刃剑。有利面在于，他们在获取清廷职位(包括军职与文职)时，所占份额远超其人口比例；与汉人相比，他们获得显赫且握有实权要职的机会也大得多。不利面则在于，他们被禁止在八旗体制或官僚机构之外寻求其它职业。然而在清朝统治长时间段里，旗人所享有的职业红利远胜于身处八旗体制所带来的弊端。虽然清朝并未保证每一个男旗人一份工作，但却承诺保障所有旗人, 无论男女老少、体格强弱的基本生活，即为他们提供衣食住行。1687年(康熙二十六年) 后，满清王朝进一步简化了驻防旗人职位由子嗣承袭的程序，旗人普遍认识到，每个旗户至少需要一个有薪职位，才能维持家庭生计。既然清政府承担了供养所有旗人的责任，那么尽可能地让更多旗人担任实职，显然最符合清朝的国家利益。</p>\n<p data-source-line="147">清廷曾多次为旗人增设职位、创设新职，并扩大其他岗位名额。换言之，清政府为旗人“创造”了工作，即便这仅仅是为他们发放俸禄的一种“借口”。然而，八旗的失业问题依然存在，且影响远不止个人层面。由于满洲人失业问题日益严重，清廷在18世纪中期采取了清理整顿措施：裁撤了八旗队伍中被认为“不配”任职的人员，大部分被清理出旗的是汉军旗人，转而将职位留给那些被视为“更合格”的旗人，即所谓“真正的”满洲人。直到1860年代，清廷才不再承担满洲人生计的全部责任，正式允许旗人自行寻求其它职业。然而，极少有旗人会自行“出旗”，这恰恰反映了当时清朝八旗“寄生性质”的生活方式已根深蒂固到了何种程度。</p>\n<p data-source-line="149">八旗的“寄生虫”性质在清朝早已有广泛的共识。一位19世纪的评论家曾精辟地指出：“满洲官员的升迁途径远比汉人官员更快捷，原因在于满洲人数较少，而为他们专设的职位却更多。”此外，还有大量仅限满人担任的职位，这些职位游离于严格的“满汉双轨制”之外，例如内务府、宗人府、理藩院以及盛京(今沈阳) 的官僚机构中的职位，进一步向满洲人倾斜权力天平。另有一些职位，其名额在满汉之间分配不均(如书吏、翻译等职)，或是出于其他(未必总是明言的) 原因，被专门留给了满洲人和蒙古人。</p>\n<p data-source-line="151">将此类职位称为“满缺”(Manchu slots)，并不意味着它们仅供满洲旗人担任，而是面向满洲和蒙古旗人(尽管在某些情况下，蒙古旗人另有专门配额)。虽然有部分职位专门留给了汉军旗人，但汉军旗在争取常规官职时，通常仍需与旗人体制外的汉人官员展开竞争。此外，在清朝前中期，许多重要的地方官职(亦即“双轨制”行政体系之外) 在正式或非正式层面均由满洲旗人把持。例如，在1667年(康熙六年)之前，汉人被明确禁止担任巡抚和总督等地方高级官职。1680年代以后，尽管汉军旗出任地方官的情况有所减少，这些职位仍主要由满洲人担任；此类情况在西北地区尤为明显，自1666年起，陕西、甘肃及山西的巡抚等职便被规定仅限满洲人担任。此外，在盐政、内务府所属官营工场以及海关等部门，旗人(尤其是“包衣”) 也常被委以重任，而汉人则往往被排斥在外。</p>\n<p data-source-line="153">在清朝对特权的分配中，可以清楚地看到族群范畴所起的作用：满洲人和蒙古人始终享有优于汉人以及“汉军旗”的待遇。尽管此类种族歧视的言论是在私下场合发表的，但当时对满蒙优待的政策并非秘密，它们不仅会被写入清朝的典章制度，也曾是时人热议的话题。尽管到1800年左右，此类种族偏向似乎有所缓和，但针对众多官职设置“仅限满洲人/首崇满洲”配额的做法，可以被视为满洲统治阶层内心深处始终“防汉”的体现，即满洲统治者对汉人的忠诚度和能力始终心存疑虑。</p>\n<p data-source-line="155"><img src="/api/resources/01a1d02b-b8b5-7708-9814-210ed6c7fad8/file" alt=""></p>\n<p data-source-line="157"><img src="" alt=""></p>\n<p data-source-line="159"><img src="/api/resources/01a1d02b-ba7e-76e4-aa5c-3a0651b3730c/file" alt=""></p>\n<p data-source-line="161"><img src="" alt=""></p>\n<p data-source-line="163"><img src="/api/resources/01a1d02b-bba5-73fc-95d3-08d0beb2e201/file" alt=""></p>\n<p data-source-line="165"><img src="" alt=""></p>\n<h3 id="3.%E2%80%9C%E5%AF%84%E7%94%9F%E2%80%9D%2B%E2%80%9C%E7%89%B9%E6%9D%83%E2%80%9D%E6%B1%87%E6%80%BB" tabindex="-1" data-source-line="167"><a class="header-anchor" href="#3.%E2%80%9C%E5%AF%84%E7%94%9F%E2%80%9D%2B%E2%80%9C%E7%89%B9%E6%9D%83%E2%80%9D%E6%B1%87%E6%80%BB" aria-hidden="true">#</a> 3.“寄生”+“特权”汇总</h3>\n<p data-source-line="169">在清朝统治中国的前100年左右，旗人生活的显著特征在于八旗浓厚的军事色彩，以及清朝提供相当程度的经济支持。直到19世纪，旗人每月都能领到银饷；只要善于持家，他们所获的粮银便足以维持生计。八旗确实没什么可抱怨的，也很少听到他们有怨言。即便清后期他们的俸饷已不足以覆盖他们日常的开销，以至于旗人群体在经济上已不再具有优势(事实上，许多旗人陷入了贫困)，但这种“依附”于国家的状态，依然是界定八旗群体的核心特征。借用德国历史学家梅谷(Franz Michael) 的话来说：即便旗人连箭都拉不动，他们也总能有一碗饭吃。虽然后来这碗也确实成了他们讨饭的碗，但“铁杆庄稼”依然是旗人身份的明确标志。</p>\n<p data-source-line="171">到了清末，正如阿拉伯哲学家伊本·赫勒敦(Ibn Khaldun) 那充满讽刺意味的预言，那些被视为满人衰落标志的特征，恰恰也成了他们身份的标签。当时人们眼中的“旗人”(尽管满洲统治阶层依然相当富裕), 不仅未因其优渥的生活而令人艳羡，反而被普遍视为“既穷又横”。一位19世纪末的西方人士曾对八旗因“特权心态”而滋生的傲慢与懒惰，作出了令人印象深刻的描述：</p>\n<blockquote data-source-line="173">\n<p data-source-line="173">谈及满人(其与汉人完全隔离)，我能就居住在北京、广州、福州、南京和镇江这几处的满洲人发表看法。除北京外，各地的满人皆属“旗人”；即一个享有特权的军事阶层，他们携家眷居住在驻防满城内，周遭民众所操的方言与他们说的话也截然不同。旗人的生活方式傲慢且封闭；其本性难免受到周边环境的影响，正如驻扎在石勒苏益格的普鲁士驻军，或是驻扎在波兰的俄国驻军，即便他们军事纪律严明，也总是表露出征服者的姿态。若要概括满人的主要特征，我认为是懒惰与傲慢的结合；然而，在与洋人建立“师生”关系时，比起汉人(其不跪舔洋人)，满人的举止显然又少了几分自命不凡之感，反而倒展现出一种谦卑之姿。</p>\n</blockquote>\n<p data-source-line="175">如果旗人“寄生虫”与“特权层”性质的生活对于局外人而言如此显而易见，那么这些恐怕也难逃本土汉人的意识，尤其考虑到在1860年代爆发的“太平天国”运动中，八旗驻防满城曾是太平军血腥屠戮的目标地。也有证据表明，英国人倾向于将满人在中国的地位，与他们自己在印度的情况相类比。诚然，在清末之前，史料中几乎找不到中国人指责满人“懒惰”或“傲慢”的言论；因为对于汉人平民而言，若将此类观点诉诸笔端，无异于自决死刑。当时汉人百姓的“反man”言论，大多以隐晦的方式表达；唯有像太平天国那样英勇的起义者，他们为了民族光复无畏生死，才敢公然发表此类言论。</p>\n	47530	\N	1794493886168	1794537215049	\N
01a1d02b-af51-76e1-8cbf-4d6243fd6237	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw021ylti2qoag-20261103101505-x	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw021ylti2qoag-20261103101505-xlzn2px.jpeg	01a1d02b-af51-76e1-8cbf-4d6243fd6237.jpeg	11303	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw021ylti2qoag-20261103101505-xlzn2px.jpeg", "content_sha256": "75f2f45879ab6b75b1f74ab3cea3c1fee9410bf3b84bf9ef37b240d15c23737b", "transformed_path": "01a1d02b-af51-76e1-8cbf-4d6243fd6237.jpeg"}	1794493886298	1794493886298	\N
01a1d02b-afc8-7251-8186-822e05fd5d1e	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw021ywvhnxk80-20261103101505-x	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw021ywvhnxk80-20261103101505-xhw0joc.jpeg	01a1d02b-afc8-7251-8186-822e05fd5d1e.jpeg	60013	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw021ywvhnxk80-20261103101505-xhw0joc.jpeg", "content_sha256": "a28b89e80d847927dd215a9feede2cd6dbd2e7acf53a3f7cb8035e22438b474e", "transformed_path": "01a1d02b-afc8-7251-8186-822e05fd5d1e.jpeg"}	1794493886416	1794493886416	\N
01a1d02b-b039-7668-ac08-a8f0b48de6b5	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw021z1okcvryi-20261103101505-r	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw021z1okcvryi-20261103101505-rrmoa5c.jpeg	01a1d02b-b039-7668-ac08-a8f0b48de6b5.jpeg	40932	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw021z1okcvryi-20261103101505-rrmoa5c.jpeg", "content_sha256": "f63bdf20752a0e4b1362def146eb3ca467d923b0892fb29ac7b85bc6eb0a818a", "transformed_path": "01a1d02b-b039-7668-ac08-a8f0b48de6b5.jpeg"}	1794493886533	1794493886533	\N
01a1d02b-b0ae-77b4-9bd9-5e7844efb76f	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw021z2bdwq4uk-20261103101505-u	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw021z2bdwq4uk-20261103101505-uini9jo.jpeg	01a1d02b-b0ae-77b4-9bd9-5e7844efb76f.jpeg	73383	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw021z2bdwq4uk-20261103101505-uini9jo.jpeg", "content_sha256": "25fdc8ab3b4454b174e08c4d08e4ef0510f1cf71cfdbfa1adb7493ae2d5e28f1", "transformed_path": "01a1d02b-b0ae-77b4-9bd9-5e7844efb76f.jpeg"}	1794493886645	1794493886645	\N
01a1d02b-b121-71d0-adec-3a1263ed50d9	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw021zyy59cf17-20261103101505-z	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw021zyy59cf17-20261103101505-zdgud53.jpeg	01a1d02b-b121-71d0-adec-3a1263ed50d9.jpeg	60564	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw021zyy59cf17-20261103101505-zdgud53.jpeg", "content_sha256": "077a67a374e2bce95c58aa06792967c658e8bfdbc14e75a815fe2e071aecee23", "transformed_path": "01a1d02b-b121-71d0-adec-3a1263ed50d9.jpeg"}	1794493886762	1794493886762	\N
01a1d02b-b1a3-7380-8042-5d0e52b57151	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw02201ue7qjxl-20261103101505-3	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw02201ue7qjxl-20261103101505-35tz0px.jpeg	01a1d02b-b1a3-7380-8042-5d0e52b57151.jpeg	52732	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw02201ue7qjxl-20261103101505-35tz0px.jpeg", "content_sha256": "e22ff34e3d8151787fda7af74e7f13600a8a75a9422a86622a3fc5c1703f6f98", "transformed_path": "01a1d02b-b1a3-7380-8042-5d0e52b57151.jpeg"}	1794493886896	1794493886896	\N
01a1d40c-31d9-7495-8b53-41be8e80ff34	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d40c-272a-728a-9844-4164de145220	Probe-Doc	\N	dqlprobe, demo	3	\N			\N	\N	0	\N	\N	hello		5	\N	1794558931417	1794558931417	1794559184439
01a1e23f-ba01-759e-b5e6-f2f55431138a	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e23f-b83b-7385-87a5-f3221168c5f8	2026-11-02	\N	\N	3	\N	\N	\N	\N	\N	0	\N	\N	‍	\N	\N	\N	1794797189633	1794797189633	1794798933999
01a1e270-0e32-71e9-9e5f-88581359365f	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	DbgProbe	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794800356914	1794800356914	1794800357249
01a1e26f-b740-7045-af15-4944157f556f	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	IdxT	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794800334656	1794800334656	1794801111636
01a1e270-13b2-76e1-bc4b-bfb4fbe20c05	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e270-108e-72ff-840c-656306ca0c4e	IdxA	\N	\N	3	\N			\N	\N	0	\N	\N	\N		0	\N	1794800358322	1794800358322	1794801112246
01a1e274-c676-76df-a2de-fe0b98febc5d	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	IdxT2	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794800666230	1794800666230	1794801113199
01a1e277-fd90-7755-b21f-b70400c88a22	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	IdxT3	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794800876944	1794800876944	1794801113603
01a1d02b-b241-7208-ab62-1f34e5a3b125	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0220cqovubms-20261103101505-p	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0220cqovubms-20261103101505-p58ehtf.jpeg	01a1d02b-b241-7208-ab62-1f34e5a3b125.jpeg	85534	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0220cqovubms-20261103101505-p58ehtf.jpeg", "content_sha256": "e8b197cb3967cfbda821f5e40d7359388cf6cb52bf6622d09676541be8b4262a", "transformed_path": "01a1d02b-b241-7208-ab62-1f34e5a3b125.jpeg"}	1794493887052	1794493887052	\N
01a1d02b-b386-7363-b51d-f9a59afedc5f	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0220ztiw1tbg-20261103101505-t	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0220ztiw1tbg-20261103101505-tc7ra39.jpeg	01a1d02b-b386-7363-b51d-f9a59afedc5f.jpeg	31104	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0220ztiw1tbg-20261103101505-tc7ra39.jpeg", "content_sha256": "4c63cf3e4361d0bce2adbae86d264213291bafa5d48e945cfc8e38dced062884", "transformed_path": "01a1d02b-b386-7363-b51d-f9a59afedc5f.jpeg"}	1794493887375	1794493887375	\N
01a1d02b-b4ff-7248-962b-a160331803a9	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0222toutickb-20261103101505-a	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0222toutickb-20261103101505-ablbcdr.jpeg	01a1d02b-b4ff-7248-962b-a160331803a9.jpeg	85331	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0222toutickb-20261103101505-ablbcdr.jpeg", "content_sha256": "99f6b9e7da25aaa345ce8ac0d3501d54c59da3e360a157c4e36932219f38a99e", "transformed_path": "01a1d02b-b4ff-7248-962b-a160331803a9.jpeg"}	1794493887752	1794493887752	\N
01a1d02b-b675-7449-b80f-92a5e4391c8f	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0223kk0tufyc-20261103101505-d	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0223kk0tufyc-20261103101505-dzkmhsn.jpeg	01a1d02b-b675-7449-b80f-92a5e4391c8f.jpeg	90060	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0223kk0tufyc-20261103101505-dzkmhsn.jpeg", "content_sha256": "9a762bdfe37d62e589ceb72560221abc13eed2aaf270ff05de1a9905bdd48eb9", "transformed_path": "01a1d02b-b675-7449-b80f-92a5e4391c8f.jpeg"}	1794493888126	1794493888126	\N
01a1d02b-b800-7697-9331-1a82f43dcf90	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0224hlziu4ko-20261103101505-o	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0224hlziu4ko-20261103101505-o8hh8vg.jpeg	01a1d02b-b800-7697-9331-1a82f43dcf90.jpeg	75501	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0224hlziu4ko-20261103101505-o8hh8vg.jpeg", "content_sha256": "956e4363fb5a2b7ca9f94dc1e08264d0428c471a4c996eaa785f7914f1afe031", "transformed_path": "01a1d02b-b800-7697-9331-1a82f43dcf90.jpeg"}	1794493888519	1794493888519	\N
01a1d02b-b9f5-76ae-a480-dd0c214f88ed	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0226bvt3rcrn-20261103101506-z	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0226bvt3rcrn-20261103101506-z5wrni8.jpeg	01a1d02b-b9f5-76ae-a480-dd0c214f88ed.jpeg	39663	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0226bvt3rcrn-20261103101506-z5wrni8.jpeg", "content_sha256": "585908c0590f85a3ed9d908f001614b0abad85216901bdea7ace40763277a968", "transformed_path": "01a1d02b-b9f5-76ae-a480-dd0c214f88ed.jpeg"}	1794493889021	1794493889021	\N
01a1d02b-bba5-73fc-95d3-08d0beb2e201	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0226w83crahv-20261103101506-d	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0226w83crahv-20261103101506-dxm3uxz.jpeg	01a1d02b-bba5-73fc-95d3-08d0beb2e201.jpeg	64847	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0226w83crahv-20261103101506-dxm3uxz.jpeg", "content_sha256": "d014a27d9725d6a5b806ee77d3032648efa090fd1d8c27d07a3ebf5434e29db0", "transformed_path": "01a1d02b-bba5-73fc-95d3-08d0beb2e201.jpeg"}	1794493889453	1794493889453	\N
01a1d02b-b2ba-71e7-85f3-a352eacbaf97	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0220lyuzb4bq-20261103101505-9	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0220lyuzb4bq-20261103101505-9r951n9.jpeg	01a1d02b-b2ba-71e7-85f3-a352eacbaf97.jpeg	22556	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0220lyuzb4bq-20261103101505-9r951n9.jpeg", "content_sha256": "a1895427afa83df1e5116450ffadbdb20bb85399d4d5555d56c106b5ce6665e2", "transformed_path": "01a1d02b-b2ba-71e7-85f3-a352eacbaf97.jpeg"}	1794493887172	1794493887172	\N
01a1d02b-b40d-74cc-8dbe-e8cc31390152	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw02217fl78yki-20261103101505-v	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw02217fl78yki-20261103101505-v8ursel.jpeg	01a1d02b-b40d-74cc-8dbe-e8cc31390152.jpeg	20476	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw02217fl78yki-20261103101505-v8ursel.jpeg", "content_sha256": "5983d94cb0393d08398b88c36841f701249e2565aecc10f71b070f370bde7922", "transformed_path": "01a1d02b-b40d-74cc-8dbe-e8cc31390152.jpeg"}	1794493887516	1794493887516	\N
01a1d02b-b579-7150-b852-66d48004cfe9	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0222y136icoz-20261103101505-4	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0222y136icoz-20261103101505-4hepbnx.jpeg	01a1d02b-b579-7150-b852-66d48004cfe9.jpeg	49347	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0222y136icoz-20261103101505-4hepbnx.jpeg", "content_sha256": "4aa0ceb1f8c398f554a77c5c14a2749262539426382794a0a050e53fbbff137e", "transformed_path": "01a1d02b-b579-7150-b852-66d48004cfe9.jpeg"}	1794493887874	1794493887874	\N
01a1d02b-b6e8-72f0-b85f-f1a3558ffebb	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0223y8zhiuq9-20261103101505-d	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0223y8zhiuq9-20261103101505-dw0fhtu.jpeg	01a1d02b-b6e8-72f0-b85f-f1a3558ffebb.jpeg	31464	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0223y8zhiuq9-20261103101505-dw0fhtu.jpeg", "content_sha256": "e2577d0d876d42583b15c15aa3bfe9d3133e7d03bdfe525a6b9037a1a41283d0", "transformed_path": "01a1d02b-b6e8-72f0-b85f-f1a3558ffebb.jpeg"}	1794493888243	1794493888243	\N
01a1d02b-b8b5-7708-9814-210ed6c7fad8	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0225kms5ndco-20261103101506-x	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0225kms5ndco-20261103101506-x56iy6p.jpeg	01a1d02b-b8b5-7708-9814-210ed6c7fad8.jpeg	75646	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0225kms5ndco-20261103101506-x56iy6p.jpeg", "content_sha256": "18b186d4d380d154a42abc765d1c506c40b67e7503b18ae1454f7941f7280636", "transformed_path": "01a1d02b-b8b5-7708-9814-210ed6c7fad8.jpeg"}	1794493888712	1794493888712	\N
01a1d02b-ba7e-76e4-aa5c-3a0651b3730c	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0226is8s4ql0-20261103101506-n	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0226is8s4ql0-20261103101506-nb1culk.jpeg	01a1d02b-ba7e-76e4-aa5c-3a0651b3730c.jpeg	72776	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0226is8s4ql0-20261103101506-nb1culk.jpeg", "content_sha256": "bd79f32c294fe41ff17ed5068e220181e170c3e729c41dbc87d83f8b95dc4030", "transformed_path": "01a1d02b-ba7e-76e4-aa5c-3a0651b3730c.jpeg"}	1794493889159	1794493889159	\N
01a1e277-fed0-73db-ab19-f9bb07b6c1fc	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e277-fd90-7755-b21f-b70400c88a22	index	\N	\N	3	\N	\N	\N	\N	1	1	\N	-1	## 目录\n\n```children\n```\n\n## 最近查看\n\n```recent\n```\n		59	{"index": true}	1794800877264	1794800877264	1794801113603
01a1e279-80a4-738c-93a8-1cb388f7f743	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e279-7ff2-7317-bce4-6d7abb37a05a	index	\N	\N	3	\N	\N	\N	\N	1	1	\N	-1	## 目录\n\n```children\n```\n\n## 最近查看\n\n```recent\n```\n		59	{"index": true}	1794800976036	1794800976036	1794801113955
01a1e279-82cd-7684-88b0-d6a4122dbac4	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e279-7ff2-7317-bce4-6d7abb37a05a	DirA	\N	\N	3	\N			\N	\N	0	\N	\N	\N		0	\N	1794800976589	1794800976589	1794801113955
01a1e261-f1ab-7181-857e-3c59d8390447	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e253-bf21-720a-90f7-5c50d5115eae	index	\N	\N	3	\N	\N	\N	\N	1	1	\N	-1	## 目录\n\n```children\n```\n\n## 最近查看\n\n```recent\n```\n	\N	13	{"index": true}	1794799432107	1794801647272	1795203427357
01a1d02b-b31f-7505-8247-05827faea653	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0220wk59ozuj-20261103101505-1	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0220wk59ozuj-20261103101505-1l115ck.jpeg	01a1d02b-b31f-7505-8247-05827faea653.jpeg	25168	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0220wk59ozuj-20261103101505-1l115ck.jpeg", "content_sha256": "6f672926622a9e7883d1e325674335825505922b077843d2cadc315067aa68eb", "transformed_path": "01a1d02b-b31f-7505-8247-05827faea653.jpeg"}	1794493887270	1794493887270	\N
01a1d02b-d0b2-7357-b726-5fcc0eeb5506	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	微信聊天记录 20261106	\N	\N	3	\N	\N	\N	\N	\N	0	\N	\N	‍\n\n## 图片\n\n![Weixin Image_20261106120808_10_769.jpg](http://qnap.elf-balance.ts.net:7658/file/1794116680872_Weixin_Image_20261106120808_10_769-20261108125259-z9vfjc6.jpg)\n\n![微信图片_20261106115417_717_24.jpg](http://qnap.elf-balance.ts.net:7658/file/1794116690436_微信图片_20261106115417_717_24-20261108125259-8qnualb.jpg)\n\n![微信图片_20261106115417_718_24.jpg](http://qnap.elf-balance.ts.net:7658/file/1794116686705_微信图片_20261106115417_718_24-20261108125259-k2xy3id.jpg)\n\n![微信图片_20261106115417_719_24.jpg](http://qnap.elf-balance.ts.net:7658/file/1794116696644_微信图片_20261106115417_719_24-20261108125259-mgm4t0w.jpg)\n\n![微信图片_20261106115417_720_24.jpg](http://qnap.elf-balance.ts.net:7658/file/1794116680213_微信图片_20261106115417_720_24-20261108125300-apybaar.jpg)\n\n![微信图片_20261106115417_721_24.jpg](http://qnap.elf-balance.ts.net:7658/file/1794116704879_微信图片_20261106115417_721_24-20261108125300-g21ywsa.jpg)\n\n![微信图片_20261106115417_722_24.jpg](http://qnap.elf-balance.ts.net:7658/file/1794116702994_微信图片_20261106115417_722_24-20261108125300-7njz7e1.jpg)\n\n![微信图片_20261106115417_723_24.jpg](http://qnap.elf-balance.ts.net:7658/file/1794116684133_微信图片_20261106115417_723_24-20261108125300-w4mqtoj.jpg)\n\n## 视频\n\n<video controls="controls" src="/api/resources/01a1d02b-d196-7245-ab98-46ac112d3704/file" data-src="/api/resources/01a1d02b-d196-7245-ab98-46ac112d3704/file"></video>\n\n<video controls="controls" src="/api/resources/01a1d02b-d389-7692-a548-844e045cd31f/file" data-src="/api/resources/01a1d02b-d389-7692-a548-844e045cd31f/file"></video>\n\n<video controls="controls" src="/api/resources/01a1d02b-d551-776d-af36-39139f7eb618/file" data-src="/api/resources/01a1d02b-d551-776d-af36-39139f7eb618/file"></video>\n\n<video controls="controls" src="/api/resources/01a1d02b-d65e-7656-9f6f-1791a88616f2/file" data-src="/api/resources/01a1d02b-d65e-7656-9f6f-1791a88616f2/file"></video>\n\n‍	\N	\N	\N	1794493894834	1794493898540	\N
01a1d02b-b491-770d-b607-7880ee2481c4	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0221uuwrvy95-20261103101505-l	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0221uuwrvy95-20261103101505-lh1guah.jpeg	01a1d02b-b491-770d-b607-7880ee2481c4.jpeg	25407	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0221uuwrvy95-20261103101505-lh1guah.jpeg", "content_sha256": "4d9bbd3c56a9e6a9838ba3d1557b2fe3715eb053c8d7322681e1c92102c6c42b", "transformed_path": "01a1d02b-b491-770d-b607-7880ee2481c4.jpeg"}	1794493887640	1794493887640	\N
01a1d02b-b5f2-7284-8ed5-105f7f5c494e	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw02231fjl29pn-20261103101505-m	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw02231fjl29pn-20261103101505-mnrd3m1.jpeg	01a1d02b-b5f2-7284-8ed5-105f7f5c494e.jpeg	51531	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw02231fjl29pn-20261103101505-mnrd3m1.jpeg", "content_sha256": "796361653930e72fd53307779efb57087f355bda0dc79c5c15f9ba02492aa253", "transformed_path": "01a1d02b-b5f2-7284-8ed5-105f7f5c494e.jpeg"}	1794493887994	1794493887994	\N
01a1d02b-b770-72ea-9b8f-549ce4ac54d4	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0224giafi92l-20261103101505-5	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0224giafi92l-20261103101505-5hak4pa.jpeg	01a1d02b-b770-72ea-9b8f-549ce4ac54d4.jpeg	42487	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0224giafi92l-20261103101505-5hak4pa.jpeg", "content_sha256": "899e5ba4decd2bc87d21135cfa6af549bf1e851942dc53238a9dd2a83fe2d61c", "transformed_path": "01a1d02b-b770-72ea-9b8f-549ce4ac54d4.jpeg"}	1794493888376	1794493888376	\N
01a1d02b-bb26-761c-a48a-f7469459bb12	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-aed8-74bb-925b-50c9bd4d6881	Zhihu - 国家为何还不修清史？-mw0226nwsjjx5r-20261103101506-p	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	Zhihu - 国家为何还不修清史？-mw0226nwsjjx5r-20261103101506-p95cwxd.jpeg	01a1d02b-bb26-761c-a48a-f7469459bb12.jpeg	56214	{"filetype": "image/jpeg", "original_path": "Zhihu - 国家为何还不修清史？-mw0226nwsjjx5r-20261103101506-p95cwxd.jpeg", "content_sha256": "0470bc7f9d9df8c3b3fbfad0f1909dc6ae9f72d321e97cfdb93a552ecd589517", "transformed_path": "01a1d02b-bb26-761c-a48a-f7469459bb12.jpeg"}	1794493889326	1794493889326	\N
01a1d02b-d551-776d-af36-39139f7eb618	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1d02b-d0b2-7357-b726-5fcc0eeb5506	c08bcece412f20f425ffb36140888705-20261108125259-fu	\N	\N	4	\N	\N	\N	\N	1	0	\N	\N	c08bcece412f20f425ffb36140888705-20261108125259-fugw4ye.mp4	01a1d02b-d551-776d-af36-39139f7eb618.mp4	324198	{"filetype": "video/mp4", "original_path": "c08bcece412f20f425ffb36140888705-20261108125259-fugw4ye.mp4", "content_sha256": "6ddfd2ded2b851f8ce932aa40230ef97713da021dd5893efca5949c53573a49d", "transformed_path": "01a1d02b-d551-776d-af36-39139f7eb618.mp4"}	1794493896025	1794493896025	\N
01a1e270-108e-72ff-840c-656306ca0c4e	01a1ce57-dceb-77b1-8122-68e49e40e2c2	\N	IdxT	\N	\N	2	\N			\N	\N	0	\N	\N	\N		0	\N	1794800357518	1794800357518	1794801112246
01a1e278-007c-7631-aa08-79b111b5a20b	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e277-fd90-7755-b21f-b70400c88a22	DirKid1	\N	\N	3	\N			\N	\N	0	\N	\N	\N		0	\N	1794800877692	1794800877692	1794801113603
01a1e279-8324-76fd-8d7d-8bf8f76b3333	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e279-7ff2-7317-bce4-6d7abb37a05a	DirB	\N	\N	3	\N			\N	\N	0	\N	\N	\N		0	\N	1794800976676	1794800976676	1794801113955
01a1e278-f8cc-7284-9d81-1bef05f50d6a	01a1ce57-dceb-77b1-8122-68e49e40e2c2	01a1e253-c3cf-7296-889e-69f645b5aa06	index	\N	\N	3	\N	\N	\N	\N	1	1	\N	-1	## 目录\n\n```children\n```\n\n## 最近查看\n\n```recent\n```\n	\N	59	{"index": true}	1794800941260	1794801655258	1795203427357
\.


--
-- Data for Name: object_blobs; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.object_blobs (sha256, size, refcount, created_timestamp) FROM stdin;
d400fd68b5418b182ff3c2ab7e2187f3490479b47f7a135d067a27c3b16e6c7d	17044534	1	1794540710301
58235c016ed6184b404bdffa5b52dc34bc10af151f5ecc23c04f59a0443aef63	1743217	1	1794540739915
db70a6880d780fc0ce8de9e49070ee49951569fbe589cf6ebf309a028229e91c	2082655	1	1794540771583
752df2f8fba4c27d01264dbdbec01e9f9124e1ffafc09e3c143ecc14616f984c	1541699	1	1794540791133
75f2f45879ab6b75b1f74ab3cea3c1fee9410bf3b84bf9ef37b240d15c23737b	11303	1	1794540791755
a28b89e80d847927dd215a9feede2cd6dbd2e7acf53a3f7cb8035e22438b474e	60013	1	1794540794005
f63bdf20752a0e4b1362def146eb3ca467d923b0892fb29ac7b85bc6eb0a818a	40932	1	1794540798104
25fdc8ab3b4454b174e08c4d08e4ef0510f1cf71cfdbfa1adb7493ae2d5e28f1	73383	1	1794540800100
077a67a374e2bce95c58aa06792967c658e8bfdbc14e75a815fe2e071aecee23	60564	1	1794540801104
e22ff34e3d8151787fda7af74e7f13600a8a75a9422a86622a3fc5c1703f6f98	52732	1	1794540803521
e8b197cb3967cfbda821f5e40d7359388cf6cb52bf6622d09676541be8b4262a	85534	1	1794540805450
4c63cf3e4361d0bce2adbae86d264213291bafa5d48e945cfc8e38dced062884	31104	1	1794540806126
99f6b9e7da25aaa345ce8ac0d3501d54c59da3e360a157c4e36932219f38a99e	85331	1	1794540807732
9a762bdfe37d62e589ceb72560221abc13eed2aaf270ff05de1a9905bdd48eb9	90060	1	1794540809308
956e4363fb5a2b7ca9f94dc1e08264d0428c471a4c996eaa785f7914f1afe031	75501	1	1794540812863
585908c0590f85a3ed9d908f001614b0abad85216901bdea7ace40763277a968	39663	1	1794540813607
d014a27d9725d6a5b806ee77d3032648efa090fd1d8c27d07a3ebf5434e29db0	64847	1	1794540814846
a1895427afa83df1e5116450ffadbdb20bb85399d4d5555d56c106b5ce6665e2	22556	1	1794540815349
5983d94cb0393d08398b88c36841f701249e2565aecc10f71b070f370bde7922	20476	1	1794540815715
4aa0ceb1f8c398f554a77c5c14a2749262539426382794a0a050e53fbbff137e	49347	1	1794540816684
e2577d0d876d42583b15c15aa3bfe9d3133e7d03bdfe525a6b9037a1a41283d0	31464	1	1794540817376
18b186d4d380d154a42abc765d1c506c40b67e7503b18ae1454f7941f7280636	75646	1	1794540818649
bd79f32c294fe41ff17ed5068e220181e170c3e729c41dbc87d83f8b95dc4030	72776	1	1794540819958
6f672926622a9e7883d1e325674335825505922b077843d2cadc315067aa68eb	25168	1	1794540821126
4d9bbd3c56a9e6a9838ba3d1557b2fe3715eb053c8d7322681e1c92102c6c42b	25407	1	1794540821599
796361653930e72fd53307779efb57087f355bda0dc79c5c15f9ba02492aa253	51531	1	1794540822574
899e5ba4decd2bc87d21135cfa6af549bf1e851942dc53238a9dd2a83fe2d61c	42487	1	1794540823867
53a35c99ec4c4a0644d8ce32fd40dd03be1ee91614b6709ef304d184aed0004d	77798	1	1794540825156
0470bc7f9d9df8c3b3fbfad0f1909dc6ae9f72d321e97cfdb93a552ecd589517	56214	1	1794540826242
6ddfd2ded2b851f8ce932aa40230ef97713da021dd5893efca5949c53573a49d	324198	1	1794540834423
0c430f2c8d03a1cf3d228f6844f21c4cbe1235590232996a5d9d1ef36ac06826	29	3	1794541380985
90017dfb84d5e9d9219540fc6e741404fecf108f1682968639efdaa8a39f72cb	16	3	1794541069052
ee743c34ba9fde989a280f8c99a07900017c8edd683b516cec4c75b8a5d3195b	524296	1	1794628807688
59fdc92de01f951d2bc437bfd96416ed1269b1239e87f86c431848cd9688ec47	8388616	1	1794628869423
9f962aa499914facb912e8b288ef8959f54703639edf84741016a5617aa53734	300000	1	1794630707934
c0a7e6eccfad88050d81b35c27cfb2d01eaacd74d5988449ff7c4e2487473fe8	52428800	1	1794630782577
\.


--
-- Data for Name: permissions; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.permissions (id, node_id, user_id, permission, created_timestamp) FROM stdin;
\.


--
-- Data for Name: plugin_grants; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.plugin_grants (user_id, plugin_id, capabilities, updated_timestamp) FROM stdin;
\.


--
-- Data for Name: plugin_kv; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.plugin_kv (user_id, plugin_id, key, value, updated_timestamp) FROM stdin;
\.


--
-- Data for Name: plugins; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.plugins (id, name, version, api_version, entry, capabilities, manifest, artifact_prefix, enabled, installed_by, created_timestamp, updated_timestamp) FROM stdin;
\.


--
-- Data for Name: sessions; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.sessions (id, user_id, refresh_token, expire_token, last_refresh_timestamp, active, ip_addr, user_agent, location, type, login_timestamp, logout_timestamp) FROM stdin;
01a1d40f-0bac-71ee-a2ab-6d2d718e4255	01a1ce57-dceb-77b1-8122-68e49e40e2c2	224d7d80785145c0437774eb5942e100b2538ec1845c94d889033ceb4036393b957eed4cb3af5b177b5ae312e6	1797151118249	1794559118249	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:156.0) Gecko/20100101 Firefox/156.0	\N	login	1794559118249	0
01a1e189-9f26-70f3-82ee-66bd6d9d30eb	01a1ce57-dceb-77b1-8122-68e49e40e2c2	91bde17cfb1ae561116b5242406739d1c567eefb8bbd83dd771c00b1b7485e99a44e45eb09762e4774daaf6c28	1797377255206	1794785255206	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:159.0) Gecko/20100101 Firefox/159.0	\N	login	1794785255206	0
01a1e189-aace-74df-80f4-cd31d42e3016	01a1ce57-dceb-77b1-8122-68e49e40e2c2	dcff631b9541621a0ad8fc991e1483ebe6a372a2b7bd61762515a6b214c8a9aaa0e667a34bae8f5b323f76637e	1797377258190	1794785258190	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:159.0) Gecko/20100101 Firefox/159.0	\N	login	1794785258190	0
01a1e189-b85e-73f5-8c7f-6373cac24d6e	01a1ce57-dceb-77b1-8122-68e49e40e2c2	0d80c743042a25b743294a0a16743cafe5fc4550309ce05e480c3b80040477ac763e183fac50bc2290caaf102c	1797377261662	1794785261662	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:159.0) Gecko/20100101 Firefox/159.0	\N	login	1794785261662	0
01a1d250-6c52-728e-a547-0e1e7c1a4757	01a1ce57-dceb-77b1-8122-68e49e40e2c2	6698c7e327ab7cc9bded66f97389cd9add964ad13b367bf52df0f389ff3dcf0adc68af11adbd3311e23ee2d941	1797393542408	1794801542408	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:159.0) Gecko/20100101 Firefox/159.0	\N	login	1794529848401	0
01a1e204-2238-7023-b7db-59572eb3d8bd	01a1ce57-dceb-77b1-8122-68e49e40e2c2	6b815da1849823ec12c1d60618c7fd86d394ce8358e71d63707b524441e355eae5db98b662c122c8190fa5a6b1	1797400960995	1794808960995	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)	\N	login	1794793284147	0
01a1e1f6-7229-7678-80ea-ecb57c3d78ef	01a1ce57-dceb-77b1-8122-68e49e40e2c2	deb733ac52fbff1d59c82ae91a471dd720e3a6b0e55567794d2d29e7240b85bf4d364902dad12175a9560f5289	1797553613962	1794961613962	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:159.0) Gecko/20100101 Firefox/159.0	\N	login	1794792387105	0
01a1ce57-ee49-72e2-afda-b91944ef2874	01a1ce57-dceb-77b1-8122-68e49e40e2c2	915344032e71bafebabd7432bf030fd3df15dd34d69907ee18ef9b07580c0b25db1172eaa120b2163c35b1c1b9	1797583958780	1794991958780	1	172.29.40.3	Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:159.0) Gecko/20100101 Firefox/159.0	\N	login	1794463231561	0
01a1d6a7-5368-715a-a066-7195e0e4def3	01a1ce57-dceb-77b1-8122-68e49e40e2c2	3ea0d2d9c59b333bd226b59d3671b430eb2de4ccd808742ecff1793b84f10a2e1e1eed632cad006df3ae7aedbf	1797194652520	1794602652520	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794602652520	0
01a1d6a7-e1ec-767f-8423-51e4e1ed3f68	01a1ce57-dceb-77b1-8122-68e49e40e2c2	036443bb9b29db1f70fdde8c4cfb702c8b17463466a7a771ed8d222807b923050dc03a2e59ca025f24f78459d7	1797194689004	1794602689004	1	127.0.0.1	curl/8.7.1	\N	login	1794602689004	0
01a1d6a8-0e70-744e-87cf-c43eb3a7aa33	01a1ce57-dceb-77b1-8122-68e49e40e2c2	48d3e981792245a5f424af258800955fb6330a4d4f4148984045c0f4dc7ffc70441e6ff7d751eb6c533a7c037b	1797194700400	1794602700400	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794602700400	0
01a1d6a8-caec-7674-bde9-c7b99121d407	01a1ce57-dceb-77b1-8122-68e49e40e2c2	468b463ca1431553fbb2d20d5fd282b10ad249859c609712bdff85d8050f527ff05ad3d2d3f2430436b7164743	1797194748652	1794602748652	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794602748652	0
01a1d6a9-3ef2-7206-88e7-a3a3ad2686f1	01a1ce57-dceb-77b1-8122-68e49e40e2c2	a1855733d31982875b503cbaa7b3850b778dc274472f40074b1eaeb20ef3052b644475bc4cd474d4d37df8f838	1797194778354	1794602778354	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794602778354	0
01a1d6ae-0f22-7637-8ec6-1f25ffc205ef	01a1ce57-dceb-77b1-8122-68e49e40e2c2	f8c15771c5bb7202f6f949800ede8305906af9f4a9b8c4eb7afa22e64d577c00bf8be1dafd87bbdf77afcaa510	1797195093794	1794603093794	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794603093794	0
01a1d6ae-9ae7-77d9-a6a9-8cc2b6fabe24	01a1ce57-dceb-77b1-8122-68e49e40e2c2	9ab8b865cd69e07263dfdfcd90ec66b823515d7534eb9002e7262b05dd7f7cc1f33702f6d9e37f0f6ab27081e7	1797195129575	1794603129575	1	127.0.0.1	curl/8.7.1	\N	login	1794603129575	0
01a1d6b0-da4a-73ff-b283-337f3c41fd79	01a1ce57-dceb-77b1-8122-68e49e40e2c2	436f1982f765e99492ce966643b9c406a7cc3b0c17f9b51cd20e369c6e52a56e76df4bc232af0b598f2513e063	1797195276874	1794603276874	1	127.0.0.1	curl/8.7.1	\N	login	1794603276874	0
01a1d6b0-fdc8-7482-b490-dd8a2f2a9285	01a1ce57-dceb-77b1-8122-68e49e40e2c2	a7b61e553bc205d9bd1728d612f1b5a71c45dc946fa6f7bb5fab1a85980fc4eec8487777309074664b4e686245	1797195285960	1794603285960	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794603285960	0
01a1d6b4-d6d6-7298-93a0-795a04e390db	01a1ce57-dceb-77b1-8122-68e49e40e2c2	f5f0fbb4cb2cf268a2f3583ab3e3e5cae84976b6ea3e7c238b4336fa21bc48101547180553d7aecf75ca0bc5db	1797195538134	1794603538134	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794603538134	0
01a1d6b5-1c82-7696-bfc5-1fc526f41449	01a1ce57-dceb-77b1-8122-68e49e40e2c2	dcf438eaa855ad1487a987421580fbf557e195f7c214c2c8612dfecaab45bd6e034a9f1fe8235d1bf5e3c7ed69	1797195555970	1794603555970	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794603555970	0
01a1d6b7-eeb6-73e6-b7f6-d47c7840b357	01a1ce57-dceb-77b1-8122-68e49e40e2c2	3eccce51ddcd894cf60318971ad001cdb6411646d61c09ea8bf363a5c47a5aacf066af352274dfd099e5341b76	1797195740854	1794603740854	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794603740854	0
01a1d630-76a1-7055-a910-809b0f33b100	01a1ce57-dceb-77b1-8122-68e49e40e2c2	dc5d2c376049ee1cd998a2acf4c838e60ae52c69b5af82441a5a551dcdb89519aa116df7e82ebf28119c6d8447	1797186862752	1794594862752	1	127.0.0.1	curl/8.7.1	\N	login	1794594862752	0
01a1d632-38b5-74c8-8310-172076813d4c	01a1ce57-dceb-77b1-8122-68e49e40e2c2	d406a17d4aca010f8de152d22ebfb7ee5034b3ab9a1b855776efec025647dcce3c524db0e1bbdef9a79387ebb7	1797186977973	1794594977973	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794594977973	0
01a1d632-68d1-76f8-8dcb-0bd92f9a54e1	01a1ce57-dceb-77b1-8122-68e49e40e2c2	279821a70ce19a93e2833f268459bfdbba4cb7577ffbd31acf242fe5d26338ed41b2807620793cfc863520bec0	1797186990289	1794594990289	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794594990289	0
01a1d632-fa71-7515-a3db-a7e92219ae36	01a1ce57-dceb-77b1-8122-68e49e40e2c2	3dd9ef31022ff0bf306acf385ec559eeed406a4b55c3f799eff42c534876e9ce5d49286217de30d222dfe89d1f	1797187027569	1794595027569	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794595027569	0
01a1d633-483f-7534-914d-eed0edc50d09	01a1ce57-dceb-77b1-8122-68e49e40e2c2	99fdbab95a2a47a6624b453662bea1cff21c84c54e29435a92f9ae5cdf7e2a87dc466415babfc64dd64e3ab1e1	1797187047487	1794595047487	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794595047487	0
01a1d633-a769-70b1-b425-b54bf199cf7f	01a1ce57-dceb-77b1-8122-68e49e40e2c2	e622c5c7732d140a2c37c3b53c473ad64888d0f71170f761bd0f225f9e3c4df1c85ecdd4e0d1b2eddf5434cb48	1797187071849	1794595071849	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794595071849	0
01a1d634-323a-7109-acc9-85d8514716ec	01a1ce57-dceb-77b1-8122-68e49e40e2c2	6e122e4c4ce1cab076efb670577c5acda8c89f5b140c348b209bc92c191fd0113cd927d265b3e42ece3f5f6101	1797187107385	1794595107385	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794595107385	0
01a1d634-a1bc-7787-8154-a0deb492a65d	01a1ce57-dceb-77b1-8122-68e49e40e2c2	9ba1adaabf4eae278583a1805371d433bd1f5d01974dbf96791a1631b0a1ca9820d4df71861f7389f05e6bf384	1797187135930	1794595135930	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794595135930	0
01a1d634-eb44-7638-88dd-755b3ffd0a13	01a1ce57-dceb-77b1-8122-68e49e40e2c2	0e745293fc32c16d2f6252896566e2721b43e99bd2a96246cced0d2dec7a5b0f472eabfb1aff7e1af9913c5c15	1797187154755	1794595154755	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794595154755	0
01a1d635-e5b1-7421-aa69-78c367049f24	01a1ce57-dceb-77b1-8122-68e49e40e2c2	e827a1df1ac7fe8983f74925f3ee40bc57b3a243a2bf0819ec8b3230a52d07b669df4c2096a90f0614f0427f27	1797187218864	1794595218864	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794595218864	0
01a1d6a1-69b5-778b-88ed-b5b645a88116	01a1ce57-dceb-77b1-8122-68e49e40e2c2	30b38291f08907aa6554c107b53137f947a0cb628677f224c17e4e53060cd680a3b55cce57467b80f3384d0c6d	1797194265013	1794602265013	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794602265013	0
01a1d6a1-c0e7-7116-a6b9-d8c65fcc903d	01a1ce57-dceb-77b1-8122-68e49e40e2c2	03646e35c3dceea509fcf6a090c1e8e846be48022f4ae220300b98985a2684cf1bac8f152d17b94881b0ebc90c	1797194287334	1794602287334	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794602287334	0
01a1d6a1-fe18-73b9-9737-d9465bdb3dce	01a1ce57-dceb-77b1-8122-68e49e40e2c2	35a7cdb0f2993572c2be345c8a4c092e74e774a5ca25ac1dc3a943e2ee9f7521f5adc7d08fae503a90b94ad685	1797194303000	1794602303000	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794602303000	0
01a1d6a2-4e49-767c-ac7b-a7735aaf777c	01a1ce57-dceb-77b1-8122-68e49e40e2c2	94d022b305d9fc6c74a16812af745edb431d0663a271892aae03aad822a78fe5e5bfcdc57fc61907d2cf5dee44	1797194323529	1794602323529	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794602323529	0
01a1d6a5-84cd-77e5-a7ee-d1ac6f0115f7	01a1ce57-dceb-77b1-8122-68e49e40e2c2	f744a024bbd5cde04f8da1af4d5ad06b0e95cead7822491581c42068d0fae6c6397d805ce354da80f9fabc3feb	1797194534093	1794602534093	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794602534093	0
01a1d6a6-20f5-7069-b754-b5065861a711	01a1ce57-dceb-77b1-8122-68e49e40e2c2	3c58249d0b6ab28815a9c72e44783c2ac9446781e727fda5c9807fe9bf5313d9428f2a9d1362ba015d3d1169d9	1797194574069	1794602574069	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794602574069	0
01a1d6a6-9b5b-73ff-bad5-4c96d237c029	01a1ce57-dceb-77b1-8122-68e49e40e2c2	40bd3d225da58ca689ef9bf31e5b99fd3a4aee774ed8d5cb4f54ecf293398be10c510d1d8358dafb7a40460bfb	1797194605403	1794602605403	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794602605403	0
01a1d6a6-db9f-7350-9c31-12c6d58a240b	01a1ce57-dceb-77b1-8122-68e49e40e2c2	54a9d315b49b767079c6780f191b39327f3e9ecb21a8a30a46f56f24fbb8a43bbeef43657274a6d60ec6ad40c7	1797194621855	1794602621855	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794602621855	0
01a1d6b8-13cf-7006-97c6-2d620834257c	01a1ce57-dceb-77b1-8122-68e49e40e2c2	3d98fc5e894318aecbab7e2ec7df3545c93fa72c8895a16a3fbe8124f363b8031dbaa0b61568feab29b06ea1ed	1797195750351	1794603750351	1	::1	Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/158.0.8127.18 Safari/537.36	\N	login	1794603750351	0
01a1d831-07f0-7755-9d88-40f8c88daf70	01a1ce57-dceb-77b1-8122-68e49e40e2c2	8b1aa0d6f45b1e3d2708463e9b6840c3a612b14087fe76dd3e961f87b1411f081a314e737bdca8469876e20aef	1797220454384	1794628454384	1	127.0.0.1	curl/8.7.1	\N	login	1794628454384	0
01a1d836-682c-748e-bba4-c8d1967009db	01a1ce57-dceb-77b1-8122-68e49e40e2c2	68974cfd878cb7b81ecf66370abb1fb34d07d8da78026acaf13c1c180faa68690d4db98513a50af2856c8cf0fb	1797220806700	1794628806700	1	127.0.0.1	curl/8.7.1	\N	login	1794628806700	0
\.


--
-- Data for Name: user_oidc_providers; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.user_oidc_providers (id, user_id, provider_name, provider_user_id, created_timestamp, updated_timestamp) FROM stdin;
\.


--
-- Data for Name: user_settings; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.user_settings (user_id, general, editor, advanced) FROM stdin;
01a1ce57-dceb-77b1-8122-68e49e40e2c2	{"style": "default", "theme": "pergame", "locale": "en", "docSize": 1, "hideTOC": false, "darkMode": false, "printMode": false, "view_dock": true, "compactMode": false, "displayTabs": false, "navbarItems": {"theme": true, "search": true, "navigation": true, "breadcrumbNav": false}, "advancedView": false, "primaryColor": -2, "sidebarItems": {"cdn": true, "home": true, "teams": true, "newPage": false, "settings": true, "documents": false, "importation": false, "manageCategories": true}, "readAnnouncements": ["sort-options"], "displayFeatureTags": true, "normalizeFileIcons": false, "datatableItemsCount": 10, "displayUncategorizedResources": true}	{"editorFontSize": 14, "documentAutoSave": true, "documentFontSize": 16, "editorFontFamily": "JetBrains Mono", "editorSpellCheck": true, "documentFontFamily": "Poppins", "documentLineHeight": 1.5, "editorDisplayStats": false, "documentColoredTags": true, "editorSnippetsEnabled": true, "documentShowAttachments": false, "documentTitlesNumbering": false, "documentDefaultThumbnail": false, "documentUseParentThumbnail": false, "editorScrollBeyondLastLine": true, "editorSimplifiedViewOnMobile": true}	{"sortKey": "name", "snippets": [{"id": "!blue", "label": ":::blue\\n${0}\\n:::"}, {"id": "!green", "label": ":::green\\n${0}\\n:::"}, {"id": "!yellow", "label": ":::yellow\\n${0}\\n:::"}, {"id": "!red", "label": ":::red\\n${0}\\n:::"}, {"id": "!grey", "label": ":::grey\\n${0}\\n:::"}, {"id": "!teal", "label": ":::teal\\n${0}\\n:::"}, {"id": "!details", "label": ":::details\\n${0}\\n:::"}, {"id": "!info-red", "label": ":::info-red\\n${0}\\n:::"}, {"id": "!info-blue", "label": ":::info-blue\\n${0}\\n:::"}, {"id": "!info-teal", "label": ":::info-teal\\n${0}\\n:::"}, {"id": "!center", "label": ":::center\\n${0}\\n:::"}, {"id": "!columns", "label": ":::columns\\n${0}\\n:::"}, {"id": "!column", "label": ":::column\\n${0}\\n:::"}, {"id": "!no-print", "label": ":::no-print\\n${0}\\n:::"}, {"id": "!property", "label": ":::property ${0}\\n\\n:::"}, {"id": "!warning", "label": ":::warning ${0}\\n\\n:::"}, {"id": "!frame", "label": ":::frame ${0}\\n\\n:::"}, {"id": "!card", "label": ":::card ${0}\\n\\n:::"}, {"id": "!pannel", "label": ":::pannel ${0}\\n\\n:::"}], "sortOrder": "ascending", "developerMode": false, "stylesInjection": "", "defaultUploadFolder": null, "stylesInjectionEnabled": false, "stylesDocumentsInjection": ""}
\.


--
-- Data for Name: user_totp_recovery_codes; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.user_totp_recovery_codes (id, user_id, code, used, created_timestamp) FROM stdin;
\.


--
-- Data for Name: users; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.users (id, username, firstname, lastname, role, type, avatar, email, password, totp_secret, totp_enabled, totp_forced, created_timestamp, updated_timestamp, password_reset_token, max_file_size, max_total_size) FROM stdin;
01a1ce57-dceb-77b1-8122-68e49e40e2c2	laniakea	\N	\N	1	0	\N	\N	$2b$12$FR2Kon2812Hs2eWewZkw2utS6QQLnb3QvIba3gVd1rWYZSyD.NvCK	\N	f	f	1794463227115	1794463227115	\N	\N	\N
\.


--
-- Name: _sqlx_migrations _sqlx_migrations_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public._sqlx_migrations
    ADD CONSTRAINT _sqlx_migrations_pkey PRIMARY KEY (version);


--
-- Name: api_tokens api_tokens_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.api_tokens
    ADD CONSTRAINT api_tokens_pkey PRIMARY KEY (id);


--
-- Name: api_tokens api_tokens_token_hash_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.api_tokens
    ADD CONSTRAINT api_tokens_token_hash_key UNIQUE (token_hash);


--
-- Name: backup_targets backup_targets_name_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.backup_targets
    ADD CONSTRAINT backup_targets_name_key UNIQUE (name);


--
-- Name: backup_targets backup_targets_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.backup_targets
    ADD CONSTRAINT backup_targets_pkey PRIMARY KEY (id);


--
-- Name: logs logs_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.logs
    ADD CONSTRAINT logs_pkey PRIMARY KEY (id);


--
-- Name: node_invitations node_invitations_invitation_code_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.node_invitations
    ADD CONSTRAINT node_invitations_invitation_code_key UNIQUE (invitation_code);


--
-- Name: node_invitations node_invitations_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.node_invitations
    ADD CONSTRAINT node_invitations_pkey PRIMARY KEY (id);


--
-- Name: node_links node_links_parent_id_node_id_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.node_links
    ADD CONSTRAINT node_links_parent_id_node_id_key UNIQUE (parent_id, node_id);


--
-- Name: node_links node_links_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.node_links
    ADD CONSTRAINT node_links_pkey PRIMARY KEY (id);


--
-- Name: node_views node_views_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.node_views
    ADD CONSTRAINT node_views_pkey PRIMARY KEY (user_id, node_id);


--
-- Name: nodes nodes_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.nodes
    ADD CONSTRAINT nodes_pkey PRIMARY KEY (id);


--
-- Name: object_blobs object_blobs_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.object_blobs
    ADD CONSTRAINT object_blobs_pkey PRIMARY KEY (sha256);


--
-- Name: permissions permissions_node_id_user_id_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.permissions
    ADD CONSTRAINT permissions_node_id_user_id_key UNIQUE (node_id, user_id);


--
-- Name: permissions permissions_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.permissions
    ADD CONSTRAINT permissions_pkey PRIMARY KEY (id);


--
-- Name: plugin_grants plugin_grants_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.plugin_grants
    ADD CONSTRAINT plugin_grants_pkey PRIMARY KEY (user_id, plugin_id);


--
-- Name: plugin_kv plugin_kv_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.plugin_kv
    ADD CONSTRAINT plugin_kv_pkey PRIMARY KEY (user_id, plugin_id, key);


--
-- Name: plugins plugins_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.plugins
    ADD CONSTRAINT plugins_pkey PRIMARY KEY (id);


--
-- Name: sessions sessions_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.sessions
    ADD CONSTRAINT sessions_pkey PRIMARY KEY (id);


--
-- Name: user_oidc_providers user_oidc_providers_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_oidc_providers
    ADD CONSTRAINT user_oidc_providers_pkey PRIMARY KEY (id);


--
-- Name: user_oidc_providers user_oidc_providers_provider_name_provider_user_id_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_oidc_providers
    ADD CONSTRAINT user_oidc_providers_provider_name_provider_user_id_key UNIQUE (provider_name, provider_user_id);


--
-- Name: user_settings user_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_settings
    ADD CONSTRAINT user_settings_pkey PRIMARY KEY (user_id);


--
-- Name: user_totp_recovery_codes user_totp_recovery_codes_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_totp_recovery_codes
    ADD CONSTRAINT user_totp_recovery_codes_pkey PRIMARY KEY (id);


--
-- Name: users users_email_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_email_key UNIQUE (email);


--
-- Name: users users_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);


--
-- Name: users users_username_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_username_key UNIQUE (username);


--
-- Name: idx_api_tokens_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_api_tokens_user ON public.api_tokens USING btree (user_id);


--
-- Name: idx_backup_targets_name; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX idx_backup_targets_name ON public.backup_targets USING btree (name);


--
-- Name: idx_invitations_node; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_invitations_node ON public.node_invitations USING btree (node_id);


--
-- Name: idx_logs_created; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_logs_created ON public.logs USING btree (created_timestamp);


--
-- Name: idx_logs_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_logs_user ON public.logs USING btree (user_id);


--
-- Name: idx_node_links_node; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_node_links_node ON public.node_links USING btree (node_id);


--
-- Name: idx_node_views_user_time; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_node_views_user_time ON public.node_views USING btree (user_id, viewed_at DESC);


--
-- Name: idx_nodes_content_fts; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_nodes_content_fts ON public.nodes USING gin (content_tsv);


--
-- Name: idx_nodes_deleted; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_nodes_deleted ON public.nodes USING btree (deleted_at) WHERE (deleted_at IS NOT NULL);


--
-- Name: idx_nodes_metadata; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_nodes_metadata ON public.nodes USING gin (metadata jsonb_path_ops);


--
-- Name: idx_nodes_parent; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_nodes_parent ON public.nodes USING btree (parent_id);


--
-- Name: idx_nodes_search_fts; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_nodes_search_fts ON public.nodes USING gin (search_tsv);


--
-- Name: idx_nodes_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_nodes_user ON public.nodes USING btree (user_id);


--
-- Name: idx_nodes_user_role; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_nodes_user_role ON public.nodes USING btree (user_id, role);


--
-- Name: idx_oidc_providers_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_oidc_providers_user ON public.user_oidc_providers USING btree (user_id);


--
-- Name: idx_permissions_node; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_permissions_node ON public.permissions USING btree (node_id);


--
-- Name: idx_permissions_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_permissions_user ON public.permissions USING btree (user_id);


--
-- Name: idx_recovery_codes_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_recovery_codes_user ON public.user_totp_recovery_codes USING btree (user_id);


--
-- Name: idx_sessions_ip; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_sessions_ip ON public.sessions USING btree (ip_addr);


--
-- Name: idx_sessions_refresh_token; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_sessions_refresh_token ON public.sessions USING btree (refresh_token);


--
-- Name: idx_sessions_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_sessions_user ON public.sessions USING btree (user_id);


--
-- Name: api_tokens api_tokens_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.api_tokens
    ADD CONSTRAINT api_tokens_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: logs logs_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.logs
    ADD CONSTRAINT logs_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE SET NULL;


--
-- Name: node_invitations node_invitations_node_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.node_invitations
    ADD CONSTRAINT node_invitations_node_id_fkey FOREIGN KEY (node_id) REFERENCES public.nodes(id) ON DELETE CASCADE;


--
-- Name: node_links node_links_node_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.node_links
    ADD CONSTRAINT node_links_node_id_fkey FOREIGN KEY (node_id) REFERENCES public.nodes(id) ON DELETE CASCADE;


--
-- Name: node_links node_links_parent_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.node_links
    ADD CONSTRAINT node_links_parent_id_fkey FOREIGN KEY (parent_id) REFERENCES public.nodes(id) ON DELETE CASCADE;


--
-- Name: node_views node_views_node_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.node_views
    ADD CONSTRAINT node_views_node_id_fkey FOREIGN KEY (node_id) REFERENCES public.nodes(id) ON DELETE CASCADE;


--
-- Name: node_views node_views_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.node_views
    ADD CONSTRAINT node_views_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: nodes nodes_parent_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.nodes
    ADD CONSTRAINT nodes_parent_id_fkey FOREIGN KEY (parent_id) REFERENCES public.nodes(id) ON DELETE CASCADE;


--
-- Name: nodes nodes_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.nodes
    ADD CONSTRAINT nodes_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: permissions permissions_node_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.permissions
    ADD CONSTRAINT permissions_node_id_fkey FOREIGN KEY (node_id) REFERENCES public.nodes(id) ON DELETE CASCADE;


--
-- Name: permissions permissions_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.permissions
    ADD CONSTRAINT permissions_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: plugin_grants plugin_grants_plugin_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.plugin_grants
    ADD CONSTRAINT plugin_grants_plugin_id_fkey FOREIGN KEY (plugin_id) REFERENCES public.plugins(id) ON DELETE CASCADE;


--
-- Name: plugin_grants plugin_grants_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.plugin_grants
    ADD CONSTRAINT plugin_grants_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: plugin_kv plugin_kv_plugin_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.plugin_kv
    ADD CONSTRAINT plugin_kv_plugin_id_fkey FOREIGN KEY (plugin_id) REFERENCES public.plugins(id) ON DELETE CASCADE;


--
-- Name: plugin_kv plugin_kv_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.plugin_kv
    ADD CONSTRAINT plugin_kv_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: plugins plugins_installed_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.plugins
    ADD CONSTRAINT plugins_installed_by_fkey FOREIGN KEY (installed_by) REFERENCES public.users(id) ON DELETE SET NULL;


--
-- Name: sessions sessions_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.sessions
    ADD CONSTRAINT sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: user_oidc_providers user_oidc_providers_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_oidc_providers
    ADD CONSTRAINT user_oidc_providers_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: user_settings user_settings_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_settings
    ADD CONSTRAINT user_settings_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: user_totp_recovery_codes user_totp_recovery_codes_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_totp_recovery_codes
    ADD CONSTRAINT user_totp_recovery_codes_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--

\unrestrict b6CxPpDa4f9ZCtfYdEekVchm4evJbeH1tIkyheyIkCEzxamdtJmZxWJNYfOcjIq

