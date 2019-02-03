table! {
    scribbles (id) {
        id -> BigInt,
        created_at -> BigInt,
        text -> Text,
    }
}

table! {
    taggings (id) {
        id -> BigInt,
        created_at -> BigInt,
        scribble_id -> BigInt,
        tag_id -> BigInt,
    }
}

table! {
    tags (id) {
        id -> BigInt,
        created_at -> BigInt,
        text -> Text,
    }
}

allow_tables_to_appear_in_same_query!(
    scribbles,
    taggings,
    tags,
);
