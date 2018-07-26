#![allow(dead_code, unused_variables)] // go away
use {bot, config, message};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time;

pub struct Poll {
    poll: RwLock<Option<TwitchPoll>>,
    running: AtomicBool,
}

impl Poll {
    pub fn new(bot: &bot::Bot, _config: &config::Config) -> Arc<Self> {
        let this = Arc::new(Self {
            poll: RwLock::new(None),
            running: AtomicBool::new(false),
        });

        let next = Arc::clone(&this);

        // TODO add in proper subcommand processing
        bot.on_command("!poll", move |bot, env| {
            if !bot.is_owner_id(env.get_id().unwrap()) {
                return;
            }

            if (env.data.len() == 4 && &env.data[..4] == "stop")
                || (env.data.len() == 5 && &env.data[..5] == "start")
            {
                // just to stop the subcommands from being trigered here
                return;
            }

            if next.running.load(Ordering::Relaxed) {
                bot.say(&env, "poll is already running, stopping it");
                next.running.store(false, Ordering::Relaxed);
            }

            let options = Self::collect_options(&env.data);
            bot.say(&env, "is this poll right?");
            options
                .iter()
                .enumerate()
                .map(|(i, s)| format!("#{}: {}", i + 1, s))
                .for_each(|opt| bot.say(&env, &opt));

            let poll = TwitchPoll::new(&options);
            *next.poll.write().unwrap() = Some(poll);
        });

        let next = Arc::clone(&this);
        bot.on_command("!poll start", move |bot, env| {
            if !bot.is_owner_id(env.get_id().unwrap()) {
                return;
            }

            if next.running.load(Ordering::Relaxed) {
                bot.say(&env, "poll is already running");
                return;
            }

            let poll = { next.poll.read().unwrap() };
            if poll.is_none() {
                bot.say(&env, "no poll set up");
                return;
            }

            bot.say(
                &env,
                "starting poll for the next 30 seconds. use '!vote n' to vote for that option",
            );
            info!("start poll");

            next.run_poll(bot, env);
        });

        let next = Arc::clone(&this);
        bot.on_command("!poll stop", move |bot, env| {
            if !bot.is_owner_id(env.get_id().unwrap()) {
                return;
            }

            if !next.running.load(Ordering::Relaxed) {
                bot.say(&env, "poll isn't running");
                return;
            }
            info!("stopping poll");
            next.running.store(false, Ordering::Relaxed);
        });

        let next = Arc::clone(&this);
        bot.on_command("!vote", move |bot, env| {
            if !next.running.load(Ordering::Relaxed) {
                // poll isn't running
                return;
            }

            if let Some(who) = env.get_id() {
                if let Some(data) = env.data.split_whitespace().take(1).next() {
                    let n: String = data
                        .chars()
                        .skip_while(|c| !c.is_ascii_digit())
                        .take_while(char::is_ascii_digit)
                        .collect();

                    if let Ok(n) = n.parse::<usize>() {
                        if let Some(ref mut poll) = *next.poll.write().unwrap() {
                            poll.vote(&who, n)
                        }
                    }
                }
            }
        });

        this
    }

    fn run_poll(&self, bot: &bot::Bot, env: &message::Envelope) {
        let now = time::Instant::now();
        thread::sleep(time::Duration::from_secs(30));
        info!("finished poll");
        self.running.store(false, Ordering::Relaxed);

        if let Some(ref mut poll) = *self.poll.write().unwrap() {
            poll.tally()
                .iter()
                .take(3)
                .enumerate()
                .inspect(|s| debug!("{:#?}", s))
                .for_each(|(i, opt)| {
                    bot.say(
                        &env,
                        &format!("#{} with {}: {}", i + 1, opt.count, opt.option),
                    )
                });
        }
    }

    fn collect_options(input: &str) -> Vec<String> {
        enum State {
            Start,
            Middle,
            End, // need more transistions
        }

        let mut state = State::Start;

        let mut options = vec![];
        let mut buf = String::new();

        let mut pos = 0;

        'parse: loop {
            match state {
                State::Start => {
                    'eat: loop {
                        match input.get(pos..pos + 1) {
                            Some("\"") | Some(" ") => pos += 1,
                            None => break 'parse,
                            s => break 'eat,
                        };
                    }
                    state = State::Middle;
                    continue;
                }
                State::Middle => {
                    'add: loop {
                        match input.get(pos..pos + 1) {
                            Some("\"") => {
                                pos += 1;
                                break 'add;
                            }
                            None => break 'parse,
                            Some("") => {
                                pos += 1;
                                continue;
                            }
                            _ => {
                                buf.push_str(&input[pos..pos + 1]);
                                pos += 1;
                            }
                        };
                    }
                    state = State::End;
                    continue;
                }
                State::End => {
                    options.push(buf.trim().to_string());
                    buf.clear();
                    state = State::Start;
                    continue;
                }
            };
        }

        // just incase
        let buf = buf.trim();
        if !buf.is_empty() {
            options.push(buf.to_string());
        }

        options
    }
}

#[derive(Debug, Clone)]
struct TwitchPoll {
    options: Vec<Choice>,
    seen: Vec<String>, // maybe use a hash set here
}

impl TwitchPoll {
    pub fn new<S>(options: &[S]) -> Self
    where
        S: AsRef<str>,
    {
        Self {
            options: options
                .iter()
                .map(|f| Choice {
                    option: f.as_ref().to_string(),
                    count: 0,
                })
                .collect(),
            seen: vec![],
        }
    }

    pub fn vote(&mut self, who: &str, option: usize) {
        if option > self.options.len() {
            // invalid choice
            return;
        }

        let who = who.to_string();
        if self.seen.contains(&who) {
            // already voted
            return;
        }

        self.seen.push(who);
        self.options[option].count += 1;
    }

    // this sorts the options
    // this probably can't be borrowed
    pub fn tally(&mut self) -> &Vec<Choice> {
        self.options.sort_by(|l, r| r.count.cmp(&l.count));
        &self.options
    }
}

#[derive(Debug, Clone)]
struct Choice {
    option: String,
    count: usize,
}
