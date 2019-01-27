CREATE TABLE scribbles (
    id         INTEGER PRIMARY KEY NOT NULL,
    created_at BIGINT NOT NULL,
    text       TEXT NOT NULL
);

CREATE TABLE tags (
    id         INTEGER PRIMARY KEY NOT NULL,
    created_at BIGINT NOT NULL,
    text       TEXT UNIQUE NOT NULL
);

CREATE TABLE taggings (
    id          INTEGER PRIMARY KEY NOT NULL,
    created_at  BIGINT NOT NULL,
    scribble_id BIGINT NOT NULL,
    tag_id      BIGINT NOT NULL,
    UNIQUE (scribble_id, tag_id)
);
CREATE INDEX taggings_scribbleid_tagid ON taggings (scribble_id, tag_id);
CREATE INDEX taggings_tagid_scribbleid ON taggings (tag_id, scribble_id);
