use crate::prelude::*;

use std::str;
use std::time::{Duration, Instant};

use hashbrown::HashSet;
use log::*;

pub const NAME: &str = "TwitchPoll";

submit! {
    template::Response("twitchpoll_already_running", "poll is already running. use `!poll stop` to stop it");
    template::Response("twitchpoll_is_poll_correct", "is this poll correct?");
    template::Response("twitchpoll_option_map", "#${index}: ${option}");
    template::Response("twitchpoll_not_configured", "no poll has been configured. use !poll title | options | ...");
    template::Response("twitchpoll_unknown_duration", "I don't know how long that is");
    template::Response("twitchpoll_start", "starting the poll for the next ${dur} seconds. use '!vote n' to vote for that option");
    template::Response("twitchpoll_poll_not_running", "no poll is running");
    template::Response("twitchpoll_unknown_option", "what option is that?");
    template::Response("twitchpoll_result", "(${count} votes) #${pos} ${option}");
    template::Response("twitchpoll_parse_error_title", "no title was provided. use !poll title | options | ...");
    template::Response("twitchpoll_parse_error_options", "no options were provided. use !poll title | options | ...");
}

pub struct TwitchPoll {
    poll: Option<Poll>,
    start: Option<Instant>,
    duration: usize,
    running: bool,
    map: CommandMap<TwitchPoll>,
}

impl Module for TwitchPoll {
    fn command(&mut self, req: &Request) -> Option<Response> {
        let map = self.map.clone();
        map.dispatch(self, req)
    }

    fn tick(&mut self, dt: Instant) -> Option<Response> {
        self.handle_tick(dt)
    }
}

impl TwitchPoll {
    pub fn create() -> Result<Self, ModuleError> {
        let map = CommandMap::create(
            NAME,
            &[
                ("!poll", Self::poll_command),
                ("!poll start", Self::poll_start_command),
                ("!poll stop", Self::poll_stop_command),
                ("!vote", Self::poll_vote_command),
            ],
        )?;

        Ok(Self {
            poll: None,
            start: None,
            duration: 0,
            running: false,
            map,
        })
    }

    fn poll_command(&mut self, req: &Request) -> Option<Response> {
        require_broadcaster!(&req);

        let poll = match Self::parse_poll(req.target(), req.args()) {
            Ok(poll) => poll,
            Err(ParseError::Title) => return reply_template!("twitchpoll_parse_error_title"),
            Err(ParseError::Options) => return reply_template!("twitchpoll_parse_error_options"),
        };

        if self.running {
            return reply_template!("twitchpoll_already_running");
        }

        let pos = template::finder();
        let pos = pos.get("twitchpoll_option_map").unwrap();

        // ask for verification that the poll is right
        let res = multi!(
            say_template!("twitchpoll_is_poll_correct"),
            say!(&poll.title),
            multi(poll.choices.iter().enumerate().map(|(i, s)| {
                say!(pos
                    .apply(&[("index", &(i + 1)), ("option", &s.option)])
                    .unwrap())
            }))
        );

        std::mem::replace(&mut self.poll, Some(poll));
        res
    }

    fn poll_start_command(&mut self, req: &Request) -> Option<Response> {
        require_broadcaster!(&req);

        if self.poll.is_none() {
            warn!("no poll");
            return reply_template!("twitchpoll_not_configured");
        }

        let dur = match req.args_iter().next().and_then(|a| {
            a.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse::<usize>()
                .ok()
        }) {
            Some(n) => n,
            None => return reply_template!("twitchpoll_unknown_duration"),
        };

        self.running = true;
        self.duration = dur;
        std::mem::replace(&mut self.start, Some(Instant::now()));
        say_template!("twitchpoll_start", ("dur", &dur))
    }

    fn poll_stop_command(&mut self, req: &Request) -> Option<Response> {
        require_broadcaster!(&req);

        if !self.running {
            return reply_template!("twitchpoll_poll_not_running");
        }

        info!("stopping poll");
        self.running = false;
        self.duration = 0;
        self.start.take();
        self.poll.take();

        None
    }

    fn poll_vote_command(&mut self, req: &Request) -> Option<Response> {
        if !self.running {
            debug!("poll not running");
            return None;
        }

        if self.poll.is_none() {
            warn!("tried to vote on an inactive poll. this shouldn't be reachable");
            return None;
        }

        let poll = self.poll.as_mut().unwrap();
        let max = poll.choices.len();

        let n = match req.args_iter().next().and_then(|a| {
            a.chars()
                .skip_while(|&c| c == '#')
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse::<usize>()
                .ok()
        }) {
            Some(n) if n == 0 || n > max => {
                return reply_template!("twitchpoll_unknown_option");
            }
            None => {
                return reply_template!("twitchpoll_unknown_option");
            }
            Some(n) => n,
        };

        trace!("attempting to vote for {}", n);
        poll.vote(req.sender(), n - 1);

        None
    }

    fn handle_tick(&mut self, _dt: Instant) -> Option<Response> {
        if !self.running || self.start.is_none() {
            return None;
        }

        let dt = Instant::now(); // don't trust the delta

        let deadline = Duration::from_secs(self.duration as u64);
        if let Some(start) = self.start {
            warn!("{:?} - {:?} < {:?}", dt, start, deadline);
            if dt - start < deadline {
                return None;
            }
        }

        info!("tallying the poll");
        self.running = false;

        let mut poll = {
            self.start.take();
            self.poll.take().expect("poll should have been running")
        };

        let rf = template::finder();
        let res = rf.get("twitchpoll_result").unwrap();

        let target = poll.target.clone(); // this is dumb
        let res = poll.tally().iter().take(3).map(|opt| {
            privmsg!(
                &target,
                res.apply(&[
                    ("count", &opt.count),   //
                    ("pos", &(opt.pos + 1)), //
                    ("option", &opt.option), //
                ])
                .unwrap()
            )
        });

        multi(res)
    }

    fn parse_poll(target: &str, data: &str) -> Result<Poll, ParseError> {
        let mut iter = data.split('|').map(str::trim).filter(|s| !s.is_empty());
        let title = iter.next().ok_or_else(|| ParseError::Title)?;
        Poll::new(target, title, iter)
    }
}

#[derive(Debug, PartialEq)]
enum ParseError {
    Title,
    Options,
}

#[derive(Debug, Clone)]
struct Choice {
    pos: usize,
    count: usize,
    option: String,
}

#[derive(Debug)]
struct Poll {
    target: String,
    title: String,
    choices: Vec<Choice>,
    seen: HashSet<i64>,
}

impl Poll {
    pub fn new<S, I>(target: S, title: S, choices: I) -> Result<Self, ParseError>
    where
        S: ToString,
        I: IntoIterator,
        I::Item: ToString,
    {
        let choices = choices
            .into_iter()
            .enumerate()
            .map(|(i, f)| Choice {
                option: f.to_string(),
                pos: i,
                count: 0,
            })
            .collect::<Vec<_>>();

        if choices.is_empty() {
            return Err(ParseError::Options);
        }

        Ok(Self {
            target: target.to_string(),
            title: title.to_string(),
            choices,
            seen: HashSet::new(),
        })
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

    pub fn tally(&mut self) -> &[Choice] {
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
        )
        .unwrap();

        assert_eq!(poll.title, "this is a test");

        let expected = &["option a", "option b", "option c"];
        poll.choices
            .iter()
            .enumerate()
            .for_each(|(i, c)| assert_eq!(expected[i], c.option));
    }

    #[test]
    fn poll_command() {
        let db = database::get_connection();
        let mut poll = TwitchPoll::create().unwrap();
        let mut env = Environment::new(&db, &mut poll);

        env.push("!poll");
        env.step_wait(false);
        assert_eq!(env.pop(), None);

        env.push_broadcaster("!poll");
        env.step();
        assert_eq!(
            env.pop().unwrap(),
            "@test: no title was provided. use !poll title | options | ..."
        );

        env.push_broadcaster("!poll test poll");
        env.step();
        assert_eq!(
            env.pop().unwrap(),
            "@test: no options were provided. use !poll title | options | ..."
        );

        env.push_broadcaster("!poll test poll | option a | option b");
        env.step();
        assert_eq!(env.pop().unwrap(), "is this poll correct?");
        assert_eq!(env.pop().unwrap(), "test poll");
        assert_eq!(env.pop().unwrap(), "#1: option a");
        assert_eq!(env.pop().unwrap(), "#2: option b");
    }

    #[test]
    fn poll_start_command() {
        let db = database::get_connection();
        let mut poll = TwitchPoll::create().unwrap();
        let mut env = Environment::new(&db, &mut poll);

        env.push("!poll start");
        env.step_wait(false);
        assert_eq!(env.pop(), None);

        env.push_broadcaster("!poll start");
        env.step();
        assert_eq!(
            env.pop().unwrap(),
            "@test: no poll has been configured. use !poll title | options | ..."
        );

        env.push_broadcaster("!poll test poll | option a | option b");
        env.step();
        env.drain();

        env.push_broadcaster("!poll start");
        env.step();
        assert_eq!(env.pop().unwrap(), "@test: I don't know how long that is");

        env.push_broadcaster("!poll start 160");
        env.step();
        assert_eq!(
            env.pop().unwrap(),
            "starting the poll for the next 160 seconds. use '!vote n' to vote for that option"
        );
    }

    #[test]
    fn poll_stop_command() {
        let db = database::get_connection();
        let mut poll = TwitchPoll::create().unwrap();
        {
            let mut env = Environment::new(&db, &mut poll);

            env.push("!poll stop");
            env.step_wait(false);
            assert_eq!(env.pop(), None);

            env.push_broadcaster("!poll stop");
            env.step();
            assert_eq!(env.pop().unwrap(), "@test: no poll is running");

            env.push_broadcaster("!poll test poll | option a | option b");
            env.step();
            env.drain();

            env.push_broadcaster("!poll start 160");
            env.step();
            env.drain();

            env.push_broadcaster("!poll stop");
            env.step_wait(false);

            env.push_broadcaster("!poll stop");
            env.step();
            assert_eq!(env.pop().unwrap(), "@test: no poll is running");
        }

        assert!(poll.poll.is_none());
        assert!(poll.start.is_none());
        assert_eq!(poll.duration, 0);
        assert_eq!(poll.running, false);
    }

    #[test]
    fn poll_vote_command() {
        let db = database::get_connection();
        let mut poll = TwitchPoll::create().unwrap();
        let mut env = Environment::new(&db, &mut poll);

        env.push("!poll vote");
        env.step_wait(false);
        assert_eq!(env.pop(), None);

        env.push_broadcaster("!poll test poll | option a | option b");
        env.step_wait(false);

        env.push_broadcaster("!poll start 1");
        env.step_wait(false);

        env.push_user("!vote 1", ("test", 1001));
        env.step_wait(false);

        env.push_user("!vote 2", ("test", 1002));
        env.step_wait(false);

        env.push_user("!vote 3", ("test", 1003));
        env.step_wait(false);

        env.push_user("!vote 1", ("test", 1003));
        env.step_wait(false);

        env.push_user("!vote 2", ("test", 1002));
        env.step_wait(false);

        env.push("!vote 1");
        env.step_wait(false);

        env.push_broadcaster("!vote 1");
        env.step_wait(false);

        env.drain();

        // TODO don't do this
        std::thread::sleep(std::time::Duration::from_secs(1));
        env.tick();

        assert_eq!(env.pop().unwrap(), "(3 votes) #1 option a");
        assert_eq!(env.pop().unwrap(), "(1 votes) #2 option b");
    }
}
