use rhis::{conf, db};
use rusqlite::Connection;
use std::collections::HashMap;

const BATCH_SIZE: usize = 200;

#[tokio::main]
async fn main() {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| shellexpand::tilde("~/.local/share/rhis/config.toml").into_owned());
    conf::conf_init(&config_path);
    db::warmup();

    let sqlite_path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "history.db".into());

    eprintln!("reading from: {sqlite_path}");
    let conn = Connection::open(&sqlite_path).expect("failed to open sqlite db");

    let mut stmt = conn
        .prepare("SELECT cmd, cnt, when_run FROM commands")
        .expect("failed to prepare query");

    let rows = stmt
        .query_map([], |row| {
            let cmd: String = row.get(0)?;
            let cnt: i32 = row.get(1)?;
            let when_run: i64 = row.get(2)?;
            Ok((cmd, cnt, when_run))
        })
        .expect("failed to query");

    let mut merged: HashMap<String, (String, i32, i64)> = HashMap::new();
    let mut total = 0;
    let mut skipped = 0;

    for row in rows {
        let (cmd, cnt, when_run) = row.expect("failed to read row");
        total += 1;

        let sanitized = db::sanitize(&cmd);
        if sanitized.is_empty() || sanitized.starts_with(' ') {
            skipped += 1;
            continue;
        }

        let key = rhis::normalize::normalize(&sanitized);
        if key.is_empty() {
            skipped += 1;
            continue;
        }

        merged
            .entry(key)
            .and_modify(|e| {
                e.1 += cnt;
                if when_run > e.2 {
                    e.0 = sanitized.clone();
                    e.2 = when_run;
                }
            })
            .or_insert_with(|| (sanitized, cnt, when_run));
    }

    let count = merged.len();
    eprintln!("total: {total}, skipped: {skipped}, deduped: {count}");

    let entries: Vec<(String, i32, i64)> = merged.into_values().collect();
    let batches = entries.len().div_ceil(BATCH_SIZE);

    for (bi, chunk) in entries.chunks(BATCH_SIZE).enumerate() {
        let batch: Vec<(String, i32, i64)> = chunk.to_vec();
        db::migrate_insert_batch(&batch).await;
        eprintln!("  batch {}/{batches} done ({} rows)", bi + 1, chunk.len());
    }

    eprintln!("migration complete: {count} commands inserted");
}
