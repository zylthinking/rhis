use crate::{history::db_extensions, network::Network, path_update_helpers, settings::Settings, shell, shell_history};
use rusqlite::{named_params, Connection, Transaction};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub struct Features {
    pub age_factor: f64,
    pub length_factor: f64,
    pub exit_factor: f64,
    pub recent_failure_factor: f64,
    pub selected_dir_factor: f64,
    pub dir_factor: f64,
    pub overlap_factor: f64,
    pub immediate_overlap_factor: f64,
    pub selected_occurrences_factor: f64,
    pub occurrences_factor: f64,
}

pub struct Command {
    pub cmd: String,
    pub last_run: i64,
    pub match_bounds: Vec<(usize, usize)>,
}

pub struct History {
    pub conn: Connection,
    pub network: Network,
}

impl History {
    pub fn load<const B: bool>() -> History {
        let path = Settings::db_path();

        let conn = if path.exists() {
            Connection::open(&path).unwrap_or_else(|_| panic!("Unable to open {}", path.as_path().display()))
        } else {
            History::from_shell_history()
        };

        if B {
            db_extensions::add_db_functions(&conn);
        }

        History {
            conn,
            network: Network::default(),
        }
    }

    fn ignored(command: &str) -> bool {
        const IGNORED_COMMANDS: [&str; 6] = ["pwd", "ls", "cd", "cd ..", "clear", "history"];
        command.is_empty()
            || command.starts_with(' ')
            || IGNORED_COMMANDS.contains(&command)
            || command.starts_with("rhis")
    }

    fn add_new_cmd(trans: &Transaction, dir: Option<&str>, command: &String, exit_code: i32, selected: i32) {
        let when = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        let (failed, exit_code) = if exit_code == 0 { (0, 1) } else { (when, 0) };

        trans
            .execute(
                "INSERT INTO commands(cmd, cnt, when_run, when_failed, exit_code, selected, dir) \
                 VALUES (:cmd, 1, :when_run, :when_failed, :exit_code, :selected, :dir) \
                 ON CONFLICT(cmd, dir) DO UPDATE SET \
                      id = excluded.id, cnt = cnt + 1, when_run = excluded.when_run, \
                      when_failed = excluded.when_failed, \
                      exit_code = excluded.exit_code, selected = selected + excluded.selected",
                named_params! {
                    ":cmd": &command,
                    ":when_run": &when,
                    ":when_failed": &failed,
                    ":exit_code": &exit_code,
                    ":selected": &selected,
                    ":dir": &dir
                },
            )
            .unwrap();
    }

    fn add_commands(
        connection: &mut Connection, dir: Option<&str>, exit_code: i32, selected: i32, commands: &Vec<String>,
    ) {
        let transaction = connection.transaction().unwrap();
        for command in commands {
            if Self::ignored(&command.as_str()) {
                continue;
            }
            Self::add_new_cmd(&transaction, dir, command, exit_code, selected);
        }
        transaction.commit().unwrap();
    }

    pub fn add(&mut self, command: &str, session_id: &str, dir: &str, exit_code: i32) {
        if exit_code == 0 {
            self.possibly_update_paths(command, dir);
        } else if !shell::execute_able(command, dir, exit_code) {
            return;
        }
        let ebm = self.execute_by_me(command, session_id, dir);
        let v = vec![command.to_owned()];
        Self::add_commands(&mut self.conn, Some(dir), exit_code, ebm as i32, &v);
    }

    fn execute_by_me(&self, command: &str, session_id: &str, dir: &str) -> bool {
        let rows_affected = self
            .conn
            .execute(
                "DELETE FROM selected_commands \
                 WHERE cmd = :cmd AND session_id = :session_id AND dir = :dir",
                &[(":cmd", command), (":session_id", session_id), (":dir", dir)],
            )
            .unwrap_or(0);

        self.conn
            .execute("DELETE FROM selected_commands WHERE session_id = :session_id", &[(":session_id", session_id)])
            .unwrap_or(0);

        rows_affected > 0
    }

    pub fn record_selected_from_ui(&self, command: &str, session_id: &str, dir: &str) {
        self.conn
            .execute(
                "INSERT INTO selected_commands (cmd, session_id, dir) VALUES (:cmd, :session_id, :dir)",
                &[(":cmd", command), (":session_id", session_id), (":dir", dir)],
            )
            .unwrap_or(0);
    }

    pub fn possibly_update_paths(&self, command: &str, dir: &str) {
        let is_move = |c: &str| c.to_lowercase().starts_with("mv ") && !c.contains('*') && !c.contains('?');
        if !is_move(command) {
            return;
        }

        let parts = path_update_helpers::parse_mv_command(command);
        if parts.len() == 2 {
            let normalized_from = shell::normalize_path(&parts[0], dir);
            let normalized_to = shell::normalize_path(&parts[1], dir);
            if normalized_from.is_none() || normalized_to.is_none() {
                return;
            }

            let normalized_from = normalized_from.unwrap();
            let path_from = PathBuf::from(&normalized_from);
            if !path_from.is_dir() {
                return;
            }
            let normalized_to = normalized_to.unwrap();
            let path_to = PathBuf::from(&normalized_to);

            if let Some(basename) = path_from.file_name() {
                if let Some(utf8_basename) = basename.to_str() {
                    let maybe_moved_directory = path_to.join(utf8_basename);
                    if maybe_moved_directory.exists() && maybe_moved_directory.is_dir() {
                        self.update_paths(&normalized_from, maybe_moved_directory.to_str().unwrap());
                        return;
                    }
                } else {
                    return;
                }
            }

            if path_to.exists() && path_to.is_dir() {
                self.update_paths(&normalized_from, &normalized_to);
            }
        }
    }

    pub fn find_matches(&self, cmd: &str, num: i16, rank: bool) -> Vec<Command> {
        let mut like_query = "%".to_string();
        like_query.push_str(cmd);
        like_query.push('%');

        let order_by_column: &str = if rank { "rank" } else { "last_run" };
        let query: &str = &format!(
            "SELECT cmd, last_run FROM contextual_commands \
             WHERE cmd LIKE (:like) \
             ORDER BY {order_by_column} DESC LIMIT :limit"
        )[..];

        let mut statement = self.conn.prepare(query).unwrap();
        let command_iter = statement
            .query_map(named_params! { ":like": &like_query, ":limit": &num }, |row| {
                let text: String = row.get(0).unwrap();
                let bounds = text.match_indices(cmd).map(|(index, _)| (index, index + cmd.len())).collect::<Vec<_>>();
                Ok(Command {
                    cmd: text,
                    last_run: row.get(1).unwrap(),
                    match_bounds: bounds,
                })
            })
            .unwrap();

        let mut names = Vec::new();
        for result in command_iter {
            let cmd = result.unwrap();
            if cmd.match_bounds.len() > 0 {
                names.push(cmd);
            }
        }
        names
    }

    pub fn build_cache_table(&self, dir: &str, anywhere: bool) {
        self.conn.execute("PRAGMA temp_store = MEMORY;", []).unwrap();
        self.conn.execute("DROP TABLE IF EXISTS temp.contextual_commands;", []).unwrap();

        let (mut when_run_min, when_run_max): (i64, i64) = self
            .conn
            .query_row("SELECT IFNULL(MIN(when_run), 0), IFNULL(MAX(when_run), 0) FROM commands", [], |row| {
                Ok((row.get_unwrap(0), row.get_unwrap(1)))
            })
            .unwrap();

        if when_run_max == when_run_min {
            when_run_min -= 1;
        }

        let max_occurrences: f64 = self
            .conn
            .query_row("SELECT SUM(cnt) AS c FROM commands GROUP BY cmd ORDER BY c DESC LIMIT 1", [], |row| row.get(0))
            .unwrap_or(1.0);

        let max_selected_occurrences: f64 = self
            .conn
            .query_row(
                "SELECT sum(selected) AS c FROM commands WHERE selected != 0 GROUP BY cmd ORDER BY c DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(1.0); // FIXME: 1.0 seems wrong.

        let mut max_length = self
            .conn
            .query_row("SELECT IFNULL(MAX(LENGTH(cmd)), 0) FROM commands", [], |row| row.get(0))
            .unwrap_or(100);

        if max_length == 0 {
            max_length = 1;
        }

        self.conn.execute(
            "CREATE TEMP TABLE contextual_commands AS SELECT
                  cmd, MAX(when_run) AS last_run, 0.0 AS rank,

                  (LENGTH(c.cmd) * 1.0) / :max_length AS length_factor,
                  (MIN((:when_run_max - when_run) * 1.0) / :history_duration) AS age_factor,
                  SUM(exit_code * 1.0) / SUM(cnt) as exit_factor,
                  MAX(CASE WHEN exit_code == 0 AND :now - when_failed < 120 THEN 1.0 ELSE 0.0 END) AS recent_failure_factor,
                  SUM(CASE WHEN dir = :directory THEN cnt * 1.0 ELSE 0.0 END) / SUM(cnt) as dir_factor,
                  SUM(CASE WHEN dir = :directory THEN selected * 1.0 ELSE 0.0 END) / (SUM(selected * 1.0) + 1.0) as selected_dir_factor,

                  SUM(selected * 1.0) / :max_selected_occurrences AS selected_occurrences_factor,
                  SUM(cnt) / :max_occurrences AS occurrences_factor

                  FROM commands c
                  WHERE (:anywhere OR dir LIKE :directory)
                  GROUP BY cmd
                  ORDER BY id DESC;",
            named_params! {
                ":when_run_max": &when_run_max,
                ":history_duration": &(when_run_max - when_run_min),
                ":directory": &dir.to_owned(),
                ":anywhere": &anywhere,
                ":max_occurrences": &max_occurrences,
                ":max_length": &max_length,
                ":max_selected_occurrences": &max_selected_occurrences,
                ":now": &(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64),
            }).unwrap();

        self.conn
            .execute(
                "UPDATE contextual_commands
                 SET rank = nn_rank(age_factor, length_factor, exit_factor,
                                    recent_failure_factor, selected_dir_factor, dir_factor,
                                    0.0, 0.0,
                                    selected_occurrences_factor, occurrences_factor);",
                [],
            )
            .unwrap();
    }

    pub fn delete_command(&mut self, command: &str) {
        let transaction = self.conn.transaction().unwrap();
        transaction.execute("DELETE FROM commands WHERE cmd = :command", &[(":command", &command)]).unwrap();
        transaction.execute("DELETE FROM contextual_commands WHERE cmd = :command", &[(":command", &command)]).unwrap();
        transaction.commit().unwrap();
    }

    fn update_paths(&self, old_path: &str, new_path: &str) {
        let like_query = old_path.to_string() + "/%";
        let _ = self.conn.execute(
            "UPDATE commands SET dir = :new_dir || SUBSTR(dir, :length) WHERE dir = :exact OR dir LIKE (:like)",
            named_params! {
                   ":like": &like_query,
                   ":exact": &old_path,
                   ":new_dir": &new_path,
                   ":length": &(old_path.chars().count() as u32 + 1),
            },
        );
    }

    fn from_shell_history() -> Connection {
        let commands = shell_history::full_history();
        let db_path = Settings::db_path();
        let db_dir = db_path.parent().unwrap();
        fs::create_dir_all(db_dir).unwrap_or_else(|_| panic!("Unable to create {:?}", db_dir));

        let mut connection =
            Connection::open(&db_path).unwrap_or_else(|_| panic!("Unable to create history DB at {:?}", &db_path));
        db_extensions::add_db_functions(&connection);

        connection
            .execute_batch(
                "BEGIN; \
                   CREATE TABLE commands( \
                      id INTEGER PRIMARY KEY AUTOINCREMENT, \
                      cmd TEXT NOT NULL, \
                      cnt INTEGER NOT NULL,
                      when_run INTEGER NOT NULL, \
                      when_failed INTEGER NOT NULL, \
                      exit_code INTEGER NOT NULL, \
                      selected INTEGER NOT NULL, \
                      dir TEXT\
                  ); \
                  CREATE UNIQUE INDEX command_cmds ON commands (cmd, dir);\
                  CREATE INDEX command_dirs ON commands (dir);\
                  \
                  CREATE TABLE selected_commands( \
                      id INTEGER PRIMARY KEY AUTOINCREMENT, \
                      cmd TEXT NOT NULL, \
                      session_id TEXT NOT NULL, \
                      dir TEXT NOT NULL \
                  ); \
                  CREATE INDEX selected_command_session_cmds ON selected_commands (session_id, cmd); \
                  COMMIT;",
            )
            .unwrap_or_else(|err| panic!("Unable to initialize history db ({})", err));

        History::add_commands(&mut connection, None, 0, 0, &commands);
        connection
    }
}
