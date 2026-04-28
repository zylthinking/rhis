use crate::conf;
use crate::normalize;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool, Row,
};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::runtime::Handle;
use tokio::task;

pub fn warmup() {
    pg_pool();
}

fn pg_pool() -> &'static PgPool {
    static POOL: OnceLock<Option<PgPool>> = OnceLock::new();
    let Some(pool) = POOL.get_or_init(|| {
        task::block_in_place(move || {
            Handle::current().block_on(async {
                let c = &conf::conf_get().database;
                let opt = PgConnectOptions::new()
                    .host(&c.host)
                    .port(c.port)
                    .username(&c.username)
                    .password(&c.password)
                    .database(&c.database);
                let pool = match PgPoolOptions::new()
                    .max_connections(5)
                    .connect_with(opt)
                    .await
                {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("failed to connect pg ({}:{}): {e}", c.host, c.port);
                        return None;
                    }
                };
                let sql = include_str!("init.sql").replace("{schema}", &c.schema);
                if let Err(e) = sqlx::raw_sql(&sql).execute(&pool).await {
                    eprintln!("    schema init failed: {e}");
                    return None;
                }
                Some(pool)
            })
        })
    }) else {
        panic!("pg connection failed");
    };
    pool
}

fn ignored(command: &str) -> bool {
    const IGNORED: [&str; 6] = ["pwd", "ls", "cd", "cd ..", "clear", "history"];
    command.is_empty()
        || command.starts_with(' ')
        || IGNORED.contains(&command)
        || command.to_lowercase().starts_with("rhis")
}

pub fn sanitize(raw: &str) -> String {
    raw.trim_end_matches(['\n', '\r', '\t', ' '])
        .chars()
        .filter(|&c| c >= ' ' || c == '\t')
        .collect()
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub async fn save_command(command: &str, session_id: &str, exit_code: i32) {
    let command = sanitize(command);
    if ignored(&command) {
        return;
    }
    if exit_code != 0 && !crate::shell::execute_able(&command, exit_code) {
        return;
    }

    let normalized = normalize::normalize(&command);
    let when = now_secs();
    let pool = pg_pool();
    let schema = &conf::conf_get().database.schema;

    let ebm = execute_by_me(&normalized, session_id).await;

    let sql = format!(
        "INSERT INTO {schema}.commands (original, normalized, cnt, when_run, exit_code, selected) \
         VALUES ($1, $2, 1, $3, $4, $5) \
         ON CONFLICT (normalized) DO UPDATE SET \
             original = EXCLUDED.original, \
             cnt = {schema}.commands.cnt + 1, \
             when_run = EXCLUDED.when_run, \
             exit_code = EXCLUDED.exit_code, \
             selected = {schema}.commands.selected + EXCLUDED.selected"
    );
    _ = sqlx::query(&sql)
        .bind(command)
        .bind(&normalized)
        .bind(when)
        .bind(exit_code)
        .bind(ebm as i32)
        .execute(pool)
        .await;
}

pub async fn execute_by_me(cmd: &str, session_id: &str) -> bool {
    let pool = pg_pool();
    let schema = &conf::conf_get().database.schema;
    let sql = format!(
        "DELETE FROM {schema}.selected_commands \
         WHERE cmd = $1 AND session_id = $2"
    );
    let rows = sqlx::query(&sql)
        .bind(cmd)
        .bind(session_id)
        .execute(pool)
        .await
        .map(|r| r.rows_affected())
        .unwrap_or(0);

    let cleanup = format!("DELETE FROM {schema}.selected_commands WHERE session_id = $1");
    _ = sqlx::query(&cleanup).bind(session_id).execute(pool).await;

    rows > 0
}

pub async fn record_selected(cmd: &str, session_id: &str) {
    let pool = pg_pool();
    let schema = &conf::conf_get().database.schema;
    let sql = format!(
        "INSERT INTO {schema}.selected_commands (cmd, session_id) VALUES ($1, $2)"
    );
    _ = sqlx::query(&sql)
        .bind(cmd)
        .bind(session_id)
        .execute(pool)
        .await;
}

pub async fn find_matches(
    pattern: &str,
    limit: i64,
    offset: i64,
) -> (Vec<Match>, i64) {
    let pool = pg_pool();
    let schema = &conf::conf_get().database.schema;
    let normalized_pattern = normalize::normalize(pattern);
    let like = format!("%{}%", &normalized_pattern);

    let sql = format!(
        "SELECT original, when_run FROM {schema}.commands \
         WHERE normalized LIKE $1 \
         ORDER BY when_run DESC LIMIT $2 OFFSET $3"
    );
    let rows = match sqlx::query(&sql)
        .bind(&like)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("find_matches: {e}");
            return (vec![], 0);
        }
    };

    let commands: Vec<Match> = rows
        .iter()
        .map(|row| {
            let original: String = row.get(0);
            let when_run: i64 = row.get(1);
            let bounds = original
                .match_indices(pattern)
                .map(|(i, _)| (i, i + pattern.len()))
                .collect();
            Match {
                cmd: original,
                last_run: when_run,
                match_bounds: bounds,
            }
        })
        .collect();

    let count_sql = format!(
        "SELECT COUNT(*) FROM {schema}.commands \
         WHERE normalized LIKE $1"
    );
    let total: i64 = sqlx::query_scalar(&count_sql)
        .bind(&like)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    (commands, total)
}

pub async fn delete_command(original: &str) {
    let normalized = normalize::normalize(original);
    let pool = pg_pool();
    let schema = &conf::conf_get().database.schema;
    let sql = format!("DELETE FROM {schema}.commands WHERE normalized = $1");
    _ = sqlx::query(&sql)
        .bind(&normalized)
        .execute(pool)
        .await;
}

pub async fn migrate_insert_batch(entries: &[(String, i32, i64)]) {
    if entries.is_empty() {
        return;
    }
    let pool = pg_pool();
    let schema = &conf::conf_get().database.schema;

    let mut values = String::new();
    for (i, (orig, cnt, when)) in entries.iter().enumerate() {
        if i > 0 {
            values.push(',');
        }
        let norm = normalize::normalize(orig);
        let escaped_orig = escape_sql_string(orig);
        let escaped_norm = escape_sql_string(&norm);
        values.push_str(&format!(
            "(E'{escaped_orig}', E'{escaped_norm}', {cnt}, {when}, 0, 0)"
        ));
    }

    let sql = format!(
        "INSERT INTO {schema}.commands (original, normalized, cnt, when_run, exit_code, selected) \
         VALUES {values} \
         ON CONFLICT (normalized) DO UPDATE SET \
             original = EXCLUDED.original, \
             cnt = {schema}.commands.cnt + EXCLUDED.cnt, \
             when_run = GREATEST({schema}.commands.when_run, EXCLUDED.when_run)"
    );

    _ = sqlx::raw_sql(&sql).execute(pool).await;
}

fn escape_sql_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct Match {
    pub cmd: String,
    pub last_run: i64,
    pub match_bounds: Vec<(usize, usize)>,
}
