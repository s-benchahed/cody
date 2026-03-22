use anyhow::Result;
use rusqlite::Connection;
use crate::db::store::traces_for_fn;

pub fn cmd_traces(conn: &Connection, fn_name: &str) -> Result<()> {
    let traces = traces_for_fn(conn, fn_name)?;
    if traces.is_empty() {
        println!("No traces found for: {fn_name}");
        return Ok(());
    }
    for t in traces {
        println!("─── TRACE {} (spans={}, min_conf={:.2}) ───",
            t.trace_id, t.span_count, t.min_confidence);
        println!("{}", t.text);
    }
    Ok(())
}
