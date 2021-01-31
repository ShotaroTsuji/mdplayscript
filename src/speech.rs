use std::collections::VecDeque;
use pulldown_cmark::{Event, CowStr};
use crate::{find_one_of, find_puncts_end};

#[derive(Debug,Clone,PartialEq)]
pub struct Speech<'a> {
    heading: Heading<'a>,
    body: Vec<Inline<'a>>,
}

#[derive(Debug,Clone,PartialEq)]
pub struct Heading<'a> {
    character: CowStr<'a>,
    direction: Direction<'a>,
}

#[derive(Debug,Clone,PartialEq)]
pub struct Direction<'a>(Vec<Event<'a>>);

impl<'a> Direction<'a> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push_text(&mut self, s: &'a str) {
        self.0.push(Event::Text(s.into()));
    }
}

#[derive(Debug,Clone,PartialEq)]
pub enum Inline<'a> {
    Event(Event<'a>),
    Direction(Direction<'a>),
}

fn parse_heading<'a>(s: &'a str) -> Heading<'a> {
    let open_paren = match s.find('(') {
        Some(pos) => pos,
        None => {
            let character = s.trim();
            return Heading {
                character: character.into(),
                direction: Direction::new(),
            };
        },
    };

    let character = s[..open_paren].trim();
    let s = &s[open_paren+1..];
    let mut close_paren = s.len();
    for (index, c) in s.char_indices() {
        if c == ')' {
            close_paren = index;
            break;
        }
    }

    let s = s[..close_paren].trim();
    let mut direction = Direction::new();
    direction.push_text(s);

    Heading {
        character: character.into(),
        direction: direction,
    }
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
                let s = s.into_string();
                for text in split_at_paren(s).into_iter() {
                    self.queue.push_back(Event::Text(text.into()));
                }
            },
            item => return item,
        }

        self.queue.pop_front()
    }
}

fn split_at_paren(s: String) -> Vec<String> {
    let mut s = s.as_str();
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

#[cfg(test)]
mod test {
    use super::*;
    use pulldown_cmark::Event;

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

    /*
    #[test]
    fn split_parens_in_direction() {
        assert_eq!(split_at_paren("A (running)"), vec!["A ", "(", "running", ")"]);
        assert_eq!(split_at_paren("xx (dd) yy"), vec!["xx ", "(", "dd", ")", " yy"]);
        assert_eq!(split_at_paren("Escaped (( example"), vec!["Escaped ", "((", " example"]);
    }
    */

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
}
