use anyhow::Result;
use rusqlite::Connection;
use std::collections::{HashMap, VecDeque};

pub fn cmd_path(conn: &Connection, from: &str, to: &str) -> Result<()> {
    // BFS over call edges
    let mut stmt = conn.prepare(
        "SELECT src_symbol, dst_symbol FROM edges WHERE rel='calls'
         AND src_symbol IS NOT NULL AND dst_symbol IS NOT NULL"
    )?;
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    let rows = stmt.query_map([], |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?)))?;
    for row in rows {
        let (src, dst) = row?;
        adj.entry(src).or_default().push(dst);
    }

    // BFS
    let mut queue: VecDeque<Vec<String>> = VecDeque::new();
    queue.push_back(vec![from.to_string()]);
    let mut visited = std::collections::HashSet::new();
    visited.insert(from.to_string());

    while let Some(path) = queue.pop_front() {
        let node = path.last().unwrap();
        if node == to {
            println!("Path: {}", path.join(" → "));
            return Ok(());
        }
        if let Some(nexts) = adj.get(node) {
            for next in nexts {
                if !visited.contains(next) {
                    visited.insert(next.clone());
                    let mut new_path = path.clone();
                    new_path.push(next.clone());
                    queue.push_back(new_path);
                }
            }
        }
    }
    println!("No path found from {from} to {to}");
    Ok(())
}
