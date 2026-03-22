use anyhow::Result;
use rusqlite::{Connection, params};
use crate::db::models::*;

// ── file_meta ──────────────────────────────────────────────────────────────

pub fn upsert_file_meta(conn: &Connection, m: &FileMeta) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO file_meta (file, language, lines, exports, imports, hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![m.file, m.language, m.lines, m.exports, m.imports, m.hash],
    )?;
    Ok(())
}

pub fn get_file_hash(conn: &Connection, file: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT hash FROM file_meta WHERE file = ?1")?;
    let mut rows = stmt.query(params![file])?;
    if let Some(row) = rows.next()? {
        Ok(row.get(0)?)
    } else {
        Ok(None)
    }
}

// ── symbols ────────────────────────────────────────────────────────────────

pub fn insert_symbols(conn: &Connection, symbols: &[Symbol]) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT OR REPLACE INTO symbols
         (name, kind, file, line, signature, is_exported, prov_source, prov_confidence)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
    )?;
    for s in symbols {
        stmt.execute(params![
            s.name, s.kind, s.file, s.line, s.signature,
            s.is_exported as i64, s.prov_source, s.prov_confidence
        ])?;
    }
    Ok(())
}

pub fn lookup_symbol(conn: &Connection, name: &str) -> Result<Vec<Symbol>> {
    let row_mapper = |r: &rusqlite::Row| {
        Ok(Symbol {
            id:              Some(r.get(0)?),
            name:            r.get(1)?,
            kind:            r.get(2)?,
            file:            r.get(3)?,
            line:            r.get(4)?,
            signature:       r.get(5)?,
            is_exported:     r.get::<_, i64>(6)? != 0,
            prov_source:     r.get(7)?,
            prov_confidence: r.get(8)?,
        })
    };
    let cols = "id,name,kind,file,line,signature,is_exported,prov_source,prov_confidence";

    // 1. Exact match
    let exact: Vec<Symbol> = conn.prepare(
        &format!("SELECT {cols} FROM symbols WHERE name = ?1 ORDER BY file"),
    )?.query_map(params![name], row_mapper)?.filter_map(|r| r.ok()).collect();
    if !exact.is_empty() { return Ok(exact); }

    // 2. Fuzzy: case-insensitive substring, ranked prefix-first, capped at 30
    let like_pat = format!("%{}%", name.to_lowercase());
    let fuzzy: Vec<Symbol> = conn.prepare(&format!(
        "SELECT {cols} FROM symbols WHERE lower(name) LIKE ?1
         ORDER BY
           CASE WHEN lower(name) = lower(?2)        THEN 0
                WHEN lower(name) LIKE lower(?2)||'%' THEN 1
                ELSE 2 END,
           length(name), file
         LIMIT 30",
    ))?.query_map(params![like_pat, name], row_mapper)?.filter_map(|r| r.ok()).collect();
    Ok(fuzzy)
}

// ── edges ──────────────────────────────────────────────────────────────────

pub fn insert_edges(conn: &Connection, edges: &[Edge]) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO edges (src_file, src_symbol, rel, dst_file, dst_symbol, context, line)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
    )?;
    for e in edges {
        stmt.execute(params![
            e.src_file, e.src_symbol, e.rel,
            e.dst_file, e.dst_symbol, e.context, e.line
        ])?;
    }
    Ok(())
}

pub fn callers_of(conn: &Connection, symbol: &str) -> Result<Vec<Edge>> {
    let mut stmt = conn.prepare(
        "SELECT id,src_file,src_symbol,rel,dst_file,dst_symbol,context,line
         FROM edges WHERE rel='calls' AND dst_symbol=?1",
    )?;
    let rows = stmt.query_map(params![symbol], edge_from_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

pub fn callees_of(conn: &Connection, symbol: &str) -> Result<Vec<Edge>> {
    let mut stmt = conn.prepare(
        "SELECT id,src_file,src_symbol,rel,dst_file,dst_symbol,context,line
         FROM edges WHERE rel='calls' AND src_symbol=?1",
    )?;
    let rows = stmt.query_map(params![symbol], edge_from_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

pub fn deps_of_file(conn: &Connection, file: &str) -> Result<Vec<Edge>> {
    let mut stmt = conn.prepare(
        "SELECT id,src_file,src_symbol,rel,dst_file,dst_symbol,context,line
         FROM edges WHERE rel='imports' AND src_file=?1",
    )?;
    let rows = stmt.query_map(params![file], edge_from_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

pub fn load_call_adjacency(conn: &Connection) -> Result<Vec<(String, String, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(src_symbol,''), COALESCE(src_file,''),
                COALESCE(dst_symbol,''), COALESCE(dst_file,'')
         FROM edges WHERE rel IN ('calls','data_flow') AND src_symbol IS NOT NULL AND dst_symbol IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

fn edge_from_row(r: &rusqlite::Row) -> rusqlite::Result<Edge> {
    Ok(Edge {
        id:         Some(r.get(0)?),
        src_file:   r.get(1)?,
        src_symbol: r.get(2)?,
        rel:        r.get(3)?,
        dst_file:   r.get(4)?,
        dst_symbol: r.get(5)?,
        context:    r.get(6)?,
        line:       r.get(7)?,
    })
}

// ── boundary_events ────────────────────────────────────────────────────────

pub fn insert_boundary_events(conn: &Connection, events: &[BoundaryEvent]) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO boundary_events
         (fn_name,file,line,direction,medium,key_raw,key_norm,local_var,raw_context,
          prov_source,prov_confidence,prov_plugin,prov_note)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
    )?;
    for e in events {
        stmt.execute(params![
            e.fn_name, e.file, e.line, e.direction, e.medium,
            e.key_raw, e.key_norm, e.local_var, e.raw_context,
            e.prov_source, e.prov_confidence, e.prov_plugin, e.prov_note
        ])?;
    }
    Ok(())
}

pub fn boundaries_for_fn(conn: &Connection, fn_name: &str) -> Result<Vec<BoundaryEvent>> {
    let mut stmt = conn.prepare(
        "SELECT id,fn_name,file,line,direction,medium,key_raw,key_norm,local_var,
                raw_context,prov_source,prov_confidence,prov_plugin,prov_note
         FROM boundary_events WHERE fn_name=?1",
    )?;
    let rows = stmt.query_map(params![fn_name], be_from_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

pub fn boundaries_for_medium(conn: &Connection, medium: &str) -> Result<Vec<BoundaryEvent>> {
    let mut stmt = conn.prepare(
        "SELECT id,fn_name,file,line,direction,medium,key_raw,key_norm,local_var,
                raw_context,prov_source,prov_confidence,prov_plugin,prov_note
         FROM boundary_events WHERE medium=?1 ORDER BY file,line",
    )?;
    let rows = stmt.query_map(params![medium], be_from_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

pub fn load_all_boundary_events(conn: &Connection) -> Result<Vec<BoundaryEvent>> {
    let mut stmt = conn.prepare(
        "SELECT id,fn_name,file,line,direction,medium,key_raw,key_norm,local_var,
                raw_context,prov_source,prov_confidence,prov_plugin,prov_note
         FROM boundary_events",
    )?;
    let rows = stmt.query_map([], be_from_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

pub fn insert_boundary_flows(conn: &Connection, flows: &[BoundaryFlow]) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT OR REPLACE INTO boundary_flows
         (write_fn,write_file,read_fn,read_file,medium,key_norm,confidence)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
    )?;
    for f in flows {
        stmt.execute(params![
            f.write_fn, f.write_file, f.read_fn, f.read_file,
            f.medium, f.key_norm, f.confidence
        ])?;
    }
    Ok(())
}

fn be_from_row(r: &rusqlite::Row) -> rusqlite::Result<BoundaryEvent> {
    Ok(BoundaryEvent {
        id:              Some(r.get(0)?),
        fn_name:         r.get(1)?,
        file:            r.get(2)?,
        line:            r.get(3)?,
        direction:       r.get(4)?,
        medium:          r.get(5)?,
        key_raw:         r.get(6)?,
        key_norm:        r.get(7)?,
        local_var:       r.get(8)?,
        raw_context:     r.get(9)?,
        prov_source:     r.get(10)?,
        prov_confidence: r.get(11)?,
        prov_plugin:     r.get(12)?,
        prov_note:       r.get(13)?,
    })
}

// ── entry_points ───────────────────────────────────────────────────────────

pub fn insert_entry_points(conn: &Connection, eps: &[EntryPoint]) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT OR REPLACE INTO entry_points
         (fn_name,file,line,kind,framework,path,method,confidence,heuristics,middleware)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
    )?;
    for e in eps {
        stmt.execute(params![
            e.fn_name, e.file, e.line, e.kind,
            e.framework, e.path, e.method, e.confidence,
            serde_json::to_string(&e.heuristics).unwrap_or_default(),
            e.middleware.as_deref()
        ])?;
    }
    Ok(())
}

pub fn load_entry_points(conn: &Connection, min_confidence: f64) -> Result<Vec<EntryPoint>> {
    let mut stmt = conn.prepare(
        "SELECT id,fn_name,file,line,kind,framework,path,method,confidence,heuristics,middleware
         FROM entry_points WHERE confidence >= ?1 ORDER BY confidence DESC",
    )?;
    let rows = stmt.query_map(params![min_confidence], |r| {
        let heuristics_json: String = r.get(9)?;
        let heuristics = serde_json::from_str(&heuristics_json).unwrap_or_default();
        Ok(EntryPoint {
            id:         Some(r.get(0)?),
            fn_name:    r.get(1)?,
            file:       r.get(2)?,
            line:       r.get(3)?,
            kind:       r.get(4)?,
            framework:  r.get(5)?,
            path:       r.get(6)?,
            method:     r.get(7)?,
            confidence: r.get(8)?,
            heuristics,
            middleware: r.get(10)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

// ── traces ─────────────────────────────────────────────────────────────────

pub fn insert_trace(conn: &Connection, t: &Trace) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO traces
         (trace_id,root_fn,root_file,service,text,compact,otlp,span_count,
          fn_names,media,value_names,min_confidence,created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![
            t.trace_id, t.root_fn, t.root_file, t.service,
            t.text, t.compact, t.otlp, t.span_count,
            serde_json::to_string(&t.fn_names).unwrap_or_default(),
            serde_json::to_string(&t.media).unwrap_or_default(),
            serde_json::to_string(&t.value_names).unwrap_or_default(),
            t.min_confidence, t.created_at
        ],
    )?;
    Ok(())
}

pub fn traces_for_fn(conn: &Connection, fn_name: &str) -> Result<Vec<Trace>> {
    let mut stmt = conn.prepare(
        "SELECT id,trace_id,root_fn,root_file,service,text,compact,otlp,span_count,
                fn_names,media,value_names,min_confidence,created_at
         FROM traces WHERE root_fn=?1 OR fn_names LIKE ?2",
    )?;
    let pattern = format!("%\"{}\"%" , fn_name);
    let rows = stmt.query_map(params![fn_name, pattern], trace_from_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

pub fn load_all_traces(conn: &Connection) -> Result<Vec<Trace>> {
    let mut stmt = conn.prepare(
        "SELECT id,trace_id,root_fn,root_file,service,text,compact,otlp,span_count,
                fn_names,media,value_names,min_confidence,created_at FROM traces",
    )?;
    let rows = stmt.query_map([], trace_from_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

fn trace_from_row(r: &rusqlite::Row) -> rusqlite::Result<Trace> {
    Ok(Trace {
        id:             Some(r.get(0)?),
        trace_id:       r.get(1)?,
        root_fn:        r.get(2)?,
        root_file:      r.get(3)?,
        service:        r.get(4)?,
        text:           r.get(5)?,
        compact:        r.get(6)?,
        otlp:           r.get(7)?,
        span_count:     r.get(8)?,
        fn_names:       serde_json::from_str(&r.get::<_, String>(9)?).unwrap_or_default(),
        media:          serde_json::from_str(&r.get::<_, String>(10)?).unwrap_or_default(),
        value_names:    serde_json::from_str(&r.get::<_, String>(11)?).unwrap_or_default(),
        min_confidence: r.get(12)?,
        created_at:     r.get(13)?,
    })
}

// ── checkpoints ────────────────────────────────────────────────────────────

pub fn checkpoint(conn: &Connection, step: &str, key: &str, status: &str, detail: Option<&str>) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO pipeline_checkpoints (step,key,status,detail,updated_at)
         VALUES (?1,?2,?3,?4,datetime('now'))",
        params![step, key, status, detail],
    )?;
    Ok(())
}

// ── stats ──────────────────────────────────────────────────────────────────

pub fn stats(conn: &Connection) -> Result<serde_json::Value> {
    let sym_count: i64 = conn.query_row("SELECT COUNT(*) FROM symbols", [], |r| r.get(0))?;
    let edge_count: i64 = conn.query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0))?;
    let file_count: i64 = conn.query_row("SELECT COUNT(*) FROM file_meta", [], |r| r.get(0))?;
    let be_count: i64 = conn.query_row("SELECT COUNT(*) FROM boundary_events", [], |r| r.get(0))?;
    let ep_count: i64 = conn.query_row("SELECT COUNT(*) FROM entry_points", [], |r| r.get(0))?;
    let tr_count: i64 = conn.query_row("SELECT COUNT(*) FROM traces", [], |r| r.get(0))?;
    Ok(serde_json::json!({
        "files": file_count,
        "symbols": sym_count,
        "edges": edge_count,
        "boundary_events": be_count,
        "entry_points": ep_count,
        "traces": tr_count,
    }))
}
