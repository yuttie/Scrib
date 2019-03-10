table! {
    scribbles (id) {
        id -> Int8,
        created_at -> Int8,
        updated_at -> Nullable<Int8>,
        text -> Text,
    }
}

table! {
    taggings (id) {
        id -> Int8,
        created_at -> Int8,
        scribble_id -> Int8,
        tag_id -> Int8,
    }
}

table! {
    tags (id) {
        id -> Int8,
        created_at -> Int8,
        text -> Text,
    }
}

allow_tables_to_appear_in_same_query!(
    scribbles,
    taggings,
    tags,
);
