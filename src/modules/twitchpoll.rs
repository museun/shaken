use crate::*;
use parking_lot::Mutex;
use std::{
    collections::HashSet,
    fmt, str,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::{Duration, Instant},
};

pub struct TwitchPoll {
    poll: Mutex<Option<Poll>>,
    start: Mutex<Option<Instant>>,
    duration: AtomicUsize,
    running: AtomicBool, // this is so we don't have to lock the mutex every tick
    commands: Vec<Command<TwitchPoll>>,
}

impl Module for TwitchPoll {
    fn command(&self, req: &Request) -> Option<Response> {
        dispatch_commands!(&self, &req)
    }

    fn tick(&self, dt: Instant) -> Option<Response> {
        self.handle_tick(dt)
    }
}

impl Default for TwitchPoll {
    fn default() -> Self {
        Self::new()
    }
}

impl TwitchPoll {
    pub fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            poll: Mutex::new(None),
            start: Mutex::new(None),
            duration: AtomicUsize::new(0),

            commands: command_list!(
                ("!poll", Self::poll_command),
                ("!poll start", Self::poll_start_command),
                ("!poll stop", Self::poll_stop_command),
                ("!vote", Self::poll_vote_command),
            ),
        }
    }

    fn poll_command(&self, req: &Request) -> Option<Response> {
        let req = require_owner!(&req);
        let poll = match Self::parse_poll(req.target(), req.args()) {
            Ok(poll) => poll,
            Err(err) => return reply!("{}", err),
        };

        if self.running.load(Ordering::Relaxed) {
            return reply!("poll is already running. use !poll stop to stop it");
        }

        // ask for verification that the poll is right
        let res = multi!(
            say!("is this poll correct?"),
            say!("{}", &poll.title),
            multi(
                poll.choices
                    .iter()
                    .enumerate()
                    .map(|(i, s)| say!("#{}: {}", i + 1, s.option))
            )
        );

        *self.poll.lock() = Some(poll);
        res
    }

    fn poll_start_command(&self, req: &Request) -> Option<Response> {
        let req = require_owner!(&req);

        let poll = self.poll.lock();
        if poll.is_none() {
            return reply!("no poll has been configured. use !poll title | options | ...");
        }

        let dur = match req.args_iter().next().and_then(|a| {
            a.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse::<usize>()
                .ok()
        }) {
            Some(n) => n,
            None => return reply!("I don't know how long that is"),
        };

        self.running.store(true, Ordering::Relaxed);
        self.duration.store(dur, Ordering::Relaxed);
        let _ = { self.start.lock().get_or_insert(Instant::now()) };

        say!(
            "starting the poll for the next {} seconds. use '!vote n' to vote for that option",
            dur
        )
    }

    fn poll_stop_command(&self, req: &Request) -> Option<Response> {
        let req = require_owner!(&req);

        if !self.running.load(Ordering::Relaxed) {
            return reply!("no poll is running");
        }

        info!("stopping poll");
        self.running.store(false, Ordering::Relaxed);
        self.duration.store(0, Ordering::Relaxed);
        let _ = { self.start.lock().take() };
        let _ = { self.poll.lock().take() };

        None
    }

    fn poll_vote_command(&self, req: &Request) -> Option<Response> {
        debug!("{:#?}", self.poll);
        debug!("{:#?}", self.start);
        debug!("{:#?}", self.duration);
        debug!("{:#?}", self.running);

        if !self.running.load(Ordering::Relaxed) {
            debug!("poll not running");
            return None;
        }

        let poll = &mut *self.poll.lock();
        let poll = poll.as_mut().expect("poll to be configured");
        let max = poll.choices.len();

        let n = match req.args_iter().next().and_then(|a| {
            a.chars()
                .skip_while(|&c| c == '#')
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse::<usize>()
                .ok()
        }) {
            Some(n) if n == 0 => return reply!("what option is that?"),
            Some(n) if n > max => return reply!("what option is that?"),
            Some(n) => n,
            None => return reply!("what option is that?"),
        };

        trace!("attempting to vote for {}", n);
        poll.vote(req.sender(), n - 1);

        None
    }

    fn handle_tick(&self, dt: Instant) -> Option<Response> {
        if !self.running.load(Ordering::Relaxed) || self.start.lock().is_none() {
            return None;
        }

        let dt = Instant::now(); // don't trust the delta

        let deadline = Duration::from_secs(self.duration.load(Ordering::Relaxed) as u64);
        if let Some(start) = *self.start.lock() {
            warn!("{:?} - {:?} < {:?}", dt, start, deadline);
            if dt - start < deadline {
                return None;
            }
        }

        info!("tallying the poll");
        self.running.store(false, Ordering::Relaxed);

        // clean up the mutexes
        let mut poll = {
            self.start.lock().take();

            let poll = &mut *self.poll.lock();
            let poll = poll.take();
            poll.expect("poll to be running")
        };

        let target = poll.target.clone(); // this is dumb
        let res = poll.tally().iter().take(3).enumerate().map(|(i, opt)| {
            privmsg!(
                &target,
                "({} votes) #{} {}",
                opt.count,
                opt.pos + 1,
                opt.option
            )
        });

        multi(res)
    }

    fn parse_poll(target: &str, data: &str) -> Result<Poll, ParseError> {
        let mut iter = data
            .split('|')
            .map(str::trim)
            .map(|s| if s.is_empty() { None } else { Some(s) })
            .filter_map(|s| s);

        let title = iter.next().ok_or_else(|| ParseError::Title)?;
        let options = iter.collect::<Vec<_>>();
        if options.is_empty() {
            return Err(ParseError::Options);
        }

        Ok(Poll::new(target, title, &options))
    }
}

#[derive(Debug, PartialEq)]
enum ParseError {
    Title,
    Options,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::Title => write!(f, "no title provided"),
            ParseError::Options => write!(f, "no options provided"),
        }
    }
}

#[derive(Debug)]
struct Poll {
    target: String,
    title: String,
    choices: Vec<Choice>,
    seen: HashSet<i64>,
}

#[derive(Debug, Clone)]
struct Choice {
    pos: usize,
    count: usize,
    option: String,
}

impl Poll {
    pub fn new<S, V>(target: S, title: S, choices: V) -> Self
    where
        S: AsRef<str>,
        V: AsRef<[S]>,
    {
        Self {
            target: target.as_ref().into(),
            title: title.as_ref().into(),
            choices: choices
                .as_ref()
                .iter()
                .enumerate()
                .map(|(i, f)| Choice {
                    option: f.as_ref().to_string(),
                    pos: i,
                    count: 0,
                }).collect(),
            seen: HashSet::new(),
        }
    }

    pub fn vote(&mut self, id: i64, option: usize) {
        if self.seen.contains(&id) {
            trace!("{} already voted", id);
            return;
        }

        if let Some(n) = self.choices.get_mut(option) {
            self.seen.insert(id);
            n.count += 1;
            trace!("{} is at {}", n.pos, n.count);
        }
    }

    pub fn tally(&mut self) -> &Vec<Choice> {
        self.choices.sort_by(|l, r| r.count.cmp(&l.count));
        &self.choices
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;

    #[test]
    fn parse_poll() {
        let poll = TwitchPoll::parse_poll(
            "#testing",
            "this is a test | option a|option b            |          option c",
        );
        let poll = poll.unwrap();
        assert_eq!(poll.title, "this is a test".to_owned());
        let expected = vec!["option a", "option b", "option c"];
        poll.choices
            .iter()
            .enumerate()
            .for_each(|(i, c)| assert_eq!(expected[i], c.option));
    }

    #[test]
    fn poll_command() {
        let poll = TwitchPoll::new();
        let mut env = Environment::new();
        env.add(&poll);

        env.push("!poll");
        env.step();
        assert_eq!(env.pop(), None);

        env.push_owner("!poll");
        env.step();
        assert_eq!(env.pop(), Some("@test: no title provided".into()));

        env.push_owner("!poll test poll");
        env.step();
        assert_eq!(env.pop(), Some("@test: no options provided".into()));

        env.push_owner("!poll test poll | option a | option b");
        env.step();
        assert_eq!(env.pop(), Some("is this poll correct?".into()));
        assert_eq!(env.pop(), Some("test poll".into()));
        assert_eq!(env.pop(), Some("#1: option a".into()));
        assert_eq!(env.pop(), Some("#2: option b".into()));
    }

    #[test]
    fn poll_start_command() {
        let poll = TwitchPoll::new();
        let mut env = Environment::new();
        env.add(&poll);

        env.push("!poll start");
        env.step();
        assert_eq!(env.pop(), None);

        env.push_owner("!poll start");
        env.step();
        assert_eq!(
            env.pop(),
            Some("@test: no poll has been configured. use !poll title | options | ...".into())
        );

        env.push_owner("!poll test poll | option a | option b");
        env.step();
        env.drain();

        env.push_owner("!poll start");
        env.step();
        assert_eq!(
            env.pop(),
            Some("@test: I don't know how long that is".into())
        );

        env.push_owner("!poll start 160");
        env.step();
        assert_eq!(
            env.pop(),
            Some(
                "starting the poll for the next 160 seconds. use '!vote n' to vote for that option"
                    .into()
            )
        );
    }

    #[test]
    fn poll_stop_command() {
        let poll = TwitchPoll::new();
        let mut env = Environment::new();
        env.add(&poll);

        env.push("!poll stop");
        env.step();
        assert_eq!(env.pop(), None);

        env.push_owner("!poll stop");
        env.step();
        assert_eq!(env.pop(), Some("@test: no poll is running".into()));

        env.push_owner("!poll test poll | option a | option b");
        env.step();
        env.drain();

        env.push_owner("!poll start 160");
        env.step();
        env.drain();

        env.push_owner("!poll stop");
        env.step();

        assert!(poll.poll.lock().is_none());
        assert!(poll.start.lock().is_none());
        assert_eq!(poll.duration.load(Ordering::Relaxed), 0);
        assert_eq!(poll.running.load(Ordering::Relaxed), false);

        env.push_owner("!poll stop");
        env.step();
        assert_eq!(env.pop(), Some("@test: no poll is running".into()));
    }

    #[test]
    fn poll_vote_command() {
        let poll = TwitchPoll::new();
        let mut env = Environment::new();
        env.add(&poll);

        env.push("!poll vote");
        env.step();
        assert_eq!(env.pop(), None);

        env.push_owner("!poll test poll | option a | option b");
        env.step();

        env.push_owner("!poll start 1");
        env.step();

        env.push_user("!vote 1", ("test", 1001));
        env.step();

        env.push_user("!vote 2", ("test", 1002));
        env.step();

        env.push_user("!vote 3", ("test", 1003));
        env.step();

        env.push_user("!vote 1", ("test", 1003));
        env.step();

        env.push_user("!vote 2", ("test", 1002));
        env.step();

        env.push("!vote 1");
        env.step();

        env.push_owner("!vote 1");
        env.step();

        env.drain();

        ::std::thread::sleep(::std::time::Duration::from_secs(1));
        env.tick();

        assert_eq!(env.pop(), Some("(4 votes) #1 option a".into()));
        assert_eq!(env.pop(), Some("(1 votes) #2 option b".into()));
    }
}
