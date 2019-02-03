CREATE TEMPORARY TABLE tmp_scribbles AS SELECT * FROM scribbles;
DROP TABLE scribbles;
CREATE TABLE scribbles (
    id         INTEGER PRIMARY KEY NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT,
    text       TEXT NOT NULL
);
INSERT INTO scribbles SELECT id, created_at, NULL AS updated_at, text FROM tmp_scribbles;
DROP TABLE tmp_scribbles;
