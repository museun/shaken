// this is used so modules can express their commands
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Command {
    name: String,
    help: String,
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

    pub fn build(self) -> Command {
        // TODO assert all this stuff
        Command {
            name: self.name.unwrap(),
            help: self.help.or_else(|| Some("".into())).unwrap(),
        }
    }
}
