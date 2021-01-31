use pulldown_cmark::{Event, CowStr};

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
}
