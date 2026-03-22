use anyhow::Result;
use rusqlite::Connection;

pub fn cmd_cross(conn: &Connection, svc_a: &str, svc_b: &str) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT write_fn, write_file, read_fn, read_file, medium, key_norm, confidence
         FROM boundary_flows
         WHERE (write_file LIKE ?1 OR read_file LIKE ?2)
         ORDER BY confidence DESC"
    )?;
    let pat_a = format!("%{}%", svc_a);
    let pat_b = format!("%{}%", svc_b);
    let rows = stmt.query_map([&pat_a, &pat_b], |r| {
        Ok((
            r.get::<_,String>(0)?, r.get::<_,String>(1)?,
            r.get::<_,String>(2)?, r.get::<_,String>(3)?,
            r.get::<_,String>(4)?, r.get::<_,String>(5)?,
            r.get::<_,f64>(6)?,
        ))
    })?;

    let mut found = false;
    for row in rows {
        let (wfn, wf, rfn, rf, medium, key, conf) = row?;
        if (wf.contains(svc_a) && rf.contains(svc_b))
            || (wf.contains(svc_b) && rf.contains(svc_a))
        {
            if !found {
                println!("Cross-service flows between '{svc_a}' and '{svc_b}':");
                found = true;
            }
            println!("  {medium}[\"{key}\"]  WRITE: {wfn} ({wf})  →  READ: {rfn} ({rf})  [conf={conf:.2}]");
        }
    }
    if !found {
        println!("No cross-service flows found between '{svc_a}' and '{svc_b}'");
    }
    Ok(())
}
