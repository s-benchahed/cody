pub mod models;
pub mod schema;
pub mod store;

use anyhow::Result;
use rusqlite::Connection;

pub fn open(path: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    schema::apply(&conn)?;
    Ok(conn)
}
