use anyhow::Result;
use rusqlite::Connection;

static SCHEMA: &str = include_str!("../../migrations/001_initial.sql");

pub fn apply(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    Ok(())
}
