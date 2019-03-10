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
    TagExists,
    AlreadyTagged,
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn establish_connection() -> PgConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

pub fn new_connection_pool() -> Pool<ConnectionManager<PgConnection>> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<PgConnection>::new(database_url.as_str());
    r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.")
}

pub fn create_scribble<'a>(conn: &PgConnection, text: &'a str) -> Result<Scribble> {
    use self::schema::scribbles;

    let now = Utc::now();
    let new_scribble = NewScribble {
        created_at: now.timestamp_nanos(),
        text: text,
    };

    let result = diesel::insert_into(scribbles::table)
        .values(&new_scribble)
        .get_result(conn);

    match result {
        Err(e) => {
            Err(Error::DatabaseError(e))
        },
        Ok(created) => {
            Ok(created)
        },
    }
}

pub fn update_scribble<'a>(conn: &PgConnection, scribble_id: i64, new_text: &'a str) -> Result<Scribble> {
    use self::schema::scribbles::dsl::*;

    let now = Utc::now();
    let result = diesel::update(scribbles.find(scribble_id))
        .set((updated_at.eq(now.timestamp_nanos()),
              text.eq(new_text)))
        .get_result(conn);

    match result {
        Err(e) => {
            Err(Error::DatabaseError(e))
        },
        Ok(updated) => {
            Ok(updated)
        },
    }
}

pub fn delete_scribble<'a>(conn: &PgConnection, scribble_id: i64) -> Result<()> {
    use self::schema::scribbles::dsl::*;

    let result = diesel::delete(scribbles.find(scribble_id))
        .execute(conn);

    match result {
        Err(e) => {
            Err(Error::DatabaseError(e))
        },
        Ok(_) => {
            Ok(())
        },
    }
}

pub fn create_tag<'a>(conn: &PgConnection, text: &'a str) -> Result<Tag> {
    use self::schema::tags;

    let now = Utc::now();
    let new_tag = NewTag {
        created_at: now.timestamp_nanos(),
        text: text,
    };

    let result = diesel::insert_into(tags::table)
        .values(&new_tag)
        .get_result(conn);

    match result {
        Err(e) => {
            use diesel::result::Error as DieselError;
            use diesel::result::DatabaseErrorKind;

            match e {
                DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _) => {
                    Err(Error::TagExists)
                },
                _ => {
                    Err(Error::DatabaseError(e))
                },
            }
        },
        Ok(created) => {
            Ok(created)
        },
    }
}

pub fn tag_scribble<'a>(conn: &PgConnection, scribble_id: i64, tag_text: &'a str) -> Result<Tagging> {
    use diesel::sql_types::{BigInt, Text};

    match create_tag(&conn, &tag_text) {
        Ok(_) | Err(Error::TagExists) => {
            let now = Utc::now();
            let result = diesel::sql_query("INSERT INTO taggings (created_at, scribble_id, tag_id) VALUES ($1, $2, (SELECT id FROM tags WHERE text = $3));")
                .bind::<BigInt, _>(now.timestamp_nanos())
                .bind::<BigInt, _>(scribble_id)
                .bind::<Text, _>(tag_text)
                .get_result(conn);

            match result {
                Err(e) => {
                    use diesel::result::Error as DieselError;
                    use diesel::result::DatabaseErrorKind;

                    match e {
                        DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _) => {
                            Err(Error::AlreadyTagged)
                        },
                        _ => {
                            Err(Error::DatabaseError(e))
                        },
                    }
                },
                Ok(created) => {
                    Ok(created)
                },
            }
        },
        Err(e) => {
            Err(e)
        },
    }
}

pub fn tags(conn: &PgConnection) -> Result<Vec<Tag>> {
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

pub fn list(conn: &PgConnection, size: Option<usize>) -> Result<Vec<Scribble>> {
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

pub fn tags_of(conn: &PgConnection, scribble_id: i64) -> Result<Vec<Tag>> {
    use diesel::sql_types::BigInt;

    let result = diesel::sql_query("SELECT tags.* FROM tags, taggings WHERE taggings.scribble_id = $1 AND taggings.tag_id = tags.id;")
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
