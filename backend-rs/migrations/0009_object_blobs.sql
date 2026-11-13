-- Content-addressed blob registry for cross-user dedup.
-- New uploads land at blobs/{sha256} (one copy per distinct bytes);
-- nodes.metadata.content_sha256 points here. refcount tracks live
-- references; reaching 0 deletes the S3 object.
-- Legacy objects ({user_id}/{uuid}.{ext}) stay readable: nodes without
-- content_sha256 keep the old path until the admin migration moves them.

CREATE TABLE object_blobs (
    sha256    CHAR(64) PRIMARY KEY,
    size      BIGINT   NOT NULL,
    refcount  INT      NOT NULL DEFAULT 1,
    created_timestamp BIGINT NOT NULL
);
