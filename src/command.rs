// this is used so modules can express their commands
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Command {
    name: String,
    help: String,
    subs: Vec<Box<Command>>,
}

impl Command {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn help(&self) -> &str {
        &self.help
    }
}

#[derive(Default)]
pub struct CommandBuilder {
    name: Option<String>,
    help: Option<String>,
    subs: Vec<Command>,
}

impl CommandBuilder {
    pub fn new() -> Self {
        CommandBuilder::default()
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn help(mut self, help: &str) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn children<V>(mut self, v: V) -> Self
    where
        V: AsRef<[Command]>,
    {
        self.subs.clone_from_slice(v.as_ref());
        self
    }

    pub fn build(self) -> Command {
        // TODO assert all this stuff
        Command {
            name: self.name.unwrap(),
            help: self.help.or_else(|| Some("".into())).unwrap(),
            subs: self.subs.into_iter().map(Box::new).collect(),
        }
    }
}
