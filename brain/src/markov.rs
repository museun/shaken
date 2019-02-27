use hashbrown::HashMap;
use rand::prelude::*;
use serde::{Deserialize, Serialize};

use std::cmp::{min, Ordering};
use std::ops::{Deref, DerefMut};

#[derive(Default, Serialize, Deserialize)]
pub struct Markov<'a> {
    #[serde(borrow)]
    chain: HashMap<Vec<&'a str>, LinkSet<'a>>,
    entries: Vec<&'a str>,
    depth: usize,
}

impl<'a> Markov<'a> {
    pub fn with_depth(depth: usize) -> Self {
        Markov {
            depth,
            ..Default::default()
        }
    }

    pub fn train_text(&mut self, input: &'a str) {
        input
            .split_terminator(|c| ".?!\n".contains(c))
            .map(str::trim)
            .map(|s| s.split_whitespace().collect::<Vec<_>>())
            .filter(|s| !s.is_empty())
            .for_each(|set| self.train_words(&set));
    }

    fn train_words(&mut self, words: &[&'a str]) {
        let depth = min(self.depth, words.len() - 1);
        if !self.entries.iter().any(|s| *s == words[0]) {
            let start = words[0].trim_start_matches(|c: char| !c.is_alphabetic());
            if start.is_empty() {
                return;
            }
            self.entries.push(start);
        }

        for width in 1..=depth {
            for window in words.windows(width + 1) {
                self.train_link(
                    &window[..window.len() - 1],
                    &Token::Word(window.last().expect("get last window")),
                );
            }

            self.train_link(&words[words.len() - width..], &Token::End);
        }
    }

    fn train_link(&mut self, context: &[&'a str], token: &Token<'a>) {
        let ctx = context.to_vec();
        if let Some(link_set) = self.chain.get_mut(&ctx) {
            link_set.insert(&token);
            return;
        }

        self.chain.entry(ctx).or_default().insert(&token);
    }

    pub fn generate(&self, rng: &mut impl Rng) -> String {
        let mut words: Vec<&'a str> = vec![];
        let start = self.entries.choose(rng).expect("push start seed");
        words.push(*start);

        fn context<'a, 'b>(words: &'a [&'b str], depth: usize) -> &'a [&'b str] {
            &words[words.len().saturating_sub(depth)..]
        };

        while let Token::Word(word) = self.next_word(rng, context(&words, self.depth)) {
            words.push(word);
        }

        words.join(" ")
    }

    fn next_word(&self, rng: &mut impl Rng, context: &[&'a str]) -> Token<'a> {
        let subcontext = |width| &context[context.len() - width..];
        let depth = min(self.depth, context.len());

        let link_sets = (1..=depth).filter_map(|width| {
            let s = subcontext(width).to_vec();
            self.chain.get(&s).map(|link_set| (width, link_set))
        });

        // why
        let mut pooled_links: Vec<Link<'a>> = {
            if let Some((_, link_set)) = link_sets.clone().next() {
                let num_links = link_set.len();
                Vec::with_capacity(num_links)
            } else {
                return Token::End;
            }
        };

        for (width, link_set) in link_sets {
            for mut link in link_set.iter().cloned() {
                link.count *= width;
                if let Some(existing) = pooled_links.iter_mut().find(|l| l.token == link.token) {
                    existing.merge(&link);
                } else {
                    pooled_links.push(link);
                }
            }
        }

        Self::weighted_selection(rng, &pooled_links).token
    }

    fn weighted_selection(rng: &mut impl Rng, links: &[Link<'a>]) -> Link<'a> {
        let total_count: usize = links.iter().map(|l| l.count).sum();
        links
            .iter()
            .cycle()
            .skip(rng.gen::<usize>() % total_count)
            .scan(total_count, |remaining, link| {
                *remaining = remaining.saturating_sub(link.count);
                Some((*remaining, link))
            })
            .filter(|(remaining, _)| *remaining == 0)
            .map(|(_, link)| link)
            .next()
            .expect("get next weighted")
            .clone()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd, Eq)]
pub enum Token<'a> {
    Word(&'a str),
    End,
}

#[derive(Serialize, Deserialize, Clone, PartialOrd, Eq)]
pub struct Link<'a> {
    #[serde(borrow)]
    pub token: Token<'a>,
    pub count: usize,
}

impl<'a> Link<'a> {
    pub fn from(token: Token<'a>) -> Self {
        Link { token, count: 1 }
    }

    pub fn merge(&mut self, rhs: &Self) {
        debug_assert!(rhs.token == self.token);
        self.count += rhs.count;
    }
}

impl<'a> PartialEq for Link<'a> {
    fn eq(&self, rhs: &Self) -> bool {
        self.count.eq(&rhs.count)
    }
}

impl<'a> Ord for Link<'a> {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.count.cmp(&rhs.count)
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct LinkSet<'a>(#[serde(borrow)] Vec<Link<'a>>);

impl<'a> LinkSet<'a> {
    pub fn insert(&mut self, token: &Token<'a>) {
        let link = Link::from(token.clone());

        if let Some(existing) = self.existing(&token) {
            existing.merge(&link);
            self.sort_unstable_by(|a, b| b.cmp(a)); // reverse
        } else {
            self.push(link);
        }
    }

    fn existing(&mut self, token: &Token<'a>) -> Option<&mut Link<'a>> {
        self.iter_mut().find(|l| l.token == *token)
    }
}

impl<'a> Deref for LinkSet<'a> {
    type Target = Vec<Link<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> DerefMut for LinkSet<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
