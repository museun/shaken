use crate::{bot, config};

pub struct Builtin;

impl Builtin {
    pub fn new(bot: &bot::Bot, config: &config::Config) -> Self {
        bot.on_raw("PING", |bot, msg| {
            bot.proto().send(&format!("PONG :{}", &msg.data));
        });

        let channels = config.twitch.channels.to_vec();
        bot.on_raw("001", move |bot, _msg| {
            let proto = bot.proto();
            for ch in &channels {
                proto.join(ch)
            }
        });

        bot.on_command("!version", |bot, env| {
            let rev = option_env!("SHAKEN_GIT_REV").unwrap();
            let branch = option_env!("SHAKEN_GIT_BRANCH").unwrap();

            let msg = format!(
                "https://github.com/museun/shaken ({} on '{}' branch)",
                rev, branch
            );

            bot.say(&env, &msg);
        });

        bot.on_command("!shaken", |bot, env| {
            bot.say(
                &env,
                "I try to impersonate The Bard, by being trained on all of his works.",
            );
        });

        bot.on_command("!commands", |bot, env| {
            let commands = bot.get_commands();
            let commands = ::util::join_with(commands.iter(), " ");
            bot.say(&env, &format!("available commands: {}", commands));
        });

        Self {}
    }
}
