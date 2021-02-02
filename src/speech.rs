use std::collections::VecDeque;
use pulldown_cmark::{Event, CowStr};
use crate::{find_one_of, find_puncts_end};
use crate::parser::split_speech_heading;

#[derive(Debug,Clone,PartialEq)]
pub struct Speech<'a> {
    pub heading: Heading<'a>,
    pub body: Vec<Inline<'a>>,
}

#[derive(Debug,Clone,PartialEq)]
pub struct Heading<'a> {
    pub character: CowStr<'a>,
    pub direction: Direction<'a>,
}

#[derive(Debug,Clone,PartialEq)]
pub enum Inline<'a> {
    Event(Event<'a>),
    Direction(Direction<'a>),
}

#[derive(Debug,Clone,PartialEq)]
pub struct Direction<'a>(pub Vec<Event<'a>>);

impl<'a> Direction<'a> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push_string(&mut self, s: String) {
        self.0.push(Event::Text(s.into()));
    }
}

pub fn parse_speech<'a>(events: Vec<Event<'a>>) -> Option<Speech<'a>> {
    let mut iter = events.into_iter();

    let first = iter.next();

    let (heading, first) = match first {
        Some(Event::Text(s)) => {
            let s = s.to_string();
            if let Some((heading, line)) = split_speech_heading(s.as_ref()) {
                let heading = heading.to_owned();
                let line = line.to_owned();
                (parse_heading(&heading), Event::Text(line.into()))
            } else {
                return None;
            }
        },
        _ => return None,
    };

    let mut events = vec![first];
    iter.for_each(|e| { events.push(e); });

    let body = parse_body(events);

    Some(Speech {
        heading: heading,
        body: body,
    })
}

pub fn parse_heading(s: &str) -> Heading<'static> {
    let open_paren = match s.find('(') {
        Some(pos) => pos,
        None => {
            let character = s.trim().to_owned();
            return Heading {
                character: character.into(),
                direction: Direction::new(),
            };
        },
    };

    let character = s[..open_paren].trim().to_owned();
    let s = &s[open_paren+1..];
    let mut close_paren = s.len();
    for (index, c) in s.char_indices() {
        if c == ')' {
            close_paren = index;
            break;
        }
    }

    let s = s[..close_paren].trim().to_owned();
    let mut direction = Direction::new();
    direction.push_string(s);

    Heading {
        character: character.into(),
        direction: direction,
    }
}

pub fn parse_body<'a>(events: Vec<Event<'a>>) -> Vec<Inline<'a>> {
    let mut body = Vec::new();
    let mut direction = Vec::new();
    let mut paren_level = 0usize;

    for event in ParenSplitter::new(events.into_iter()) {
        match event {
            Event::Text(s) if s.as_ref() == "(" => {
                if paren_level > 0 {
                    direction.push(Event::Text(s));
                }

                paren_level = paren_level + 1;
            },
            Event::Text(s) if s.as_ref() == ")" => {
                match paren_level {
                    0 => {
                        body.push(Inline::Event(Event::Text(s)));
                    },
                    1 => {
                        let mut pushed = Vec::new();
                        std::mem::swap(&mut pushed, &mut direction);
                        let pushed = Direction(pushed);
                        body.push(Inline::Direction(pushed));
                        paren_level = paren_level - 1;
                    },
                    _ => {
                        direction.push(Event::Text(s));
                        paren_level = paren_level -1;
                    },
                }
            },
            _ => {
                if paren_level > 0 {
                    direction.push(event);
                } else {
                    body.push(Inline::Event(event));
                }
            },
        }
    }

    if direction.len() > 0 {
        let direction = Direction(direction);
        body.push(Inline::Direction(direction));
    }

    trim_start_of_line_head(body)
}

#[derive(Debug)]
pub struct ParenSplitter<'a, I> {
    iter: I,
    queue: VecDeque<Event<'a>>,
}

impl<'a, I> ParenSplitter<'a, I>
where
    I: Iterator<Item=Event<'a>>,
{
    pub fn new(iter: I) -> Self {
        Self {
            iter: iter,
            queue: VecDeque::new(),
        }
    }
}

impl<'a, I> Iterator for ParenSplitter<'a, I>
where
    I: Iterator<Item=Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(event) = self.queue.pop_front() {
            return Some(event);
        }

        match self.iter.next() {
            Some(Event::Text(s)) => {
                for text in split_at_paren(s).into_iter() {
                    self.queue.push_back(Event::Text(text.into()));
                }
            },
            item => return item,
        }

        self.queue.pop_front()
    }
}

fn split_at_paren<T: AsRef<str>>(s: T) -> Vec<String> {
    let mut s = s.as_ref();
    let mut v = Vec::new();

    loop {
        if s.len() == 0 {
            break;
        }

        match find_one_of(s, "()") {
            Some((index, c)) => {
                let before = &s[..index];
                let (parens, after) = find_puncts_end(&s[index..], c);
                v.push(before.to_owned());
                v.push(parens.to_owned());
                s = after;
            },
            None => {
                v.push(s.to_owned());
                s = "";
            },
        }
    }

    v
}

pub fn trim_start_of_line_head<'a>(body: Vec<Inline<'a>>) -> Vec<Inline<'a>> {
    let mut ret = Vec::with_capacity(body.len());
    let mut is_line_head = true;

    for inline in body.into_iter() {
        match (inline, is_line_head) {
            (Inline::Event(Event::Text(s)), true) => {
                let trimmed = s.trim_start();
                if trimmed.len() > 0 {
                    let trimmed = trimmed.to_owned();
                    ret.push(Inline::Event(Event::Text(trimmed.into())));
                }
                is_line_head = false;
            },
            (inline @ Inline::Event(Event::SoftBreak), _) => {
                ret.push(inline);
                is_line_head = true;
            },
            (inline, _) => {
                ret.push(inline);
                is_line_head = false;
            },
        }
    }

    ret
}

#[cfg(test)]
mod test {
    use super::*;
    use pulldown_cmark::Event;
    use big_s::S;

    #[test]
    fn parse_heading_only_with_character() {
        assert_eq!(parse_heading("A  "), Heading {
            character: "A".into(),
            direction: Direction::new(),
        });
    }

    #[test]
    fn parse_heading_with_direction() {
        assert_eq!(parse_heading("A (running) "), Heading {
            character: "A".into(),
            direction: Direction(vec![Event::Text("running".into())]),
        });
    }

    #[test]
    fn split_parens_in_direction() {
        assert_eq!(split_at_paren("A (running)"), vec![S("A "), S("("), S("running"), S(")")]);
        assert_eq!(split_at_paren("xx (dd) yy"), vec![S("xx "), S("("), S("dd"), S(")"), S(" yy")]);
        assert_eq!(split_at_paren("Escaped (( example"), vec![S("Escaped "), S("(("), S(" example")]);
    }

    #[test]
    fn paren_splitter_for_two_lines() {
        let v = vec![Event::Text("Hello! (xxx)".into()), Event::SoftBreak, Event::Text("Bye!".into())];
        let mut iter = ParenSplitter::new(v.into_iter());

        assert_eq!(iter.next(), Some(Event::Text("Hello! ".into())));
        assert_eq!(iter.next(), Some(Event::Text("(".into())));
        assert_eq!(iter.next(), Some(Event::Text("xxx".into())));
        assert_eq!(iter.next(), Some(Event::Text(")".into())));
        assert_eq!(iter.next(), Some(Event::SoftBreak));
        assert_eq!(iter.next(), Some(Event::Text("Bye!".into())));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn parse_body_only_with_text() {
        let v = vec![Event::Text("Hello!".into()), Event::SoftBreak];
        assert_eq!(parse_body(v), vec![Inline::Event(Event::Text("Hello!".into())), Inline::Event(Event::SoftBreak)]);
    }

    #[test]
    fn parse_body_with_direction() {
        let input = vec![
            Event::Text("Hello! (running) Bye!".into()),
        ];
        let output = vec![
            Inline::Event(Event::Text("Hello! ".into())),
            Inline::Direction(Direction(
                    vec![Event::Text("running".into())]
            )),
            Inline::Event(Event::Text(" Bye!".into())),
        ];
        assert_eq!(parse_body(input), output);
    }

    #[test]
    fn parse_body_with_nested_parens() {
        let input = vec![
            Event::Text("Hello! (running (xxx) ) Bye!".into()),
        ];
        let output = vec![
            Inline::Event(Event::Text("Hello! ".into())),
            Inline::Direction(Direction(vec![
                    Event::Text("running ".into()),
                    Event::Text("(".into()),
                    Event::Text("xxx".into()),
                    Event::Text(")".into()),
                    Event::Text(" ".into()),
            ])),
            Inline::Event(Event::Text(" Bye!".into())),
        ];
        assert_eq!(parse_body(input), output);
    }

    #[test]
    fn parse_speech_of_one_line() {
        let input = vec![
            Event::Text("A (running)> Hello! (exit)".into()),
        ];
        let output = Speech {
            heading: Heading {
                character: "A".into(),
                direction: Direction(vec![Event::Text("running".into())]),
            },
            body: vec![
                Inline::Event(Event::Text("Hello! ".into())),
                Inline::Direction(Direction(vec![
                        Event::Text("exit".into()),
                ])),
            ],
        };
        assert_eq!(parse_speech(input), Some(output));
    }

    #[test]
    fn trim_start_of_body_line_head() {
        let input = vec![
            Inline::Event(Event::Text(" Hello!".into())),
            Inline::Event(Event::SoftBreak),
            Inline::Event(Event::Text("   Ah!".into())),
            Inline::Event(Event::SoftBreak),
            Inline::Event(Event::Text(" Oh!".into())),
            Inline::Direction(Direction(vec![Event::Text("exit".into())])),
            Inline::Event(Event::Text(" zzz".into())),
        ];
        let output = vec![
            Inline::Event(Event::Text("Hello!".into())),
            Inline::Event(Event::SoftBreak),
            Inline::Event(Event::Text("Ah!".into())),
            Inline::Event(Event::SoftBreak),
            Inline::Event(Event::Text("Oh!".into())),
            Inline::Direction(Direction(vec![Event::Text("exit".into())])),
            Inline::Event(Event::Text(" zzz".into())),
        ];
        assert_eq!(trim_start_of_line_head(input), output);
    }
}
