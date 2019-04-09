use crate::prelude::*;

use rusqlite::{Connection, NO_PARAMS};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Error {
    AlreadyExists,
}

#[derive(Debug, PartialEq, PartialOrd)]
pub struct Command {
    name: String,
    help: String,
    namespace: String,
}

impl Command {
    pub fn replace_name(&mut self, name: impl ToString) {
        self.name = name.to_string()
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn help(&self) -> &str {
        self.help.as_str()
    }

    pub fn has_help(&self) -> bool {
        self.help != "no help provided" // TODO make this some sigil value (or an Option)
    }

    pub fn namespace(&self) -> &str {
        self.namespace.as_str()
    }
}

#[derive(Default)]
pub struct CommandBuilder {
    name: String,
    help: Option<String>,
    namespace: Option<String>, // for future use
}

impl CommandBuilder {
    pub fn command<S>(name: S) -> Self
    where
        S: ToString,
    {
        Self {
            name: name.to_string(),
            ..Self::default()
        }
    }

    pub fn help<S>(mut self, help: S) -> Self
    where
        S: ToString,
    {
        std::mem::replace(&mut self.help, Some(help.to_string()));
        self
    }

    pub fn namespace<S>(mut self, namespace: S) -> Self
    where
        S: ToString,
    {
        std::mem::replace(&mut self.namespace, Some(namespace.to_string()));
        self
    }

    pub fn build(self) -> Command {
        Command {
            name: self.name,
            help: self.help.unwrap_or_else(|| "no help provided".into()),
            namespace: self.namespace.expect("namespace is required"),
        }
    }
}

pub struct Registry;

impl Registry {
    fn ensure_table(conn: &Connection) {
        conn.execute(
            r#"CREATE TABLE IF NOT EXISTS CommandRegistry(
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                command         TEXT NOT NULL,
                description     TEXT NOT NULL,
                namespace       TEXT NOT NULL
            )"#,
            NO_PARAMS,
        )
        .expect("create CommandRegistry table");
    }

    pub fn commands() -> Vec<Command> {
        let conn = database::get_connection();
        Self::ensure_table(&conn);

        let mut s = conn
            .prepare("SELECT command, description, namespace FROM CommandRegistry")
            .expect("valid sql");

        s.query_map(NO_PARAMS, |row| {
            Ok(Command {
                name: row.get(0)?,
                help: row.get(1)?,
                namespace: row.get(2)?,
            })
        })
        .expect("valid sql")
        .filter_map(Result::ok)
        .collect()
    }

    pub fn is_available(name: impl AsRef<str>) -> bool {
        let name = name.as_ref();
        !Self::commands().iter().any(|cmd| cmd.name == name)
    }

    #[must_use]
    pub fn register(cmd: &Command) -> Result<(), Error> {
        let conn = database::get_connection();
        Self::ensure_table(&conn);

        struct Command {
            name: String,
            namespace: String,
        }

        let mut s = conn
            .prepare("SELECT command, namespace FROM CommandRegistry")
            .expect("valid sql");

        let commands = s
            .query_map(NO_PARAMS, |row| {
                Ok(Command {
                    name: row.get(0)?,
                    namespace: row.get(1)?,
                })
            })
            .expect("valid sql")
            .filter_map(Result::ok);

        for command in commands {
            if command.name == cmd.name {
                if command.namespace != cmd.namespace {
                    return Err(Error::AlreadyExists);
                } else {
                    return Ok(());
                }
            }
        }

        conn.execute(
            "INSERT INTO CommandRegistry (command, description, namespace) VALUES (?1, ?2, ?3)",
            &[&cmd.name, &cmd.help, &cmd.namespace],
        )
        .expect("valid sql");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_registry() {
        let _conn = database::get_connection(); // to keep the in-memory db alive

        let cmd = CommandBuilder::command("!test").namespace("test").build();
        assert_eq!(Registry::commands().len(), 0);
        Registry::register(&cmd).unwrap();

        let commands = Registry::commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], cmd);

        let mut cmd = CommandBuilder::command("!test").namespace("test1").build();
        let err = Registry::register(&cmd).unwrap_err();
        assert_eq!(err, Error::AlreadyExists);

        cmd.replace_name("!test1");
        Registry::register(&cmd).unwrap();

        let commands = Registry::commands();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[1], cmd);
    }
}
