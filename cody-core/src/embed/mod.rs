pub mod client;

use anyhow::Result;
use rusqlite::Connection;
use crate::db::store::load_all_traces;
use client::EmbeddingClient;

const BATCH_SIZE: usize = 100;

pub async fn embed_traces(
    conn: &Connection,
    api_key: &str,
    model: &str,
) -> Result<()> {
    let traces = load_all_traces(conn)?;
    let unembed: Vec<_> = traces.iter()
        .filter(|t| {
            // Check if embedding already stored
            conn.query_row(
                "SELECT 1 FROM pipeline_checkpoints WHERE step='embed' AND key=?1 AND status='done'",
                [&t.trace_id],
                |_| Ok(true),
            ).unwrap_or(false) == false
        })
        .collect();

    if unembed.is_empty() {
        tracing::info!("All traces already embedded.");
        return Ok(());
    }

    let client = EmbeddingClient::new(api_key.to_string(), model.to_string());
    let mut total = 0usize;

    for chunk in unembed.chunks(BATCH_SIZE) {
        let texts: Vec<&str> = chunk.iter().map(|t| t.compact.as_str()).collect();
        let vectors = client.embed_batch(&texts).await?;

        for (trace, vec) in chunk.iter().zip(vectors.iter()) {
            let vec_json = serde_json::to_string(vec)?;
            conn.execute(
                "UPDATE traces SET otlp = ?1 WHERE trace_id = ?2",
                rusqlite::params![vec_json, trace.trace_id],
            )?;
            crate::db::store::checkpoint(conn, "embed", &trace.trace_id, "done", None)?;
            total += 1;
        }
        tracing::info!("Embedded {total}/{} traces", unembed.len());
    }

    tracing::info!("Done embedding {} traces", total);
    Ok(())
}
