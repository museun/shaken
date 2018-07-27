#![allow(dead_code, unused_variables)] // go away
use crate::{bot, config, message};

use parking_lot::{Mutex, RwLock};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time;

pub struct Poll {
    poll: RwLock<Option<TwitchPoll>>,
    tick: Mutex<time::Instant>,
    running: AtomicBool,
    duration: AtomicUsize,
}

impl Poll {
    pub fn new(bot: &bot::Bot, _config: &config::Config) -> Arc<Self> {
        let this = Arc::new(Self {
            poll: RwLock::new(None),
            tick: Mutex::new(time::Instant::now()),
            running: AtomicBool::new(false),
            duration: AtomicUsize::new(30),
        });

        let next = Arc::clone(&this);
        bot.on_command("!poll", move |bot, env| {
            if !bot.is_owner_id(env.get_id().unwrap()) {
                debug!("{} tried to use the poll command", env.get_id().unwrap());
                return;
            }

            // TODO add in proper subcommand processing
            if (env.data.len() >= 4 && &env.data[..4] == "stop")
                || (env.data.len() >= 5 && &env.data[..5] == "start")
            {
                // just to stop the subcommands from being trigered here
                return;
            }

            if next.running.load(Ordering::Relaxed) {
                warn!("poll is already running");
                bot.say(&env, "poll is already running, stopping it");
                next.running.store(false, Ordering::Relaxed);
            }

            trace!("collecting the options");
            let options = Self::collect_options(&env.data);
            bot.say(&env, "is this poll right?");
            trace!("verifying poll is right");
            options
                .iter()
                .enumerate()
                .map(|(i, s)| format!("#{}: {}", i + 1, s))
                .for_each(|opt| bot.say(&env, &opt));

            let poll = TwitchPoll::new(&env, &options);
            *next.poll.write() = Some(poll);
        });

        let next = Arc::clone(&this);
        bot.on_command("!poll start", move |bot, env| {
            if !bot.is_owner_id(env.get_id().unwrap()) {
                debug!("{} tried to start the poll", env.get_id().unwrap());
                return;
            }

            if next.running.load(Ordering::Relaxed) {
                warn!("poll is already running");
                bot.say(&env, "poll is already running");
                return;
            }

            let poll = { next.poll.read() };
            if poll.is_none() {
                debug!("no poll was configured");
                bot.say(&env, "no poll set up");
                return;
            }

            let n: String = env
                .data
                .chars()
                .skip_while(|&c| c == '#')
                .take_while(char::is_ascii_digit)
                .collect();

            let pos = if let Ok(n) = n.parse::<usize>() {
                n
            } else {
                next.duration.load(Ordering::Relaxed)
            };

            next.duration.store(pos, Ordering::Relaxed);

            bot.say(
                &env,
                &format!(
                    "starting poll for the next {} seconds. use '!vote n' to vote for that option",
                    next.duration.load(Ordering::Relaxed)
                ),
            );

            info!("starting the poll");
            *next.tick.lock() = time::Instant::now();
            next.running.store(true, Ordering::Relaxed);
        });

        let next = Arc::clone(&this);
        bot.on_command("!poll stop", move |bot, env| {
            if !bot.is_owner_id(env.get_id().unwrap()) {
                debug!("{} tried to stop the poll", env.get_id().unwrap());
                return;
            }

            if !next.running.load(Ordering::Relaxed) {
                warn!("poll isn't running");
                bot.say(&env, "poll isn't running");
                return;
            }

            info!("stopping the poll");
            next.running.store(false, Ordering::Relaxed);
            bot.say(&env, "stopped the poll");
        });

        let next = Arc::clone(&this);
        bot.on_command("!vote", move |bot, env| {
            if !next.running.load(Ordering::Relaxed) {
                // poll isn't running
                return;
            }

            let who = env.get_id();
            if who.is_none() {
                return;
            }
            let who = who.unwrap();

            if let Some(data) = env.data.split_whitespace().take(1).next() {
                let n: String = data
                    .chars()
                    .skip_while(|&c| c == '#')
                    .take_while(char::is_ascii_digit)
                    .collect();

                if let Ok(n) = n.parse::<usize>() {
                    if n == 0 {
                        return;
                    }

                    trace!("trying to vote for: {}", n - 1);
                    if let Some(ref mut poll) = *next.poll.write() {
                        poll.vote(&who, n - 1)
                    }
                }
            }
        });

        let next = Arc::clone(&this);
        bot.on_tick(move |bot| {
            if !next.running.load(Ordering::Relaxed) {
                // the poll isn't running
                return;
            }

            let then = next.tick.lock();
            if time::Instant::now() - *then
                < time::Duration::from_secs(next.duration.load(Ordering::Relaxed) as u64)
            {
                return;
            }

            info!("tallying the poll");
            next.running.store(false, Ordering::Relaxed);

            // this doesn't need an if let
            if let Some(ref mut poll) = *next.poll.write() {
                let target = poll.target.clone(); // might as well clone it

                let iter = poll.tally().iter().take(3).enumerate();
                iter.for_each(|(i, opt)| {
                    bot.say(
                        &target,
                        &format!("({} votes) #{} {}", opt.count, opt.position + 1, opt.option),
                    )
                });
            }
        });

        this
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
    target: message::Envelope,
    options: Vec<Choice>,
    seen: Vec<String>, // maybe use a hash set here
}

impl TwitchPoll {
    pub fn new<S>(target: &message::Envelope, options: &[S]) -> Self
    where
        S: AsRef<str>,
    {
        Self {
            target: target.clone(),
            options: options
                .iter()
                .enumerate()
                .map(|(i, f)| Choice {
                    option: f.as_ref().to_string(),
                    position: i,
                    count: 0,
                })
                .collect(),
            seen: vec![],
        }
    }

    pub fn vote(&mut self, who: &str, option: usize) {
        let who = who.to_string();
        if self.seen.contains(&who) {
            trace!("{} already voted", &who);
            // already voted
            return;
        }

        if let Some(n) = self.options.get_mut(option) {
            self.seen.push(who);
            n.count += 1;
            trace!("#{} is at {} now", n.count, option);
        }
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
    position: usize,
    count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;

    #[test]
    fn test_poll() {
        let mut env = Environment::new();
        let poll = Poll::new(&env.bot, &env.config);

        // these must match for the poll to work
        env.set_owner("23196011");
        env.set_user_id("0");

        env.push_privmsg(r#"!poll "option a" "option b" "option c""#);
        env.step();

        assert!(env.pop_env().is_none());

        env.set_owner("23196011");
        env.set_user_id("23196011");

        env.push_privmsg(r#"!poll "option a" "option b" "option c""#);
        env.step();

        assert_eq!(env.pop_env().unwrap().data, "#3: option c");
        assert_eq!(env.pop_env().unwrap().data, "#2: option b");
        assert_eq!(env.pop_env().unwrap().data, "#1: option a");
        assert_eq!(env.pop_env().unwrap().data, "is this poll right?");
        assert!(env.pop_env().is_none());

        let poll = poll.poll.read();
        assert!(poll.is_some());
        if let Some(ref poll) = *poll {
            assert_eq!(poll.options.len(), 3);
        }
    }

    #[test]
    fn test_poll_start() {
        let mut env = Environment::new();
        let poll = Poll::new(&env.bot, &env.config);

        poll.duration
            .store(1, ::std::sync::atomic::Ordering::Relaxed);

        env.set_owner("23196011");
        env.set_user_id("0");

        env.push_privmsg(r#"!poll start"#);
        env.step();
        assert!(env.pop_env().is_none());

        env.set_owner("23196011");
        env.set_user_id("23196011");

        env.push_privmsg(r#"!poll start"#);
        env.step();

        assert_eq!(env.pop_env().unwrap().data, "no poll set up");
        assert!(env.pop_env().is_none());

        env.push_privmsg(r#"!poll "option a" "option b" "option c""#);
        env.step();

        env.drain_envs();

        env.push_privmsg(r#"!poll start"#);
        env.step();

        assert_eq!(
            env.pop_env().unwrap().data,
            "starting poll for the next 1 seconds. use \'!vote n\' to vote for that option"
        );
        assert!(env.pop_env().is_none());

        ::std::thread::sleep(::std::time::Duration::from_secs(2));
        env.tick();

        assert_eq!(env.pop_env().unwrap().data, "(0 votes) #3 option c");
        assert_eq!(env.pop_env().unwrap().data, "(0 votes) #2 option b");
        assert_eq!(env.pop_env().unwrap().data, "(0 votes) #1 option a");
        assert!(env.pop_env().is_none());
    }

    #[test]
    fn test_poll_vote() {
        let mut env = Environment::new();
        let poll = Poll::new(&env.bot, &env.config);

        poll.duration
            .store(1, ::std::sync::atomic::Ordering::Relaxed);

        env.set_owner("23196011");
        env.set_user_id("23196011");

        env.push_privmsg(r#"!poll "option a" "option b" "option c""#);
        env.step();
        env.drain_envs();

        env.push_privmsg("!poll start");
        env.step();
        env.drain_envs();

        for i in 0..10 {
            env.set_user_id(&format!("{}", i));
            env.push_privmsg(&format!("!vote {}", i % 3));
            env.step();
        }

        ::std::thread::sleep(::std::time::Duration::from_secs(2));
        env.tick();

        assert_eq!(env.pop_env().unwrap().data, "(0 votes) #3 option c");
        assert_eq!(env.pop_env().unwrap().data, "(3 votes) #2 option b");
        assert_eq!(env.pop_env().unwrap().data, "(3 votes) #1 option a");
        assert!(env.pop_env().is_none());
    }

    #[test]
    fn test_poll_stop() {
        let mut env = Environment::new();
        let poll = Poll::new(&env.bot, &env.config);

        env.set_owner("23196011");
        env.set_user_id("23196011");

        env.push_privmsg("!poll stop");
        env.step();
        assert_eq!(env.pop_env().expect("a").data, "poll isn't running");
        assert!(env.pop_env().is_none());

        env.push_privmsg(r#"!poll "option a" "option b" "option c""#);
        env.step();
        env.drain_envs();

        env.push_privmsg("!poll stop");
        env.step();
        assert_eq!(env.pop_env().unwrap().data, "poll isn't running");
        assert!(env.pop_env().is_none());

        env.push_privmsg("!poll start");
        env.step();
        env.drain_envs();

        env.push_privmsg("!poll stop");
        env.step();
        assert_eq!(env.pop_env().unwrap().data, "stopped the poll");
        assert!(env.pop_env().is_none());

        env.push_privmsg("!poll stop");
        env.step();
        assert_eq!(env.pop_env().unwrap().data, "poll isn't running");
        assert!(env.pop_env().is_none());
    }
}
