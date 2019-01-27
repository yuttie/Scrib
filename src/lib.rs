#[macro_use]
extern crate diesel;
extern crate serde;
#[macro_use]
extern crate serde_derive;

pub mod schema;
pub mod models;
pub mod server;

use std::env;

use chrono::prelude::*;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use dotenv::dotenv;
use r2d2;

use self::models::{Scribble, NewScribble, Tag, NewTag, Tagging};


#[derive(Debug)]
pub enum Error {
    DatabaseError(diesel::result::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn establish_connection() -> SqliteConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    SqliteConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

pub fn new_connection_pool() -> Pool<ConnectionManager<SqliteConnection>> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<SqliteConnection>::new(database_url.as_str());
    r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.")
}

pub fn create_scribble<'a>(conn: &SqliteConnection, text: &'a str) -> Result<Scribble> {
    use self::schema::scribbles;

    let now = Utc::now();
    let new_scribble = NewScribble {
        created_at: now.timestamp_nanos(),
        text: text,
    };

    let result = diesel::insert_into(scribbles::table)
        .values(&new_scribble)
        .execute(conn);

    match result {
        Err(e) => {
            Err(Error::DatabaseError(e))
        },
        Ok(_) => {
            let created = diesel::sql_query("SELECT * FROM scribbles WHERE rowid = last_insert_rowid();")
                .get_result(conn)
                .unwrap();
            Ok(created)
        },
    }
}

pub fn create_tag<'a>(conn: &SqliteConnection, text: &'a str) -> Result<Tag> {
    use self::schema::tags;

    let now = Utc::now();
    let new_tag = NewTag {
        created_at: now.timestamp_nanos(),
        text: text,
    };

    let result = diesel::insert_into(tags::table)
        .values(&new_tag)
        .execute(conn);

    match result {
        Err(e) => {
            Err(Error::DatabaseError(e))
        },
        Ok(_) => {
            let created = diesel::sql_query("SELECT * FROM tags WHERE rowid = last_insert_rowid();")
                .get_result(conn)
                .unwrap();
            Ok(created)
        },
    }
}

pub fn tag_scribble<'a>(conn: &SqliteConnection, scribble_id: i64, tag_text: &'a str) -> Result<Tagging> {
    use diesel::sql_types::{BigInt, Text};

    let now = Utc::now();
    let result = diesel::sql_query("INSERT INTO taggings (created_at, scribble_id, tag_id) VALUES (?, ?, (SELECT id FROM tags WHERE text = ?));")
        .bind::<BigInt, _>(now.timestamp_nanos())
        .bind::<BigInt, _>(scribble_id)
        .bind::<Text, _>(tag_text)
        .execute(conn);

    match result {
        Err(e) => {
            Err(Error::DatabaseError(e))
        },
        Ok(_) => {
            let created = diesel::sql_query("SELECT * FROM taggings WHERE rowid = last_insert_rowid();")
                .get_result(conn)
                .unwrap();
            Ok(created)
        },
    }
}

pub fn tags(conn: &SqliteConnection) -> Result<Vec<Tag>> {
    use self::schema::tags::dsl::*;

    let result = tags.load::<Tag>(conn);

    match result {
        Err(e) => {
            Err(Error::DatabaseError(e))
        },
        Ok(selected) => {
            Ok(selected)
        },
    }
}

pub fn list(conn: &SqliteConnection, size: Option<usize>) -> Result<Vec<Scribble>> {
    use self::schema::scribbles::dsl::*;

    let result = scribbles.load::<Scribble>(conn);

    match result {
        Err(e) => {
            Err(Error::DatabaseError(e))
        },
        Ok(selected) => {
            Ok(selected)
        },
    }
}

pub fn tags_of(conn: &SqliteConnection, scribble_id: i64) -> Result<Vec<Tag>> {
    use diesel::sql_types::BigInt;

    let result = diesel::sql_query("SELECT tags.* FROM tags, taggings WHERE taggings.scribble_id = ? AND taggings.tag_id = tag.id;")
        .bind::<BigInt, _>(scribble_id)
        .get_results(conn);

    match result {
        Err(e) => {
            Err(Error::DatabaseError(e))
        },
        Ok(selected) => {
            Ok(selected)
        },
    }
}
