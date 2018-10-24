use crate::color::RGB;

use std::{collections::HashMap, ops::Range};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Tags(HashMap<String, String>);

impl Tags {
    pub fn new(input: &str) -> Self {
        let mut map = HashMap::new();
        for part in input.split_terminator(';') {
            if let Some(index) = part.find('=') {
                let (k, v) = (&part[..index], &part[index + 1..]);
                map.insert(k.into(), v.into());
            }
        }
        Self { 0: map }
    }

    pub fn get_kappas(&self) -> Option<Vec<Kappa>> {
        self.get("emotes")
            .and_then(|e| {
                if !e.is_empty() {
                    Some(Kappa::parse(e))
                } else {
                    None
                }
            })
            .or_else(|| {
                debug!("no emotes attached to that message");
                None
            })
    }

    pub fn get_color(&self) -> Option<RGB> {
        self.get("color")
            .and_then(|s| Some(RGB::from(s)))
            .or_else(|| Some(RGB::from((255, 255, 255))))
    }

    pub fn get_display(&self) -> Option<&str> {
        self.get("display-name").or_else(|| {
            debug!("cannot get display-name for {:?}", self);
            None
        })
    }

    pub fn get_userid(&self) -> Option<i64> {
        self.get("user-id")?.parse::<i64>().ok().or_else(|| {
            debug!("cannot get user-id for {:?}", self);
            None
        })
    }

    pub fn get<S>(&self, s: S) -> Option<&str>
    where
        S: AsRef<str>,
    {
        let s = s.as_ref();
        self.0.get(s).map(|n| n.as_ref())
    }
}

#[derive(PartialEq, Debug, Clone, Serialize)]
pub struct Kappa {
    pub ranges: Vec<Range<u16>>,
    pub id: usize,
}

impl Kappa {
    pub fn parse(s: &str) -> Vec<Self> {
        // could count the commas to pre-size the vector
        let mut kappas = vec![];
        fn get_ranges(tail: &str) -> Option<Vec<Range<u16>>> {
            let mut vec = vec![];
            for s in tail.split_terminator(',') {
                let (start, end) = {
                    let mut s = s.split_terminator('-');
                    (s.next()?, s.next()?)
                };
                vec.push(Range {
                    start: start.parse::<u16>().ok()?,
                    end: end.parse::<u16>().ok()?,
                });
            }
            Some(vec)
        }

        fn get_parts(emote: &str) -> Option<(&str, &str)> {
            let mut s = emote.split_terminator(':');
            Some((s.next()?, s.next()?))
        }

        for emote in s.split_terminator('/') {
            if let Some((head, tail)) = get_parts(emote) {
                if let Some(ranges) = get_ranges(&tail) {
                    kappas.push(Kappa {
                        id: head.parse::<usize>().expect("twitch to be not-wrong"),
                        ranges,
                    });
                }
            }
        }

        kappas
    }
}

#[cfg(test)]
mod kappa_test {
    use super::*;

    #[test]
    fn make_kappas() {
        let inputs = &[
            (
                "25:0-4,6-10,12-16",
                vec![Kappa {
                    id: 25,
                    ranges: vec![
                        Range { start: 0, end: 4 },
                        Range { start: 6, end: 10 },
                        Range { start: 12, end: 16 },
                    ],
                }],
            ),
            (
                "25:0-4",
                vec![Kappa {
                    id: 25,
                    ranges: vec![Range { start: 0, end: 4 }],
                }],
            ),
            (
                "1077966:0-6/25:8-12",
                vec![
                    Kappa {
                        id: 107_7966,
                        ranges: vec![Range { start: 0, end: 6 }],
                    },
                    Kappa {
                        id: 25,
                        ranges: vec![Range { start: 8, end: 12 }],
                    },
                ],
            ),
            (
                "25:0-4,6-10/33:12-19",
                vec![
                    Kappa {
                        id: 25,
                        ranges: vec![Range { start: 0, end: 4 }, Range { start: 6, end: 10 }],
                    },
                    Kappa {
                        id: 33,
                        ranges: vec![Range { start: 12, end: 19 }],
                    },
                ],
            ),
            (
                "25:0-4,15-19/33:6-13",
                vec![
                    Kappa {
                        id: 25,
                        ranges: vec![Range { start: 0, end: 4 }, Range { start: 15, end: 19 }],
                    },
                    Kappa {
                        id: 33,
                        ranges: vec![Range { start: 6, end: 13 }],
                    },
                ],
            ),
            (
                "33:0-7/25:9-13,15-19",
                vec![
                    Kappa {
                        id: 33,
                        ranges: vec![Range { start: 0, end: 7 }],
                    },
                    Kappa {
                        id: 25,
                        ranges: vec![Range { start: 9, end: 13 }, Range { start: 15, end: 19 }],
                    },
                ],
            ),
        ];

        for (input, expect) in inputs {
            let kappas = Kappa::new(&input);
            assert_eq!(kappas.len(), expect.len());
            assert_eq!(kappas, *expect);
        }
    }
}
