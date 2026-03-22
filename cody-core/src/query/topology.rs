use anyhow::Result;
use rusqlite::Connection;
use std::collections::HashMap;
use crate::traces::span::service_from_path;

pub fn cmd_topology(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT write_file, read_file, medium, key_norm, confidence
         FROM boundary_flows ORDER BY confidence DESC"
    )?;

    // (write_svc, read_svc) -> Vec<(medium, key_norm, conf)>
    let mut edges: HashMap<(String, String), Vec<(String, String, f64)>> = HashMap::new();

    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?,
            r.get::<_,String>(2)?, r.get::<_,String>(3)?, r.get::<_,f64>(4)?))
    })?;

    for row in rows {
        let (wf, rf, medium, key, conf) = row?;
        let ws = service_from_path(&wf);
        let rs = service_from_path(&rf);
        if ws == rs { continue; } // skip self-loops
        edges.entry((ws, rs)).or_default().push((medium, key, conf));
    }

    if edges.is_empty() {
        println!("No cross-service boundary flows found.");
        return Ok(());
    }

    println!("SERVICE TOPOLOGY\n");
    let mut sorted: Vec<_> = edges.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    for ((wsvc, rsvc), flows) in sorted {
        // Group by medium
        let mut by_medium: HashMap<String, (usize, f64)> = HashMap::new();
        for (medium, _, conf) in &flows {
            let e = by_medium.entry(medium.clone()).or_insert((0, 0.0));
            e.0 += 1;
            e.1 = e.1.max(*conf);
        }
        let medium_summary: Vec<String> = {
            let mut v: Vec<_> = by_medium.into_iter().collect();
            v.sort_by(|a, b| a.0.cmp(&b.0));
            v.iter().map(|(m, (n, c))| format!("{m}\u{d7}{n}(conf={c:.2})")).collect()
        };
        println!("  {wsvc}  \u{2500}\u{2500}\u{25ba}  {rsvc}   [{}]", medium_summary.join("  "));
        // Show top 3 keys
        let mut shown = 0;
        for (medium, key, conf) in &flows {
            if shown >= 3 { break; }
            println!("    {medium}[\"{key}\"]  conf={conf:.2}");
            shown += 1;
        }
    }
    Ok(())
}
