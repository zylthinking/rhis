use crate::conf;
use crate::normalize;
use sqlx::{
    PgPool, Row,
    postgres::{PgConnectOptions, PgPoolOptions},
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
    command.is_empty() || command.starts_with(' ') || IGNORED.contains(&command) || command.starts_with("rhis")
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub async fn save_command(command: &str, session_id: &str, dir: &str, exit_code: i32) {
    if ignored(command) {
        return;
    }
    if exit_code != 0 && !crate::shell::execute_able(command, dir, exit_code) {
        return;
    }

    let normalized = normalize::normalize(command);
    let when = now_secs();
    let pool = pg_pool();
    let schema = &conf::conf_get().database.schema;

    let ebm = execute_by_me(&normalized, session_id, dir).await;

    let sql = format!(
        "INSERT INTO {schema}.commands (original, normalized, cnt, when_run, exit_code, selected, dir) \
         VALUES ($1, $2, 1, $3, $4, $5, $6) \
         ON CONFLICT (normalized, dir) DO UPDATE SET \
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
        .bind(dir)
        .execute(pool)
        .await;
}

pub async fn execute_by_me(cmd: &str, session_id: &str, dir: &str) -> bool {
    let pool = pg_pool();
    let schema = &conf::conf_get().database.schema;
    let sql = format!(
        "DELETE FROM {schema}.selected_commands \
         WHERE cmd = $1 AND session_id = $2 AND dir = $3"
    );
    let rows = sqlx::query(&sql)
        .bind(cmd)
        .bind(session_id)
        .bind(dir)
        .execute(pool)
        .await
        .map(|r| r.rows_affected())
        .unwrap_or(0);

    let cleanup = format!("DELETE FROM {schema}.selected_commands WHERE session_id = $1");
    _ = sqlx::query(&cleanup).bind(session_id).execute(pool).await;

    rows > 0
}

pub async fn record_selected(cmd: &str, session_id: &str, dir: &str) {
    let pool = pg_pool();
    let schema = &conf::conf_get().database.schema;
    let sql = format!(
        "INSERT INTO {schema}.selected_commands (cmd, session_id, dir) VALUES ($1, $2, $3)"
    );
    _ = sqlx::query(&sql)
        .bind(cmd)
        .bind(session_id)
        .bind(dir)
        .execute(pool)
        .await;
}

pub async fn find_matches(
    pattern: &str,
    dir: &str,
    anywhere: bool,
    limit: i64,
    offset: i64,
) -> (Vec<Match>, i64) {
    let pool = pg_pool();
    let schema = &conf::conf_get().database.schema;
    let normalized_pattern = normalize::normalize(pattern);
    let like = format!("%{}%", &normalized_pattern);

    let sql = format!(
        "SELECT original, when_run FROM {schema}.commands \
         WHERE normalized LIKE $1 AND ($2 OR dir = $3) \
         ORDER BY when_run DESC LIMIT $4 OFFSET $5"
    );
    let rows = match sqlx::query(&sql)
        .bind(&like)
        .bind(anywhere)
        .bind(dir)
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

    let mut commands: Vec<Match> = rows
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
         WHERE normalized LIKE $1 AND ($2 OR dir = $3)"
    );
    let total: i64 = sqlx::query_scalar(&count_sql)
        .bind(&like)
        .bind(anywhere)
        .bind(dir)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    commands.sort_by(|a, b| b.last_run.cmp(&a.last_run));
    (commands, total)
}

pub async fn delete_command(original: &str, dir: &str) {
    let normalized = normalize::normalize(original);
    let pool = pg_pool();
    let schema = &conf::conf_get().database.schema;
    let sql = format!("DELETE FROM {schema}.commands WHERE normalized = $1 AND dir = $2");
    _ = sqlx::query(&sql)
        .bind(&normalized)
        .bind(dir)
        .execute(pool)
        .await;
}

#[derive(Debug, Clone)]
pub struct Match {
    pub cmd: String,
    pub last_run: i64,
    pub match_bounds: Vec<(usize, usize)>,
}
