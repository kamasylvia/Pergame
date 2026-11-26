-- SiYuan-style PDF annotations: selection highlights with comments that
-- live in Pergame (never written into the stored file bytes). Docs link to
-- them via `#annotation:<uuid>`; `annotation_refs` is reconciled from doc
-- bodies the same way `node_refs` is (see refs.rs).
--
-- quads: [{x0,y0,x1,y1}] in PDF default-user-space points (rotation-free,
-- straight from viewport.convertToPdfPoint), 1..64 rects per annotation.
-- color: palette index 0..7 (frontend-owned).
CREATE TABLE pdf_annotations (
    id                UUID PRIMARY KEY,
    user_id           UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    resource_id       UUID NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    page              INT  NOT NULL CHECK (page >= 1),
    quads             JSONB NOT NULL,
    quote             TEXT NOT NULL DEFAULT '',
    comment           TEXT NOT NULL DEFAULT '',
    color             INT  NOT NULL DEFAULT 0 CHECK (color >= 0 AND color < 8),
    created_timestamp BIGINT NOT NULL,
    updated_timestamp BIGINT NOT NULL
);
CREATE INDEX idx_pdf_annots_resource ON pdf_annotations (resource_id);
CREATE INDEX idx_pdf_annots_user ON pdf_annotations (user_id);

CREATE TABLE annotation_refs (
    source_id     UUID NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    annotation_id UUID NOT NULL REFERENCES pdf_annotations(id) ON DELETE CASCADE,
    PRIMARY KEY (source_id, annotation_id)
);
CREATE INDEX idx_annotation_refs_annotation ON annotation_refs (annotation_id);
