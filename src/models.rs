use crate::schema::{scribbles, tags, taggings};

use diesel::{Queryable, QueryableByName, Insertable};


#[derive(Queryable, QueryableByName, Serialize, Deserialize, Debug)]
#[table_name="scribbles"]
pub struct Scribble {
    pub id:         i64,
    pub created_at: i64,
    pub text:       String,
}

#[derive(Insertable, Debug)]
#[table_name="scribbles"]
pub struct NewScribble<'a> {
    pub created_at: i64,
    pub text:       &'a str,
}

#[derive(Queryable, QueryableByName, Serialize, Deserialize, Debug)]
#[table_name="tags"]
pub struct Tag {
    pub id:         i64,
    pub created_at: i64,
    pub text:       String,
}

#[derive(Insertable, Debug)]
#[table_name="tags"]
pub struct NewTag<'a> {
    pub created_at: i64,
    pub text:       &'a str,
}

#[derive(Queryable, QueryableByName, Serialize, Deserialize, Debug)]
#[table_name="taggings"]
pub struct Tagging {
    pub id:          i64,
    pub created_at:  i64,
    pub scribble_id: i64,
    pub tag_id:      i64,
}

#[derive(Insertable, Debug)]
#[table_name="taggings"]
pub struct NewTagging {
    pub created_at:  i64,
    pub scribble_id: i64,
    pub tag_id:      i64,
}
