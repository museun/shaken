use crate::prelude::*;
use chrono::prelude::*;
use log::*;
use rusqlite::{types::ToSql, Connection, NO_PARAMS};

use crate::module::CommandMap;

#[derive(Default, Debug)]
pub struct UserCommand {
    pub command: String,
    pub body: String,
    pub description: String,
    pub creator: i64,
    pub created_at: i64,
    pub uses: i64,
    pub disabled: bool,
}

pub const NAME: &str = "Builtin";

submit! {
    template::Response("builtin_reserved_name", "\"${command}\" is a reserved name");
    template::Response("builtin_already_exists", "\"${command}\" already exists as a command");
    template::Response("builtin_added_command", "added \"${command}\" as a command");
    template::Response("builtin_add_failed", "couldn't add \"${command}\" as a command");
    template::Response("builtin_edited", "edited \"${command}\"");
    template::Response("builtin_edited_failed", "couldn't edit \"${command}\"");
    template::Response("builtin_invalid_command", "\"${command}\" isn't a command");
    template::Response("builtin_command_description", "${command} -- ${description}");
    template::Response("builtin_command_created_at", "created by ${user}. used ${uses} times");
    template::Response("builtin_command_deleted", "if that was a command, its no longer one");
    template::Response("builtin_github_repo", "https://github.com/museun/shaken (${rev} on '${branch}' branch)");
    template::Response("builtin_stream_offline", "the stream doesn't seem to be live");
    template::Response("builtin_viewers", "viewers: ${viewers}");
    template::Response("builtin_uptime", "uptime: ${uptime}");
}

pub struct Builtin {
    twitch: TwitchClient,
    channel: String,
    map: CommandMap<Builtin>,
}

impl Module for Builtin {
    fn command(&mut self, req: &Request) -> Option<Response> {
        let map = self.map.clone();
        match map.dispatch(self, req) {
            Some(resp) => Some(resp),
            None => self.try_user_command(req),
        }
    }

    fn event(&mut self, msg: &irc::Message) -> Option<Response> {
        match msg.command() {
            "001" => join(&format!("#{}", self.channel)),
            "PING" => raw!("PONG :{}", msg.expect_data()),
            _ => None,
        }
    }
}

impl Builtin {
    pub fn create() -> Result<Self, ModuleError> {
        Self::ensure_table(&database::get_connection());

        for cmd in Self::fetch_command_names() {
            if !Self::is_available(&cmd) {
                Self::disable_bad_command(&cmd);
                warn!("command is already reserved: {}", cmd);
            }
        }

        Ok(Self {
            twitch: TwitchClient::new(&Config::expect_env("SHAKEN_TWITCH_CLIENT_ID")),
            map: CommandMap::create(
                NAME,
                &[
                    ("!version", Builtin::version_command),
                    ("!viewers", Builtin::viewers_command),
                    ("!uptime", Builtin::uptime_command),
                    ("!add", Builtin::add_command),
                    ("!edit", Builtin::edit_command),
                    ("!info", Builtin::info_command),
                    ("!remove", Builtin::remove_command),
                    ("!help", Builtin::help_command),
                ],
            )?,
            channel: Config::load().twitch.channel.to_string(),
        })
    }

    fn ensure_table(conn: &Connection) {
        conn.execute(
            r#"CREATE TABLE IF NOT EXISTS UserCommands(
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                command         TEXT NOT NULL,
                body            TEXT NOT NULL,
                description     TEXT NOT NULL,
                creator         INTEGER NOT NULL,
                created_at      INTEGER NOT NULL,
                uses            INTEGER NOT NULL,
                disabled        INTEGER,
                UNIQUE(command)
            )"#,
            NO_PARAMS,
        )
        .expect("create UserCommands table");
    }

    fn try_user_command(&self, req: &Request) -> Option<Response> {
        struct Command {
            body: String,
            disabled: bool,
        }

        let conn = database::get_connection();
        let mut statement = conn
            .prepare("SELECT body, disabled FROM UserCommands WHERE command = ?")
            .expect("valid sql");

        let mut result = statement
            .query_map(&[&req.args()], |row| Command {
                body: row.get(0),
                disabled: row.get(1),
            })
            .expect("valid sql");

        match result.next() {
            Some(Ok(ref command)) if !command.disabled => say!(command.body),
            _ => None,
        }
    }

    fn add_command(&mut self, req: &Request) -> Option<Response> {
        require_privileges!(&req, "you cannot do that");

        let (command, body) = match Self::arg_parts(&req) {
            Some((head, tail)) => (head, tail),
            None => return reply_template!("misc_invalid_args"),
        };

        let command = if !command.starts_with('!') {
            format!("!{}", command)
        } else {
            command
        };

        if !Self::is_available(&command) {
            return reply_template!("builtin_reserved_name", ("command", &command));
        }

        let command = UserCommand {
            command,
            body,
            description: "no description provided".into(),
            creator: req.sender(),
            created_at: Utc::now().timestamp(),
            uses: 0,
            disabled: false,
        };

        let conn = database::get_connection();
        match conn.execute(
            r#"INSERT OR IGNORE INTO UserCommands (
                command, body, description, creator, 
                created_at, uses, disabled
            ) VALUES (?, ?, ?, ?, ?, ?, ?)            
            "#,
            &[
                &command.command as &dyn ToSql,
                &command.body,
                &command.description,
                &command.creator,
                &command.created_at,
                &command.uses,
                &command.disabled,
            ],
        ) {
            Ok(row) if row == 0 => {
                reply_template!("builtin_already_exists", ("command", &command.command))
            }
            Ok(_row) => reply_template!("builtin_added_command", ("command", &command.command)),
            Err(err) => {
                // this isn't really reachable, but unsafe code is unsafe
                warn!(
                    "{} tried to add '{}' as a command, sql error: {}",
                    req.sender(),
                    command.command,
                    err
                );
                reply_template!("builtin_add_failed", ("command", &command.command))
            }
        }
    }

    fn edit_command(&mut self, req: &Request) -> Option<Response> {
        require_privileges!(&req, "you cannot do that");

        let (command, description) = match Self::arg_parts(&req) {
            Some((head, tail)) => (head, tail),
            None => return reply_template!("misc_invalid_args"),
        };

        let conn = database::get_connection();
        match conn.execute(
            "UPDATE UserCommands SET description = ? WHERE command = ?",
            &[&description as &dyn ToSql, &command],
        ) {
            Ok(_row) => reply_template!("builtin_edited", ("command", &command)),
            Err(err) => {
                warn!(
                    "{} tried to edit '{}', sql error: {}",
                    req.sender(),
                    command,
                    err
                );
                reply_template!("builtin_edited_failed", ("command", &command))
            }
        }
    }

    fn info_command(&mut self, req: &Request) -> Option<Response> {
        require_privileges!(&req, "you cannot do that");

        let command = match Self::try_get_command(req.args()) {
            None => return reply_template!("builtin_invalid_command", ("command", &req.args())),
            Some(command) => command,
        };

        // hmm
        let conn = database::get_connection();
        let user = match UserStore::get_user_by_id(&conn, command.creator) {
            Some(user) => user.display,
            None => "unknown".to_string(),
        };

        let end = reply_template!(
            "builtin_command_created_at",
            ("user", &user),
            ("uses", &command.uses.commas()),
        );

        eprintln!("{:?}", end);

        multi!(
            reply_template!(
                "builtin_command_description",
                ("command", &command.command),
                ("description", &command.description)
            ),
            reply!(command.body),
            end
        )
    }

    fn remove_command(&mut self, req: &Request) -> Option<Response> {
        require_privileges!(&req, "you cannot do that");

        let (command, _) = match Self::arg_parts(&req) {
            Some((head, tail)) => (head, tail),
            None => return reply_template!("misc_invalid_args"),
        };

        let conn = database::get_connection();
        conn.execute("DELETE FROM UserCommands WHERE command = ?", &[&command])
            .expect("valid sql");

        reply_template!("builtin_command_deleted")
    }

    fn help_command(&mut self, _req: &Request) -> Option<Response> {
        fn wrap(input: impl IntoIterator<Item = String>) -> Vec<String> {
            const WIDTH: usize = 40;
            let (mut lines, mut line) = (vec![], String::new());
            for command in input.into_iter() {
                if line.len() + command.len() > WIDTH {
                    lines.push(line.clone());
                    line.clear();
                }
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(&command);
            }
            if !line.is_empty() {
                lines.push(line)
            }
            lines
        }

        // TODO look up specific commands

        let system = wrap(
            Registry::commands()
                .into_iter()
                .map(|cmd| cmd.name().to_string()),
        );
        let user = wrap(Self::fetch_command_names());

        multi!(
            whisper!("system commands:"),
            multi(system.iter().map(|s| whisper!(s))),
            whisper!("user commands:"),
            multi(user.iter().map(|s| whisper!(s))),
        )
    }

    pub fn try_get_command(name: &str) -> Option<UserCommand> {
        let conn = database::get_connection();
        let command = conn
            .prepare(
                r#"SELECT command, body, description, creator, created_at, uses, disabled                     
                    FROM UserCommands WHERE command = ?"#,
            )
            .expect("valid sql")
            .query_map(&[&name], |row| UserCommand {
                command: row.get(0),
                body: row.get(1),
                description: row.get(2),
                creator: row.get(3),
                created_at: row.get(4),
                uses: row.get(5),
                disabled: row.get(6)
            })
            .expect("valid sql")
            .next();

        command?.ok()
    }

    pub fn fetch_command_names() -> Vec<String> {
        let conn = database::get_connection();
        let mut commands = conn
            .prepare("SELECT command FROM UserCommands")
            .expect("valid sql")
            .query_map(NO_PARAMS, |row| row.get(0))
            .expect("valid sql")
            .filter_map(Result::ok)
            .collect::<Vec<_>>(); // TODO do this in sql

        commands.sort_unstable(); // TODO do this in sql
        commands
    }

    fn disable_bad_command(cmd: impl AsRef<str>) {
        let conn = database::get_connection();
        let command = cmd.as_ref();

        conn.execute(
            "UPDATE UserCommands SET disabled = ? WHERE command = ?",
            &[&true as &dyn ToSql, &command],
        )
        .expect("valid sql");

        info!("disabled bad command: {}", command);
    }

    fn is_available(cmd: impl AsRef<str>) -> bool {
        Registry::is_available(cmd)
    }

    fn arg_parts(req: &Request) -> Option<(String, String)> {
        let mut iter = req.args_iter();
        let head = iter.next()?.to_string();
        let tail = iter.map(str::trim).fold(String::new(), |mut acc, c| {
            if !acc.is_empty() {
                acc.push_str(" ");
            }
            acc.push_str(c);
            acc
        });
        Some((head, tail))
    }
    // end of user commands

    fn version_command(&mut self, _req: &Request) -> Option<Response> {
        let rev = option_env!("SHAKEN_GIT_REV").expect("get rev");
        let branch = option_env!("SHAKEN_GIT_BRANCH").expect("get branch");
        reply_template!("builtin_github_repo", ("rev", &rev), ("branch", &branch))
    }

    fn viewers_command(&mut self, _req: &Request) -> Option<Response> {
        let streams = self.twitch.get_streams(&[self.channel.as_str()]);
        let stream = match streams {
            Ok(ref s) if !s.is_empty() => &s[0],
            _ => return reply_template!("builtin_stream_offline"),
        };

        if stream.live.is_empty() || stream.live == "" {
            return reply_template!("builtin_stream_offline");
        }

        say_template!(
            "builtin_viewers",
            ("viewers", &stream.viewer_count.commas())
        )
    }

    fn uptime_command(&mut self, _req: &Request) -> Option<Response> {
        let streams = self.twitch.get_streams(&[self.channel.as_str()]);

        let stream = match streams {
            Ok(ref s) if !s.is_empty() => &s[0],
            _ => {
                debug!("cannot get stream");
                return reply_template!("builtin_stream_offline");
            }
        };

        if stream.live.is_empty() || stream.live == "" {
            return reply_template!("builtin_stream_offline");
        }

        let start = stream
            .started_at
            .parse::<DateTime<Utc>>()
            .expect("parse datetime");

        let dur = (Utc::now() - start)
            .to_std()
            .expect("convert to std duration");

        say_template!("builtin_uptime", ("uptime", &dur.as_readable_time()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;

    #[test]
    fn autojoin() {
        let db = database::get_connection();
        let mut builtin = Builtin::create().unwrap();
        let mut env = Environment::new(&db, &mut builtin);

        env.push_raw(":test.localhost 001 museun :Welcome to IRC");
        env.step();
        assert_eq!(env.pop_raw(), Some("JOIN #museun".into()));
    }

    #[test]
    fn autopong() {
        let db = database::get_connection();
        let mut builtin = Builtin::create().unwrap();
        let mut env = Environment::new(&db, &mut builtin);

        env.push_raw("PING :foobar");
        env.step();
        assert_eq!(env.pop_raw(), Some("PONG :foobar".into()));
    }

    #[test]
    fn version_command() {
        let db = database::get_connection();
        let mut builtin = Builtin::create().unwrap();
        let mut env = Environment::new(&db, &mut builtin);

        env.push("!version");
        env.step();

        assert!(env
            .pop()
            .unwrap()
            .starts_with("@test: https://github.com/museun/shaken"));
    }

    #[test]
    fn add_command() {
        let db = database::get_connection();
        let mut builtin = Builtin::create().unwrap();
        let mut env = Environment::new(&db, &mut builtin);

        env.push_owner("!add !test this is a test");
        env.step();
        assert_eq!(env.pop().unwrap(), "@test: added \"!test\" as a command");

        env.push_owner("!add !test this is a test");
        env.step();
        assert_eq!(
            env.pop().unwrap(),
            "@test: \"!test\" already exists as a command"
        );

        Registry::register(&CommandBuilder::command("!foo").namespace("bar").build())
            .expect("reserve foo");

        env.push_owner("!add !foo this is a test");
        env.step();
        assert_eq!(env.pop().unwrap(), "@test: \"!foo\" is a reserved name");
    }

    #[test]
    fn edit_command() {
        let db = database::get_connection();
        let mut builtin = Builtin::create().unwrap();
        let mut env = Environment::new(&db, &mut builtin);

        env.push_owner("!add !test this is a test");
        env.step();
        env.drain();

        env.push_owner("!edit !test with different flavor text");
        env.step();
        assert_eq!(env.pop().unwrap(), "@test: edited \"!test\"");

        let cmd = Builtin::try_get_command("!test").unwrap();
        assert_eq!(cmd.command, "!test".to_string());
        assert_eq!(cmd.description, "with different flavor text".to_string());
    }

    #[test]
    fn info_command() {
        let db = database::get_connection();
        let mut builtin = Builtin::create().unwrap();
        let mut env = Environment::new(&db, &mut builtin);

        env.push_owner("!info !test");
        env.step();
        assert_eq!(env.pop().unwrap(), "@test: \"!test\" isn't a command");

        env.push_owner("!add !test this is a test");
        env.step();
        env.drain();

        env.push_owner("!info !test");
        env.step();
        assert_eq!(
            env.pop().unwrap(),
            "@test: !test -- no description provided"
        );
        assert_eq!(env.pop().unwrap(), "@test: this is a test");
        let res = env.pop().unwrap();
        assert!(res.starts_with("@test: created by test"));
    }

    #[test]
    fn remove_command() {
        let db = database::get_connection();
        let mut builtin = Builtin::create().unwrap();
        let mut env = Environment::new(&db, &mut builtin);

        env.push_owner("!remove !test");
        env.step();
        assert_eq!(
            env.pop().unwrap(),
            "@test: if that was a command, its no longer one"
        );

        env.push_owner("!add !test this is a test");
        env.step();
        env.drain();

        env.push_owner("!remove !test");
        env.step();
        assert_eq!(
            env.pop().unwrap(),
            "@test: if that was a command, its no longer one"
        );
    }

    #[test]
    fn help_command() {
        let db = database::get_connection();
        let mut builtin = Builtin::create().unwrap();
        let mut env = Environment::new(&db, &mut builtin);

        use rand::distributions::Alphanumeric;
        use rand::prelude::*;
        let mut rng = thread_rng();

        let mut next = || {
            let n = rng.gen_range(3, 8);
            std::iter::repeat(())
                .map(|_| rng.sample(Alphanumeric))
                .take(n)
                .collect::<String>()
        };

        for _ in 0..20 {
            env.push_owner(&format!("!add !{} foobar", next()));
            env.step();
        }
        env.drain();

        env.push_owner("!help !test");
        env.step();

        let expected = Registry::commands().len() // system
                        + Builtin::fetch_command_names().len(); // user

        let mut max = 0;
        while let Some(n) = env.pop() {
            max += n.chars().filter(|&c| c == '!').count()
        }

        assert_eq!(max, expected);
    }

    #[allow(dead_code)]
    fn dump() {
        use rusqlite::{types::ValueRef, NO_PARAMS};

        let conn = database::get_connection();
        let mut statement = conn
            .prepare("select * from usercommands")
            .expect("valid sql");
        let mut rows = statement.query(NO_PARAMS).expect("valid sql");
        while let Some(Ok(row)) = rows.next() {
            let mut s = String::new();
            for n in 0..row.column_count() {
                if !s.is_empty() {
                    s.push(' ');
                }
                s.push_str(&match row.get_raw(n) {
                    ValueRef::Null => "null".into(),
                    ValueRef::Integer(n) => format!("{}", n),
                    ValueRef::Real(n) => format!("{}", n),
                    ValueRef::Text(s) => s.into(),
                    ValueRef::Blob(b) => format!("{:?}", b),
                });
            }
            debug!("{}", s)
        }
    }
}
