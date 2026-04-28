CREATE SCHEMA IF NOT EXISTS {schema};

CREATE TABLE IF NOT EXISTS {schema}.commands (
    id SERIAL PRIMARY KEY,
    original TEXT NOT NULL,
    normalized TEXT NOT NULL,
    cnt INTEGER NOT NULL DEFAULT 1,
    when_run BIGINT NOT NULL,
    exit_code INTEGER NOT NULL DEFAULT 0,
    selected INTEGER NOT NULL DEFAULT 0,
    dir TEXT NOT NULL DEFAULT ''
);
CREATE UNIQUE INDEX IF NOT EXISTS command_norm ON {schema}.commands (normalized, dir);
CREATE INDEX IF NOT EXISTS command_dirs ON {schema}.commands (dir);
CREATE INDEX IF NOT EXISTS command_when ON {schema}.commands (when_run DESC);

CREATE TABLE IF NOT EXISTS {schema}.selected_commands (
    id SERIAL PRIMARY KEY,
    cmd TEXT NOT NULL,
    session_id TEXT NOT NULL,
    dir TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS selected_cmd_sessions ON {schema}.selected_commands (session_id, cmd);
