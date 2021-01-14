use std::marker::PhantomData;
use std::collections::VecDeque;
use pulldown_cmark::{Event, Tag};
use pulldown_cmark::escape::{StrWrite, escape_html, escape_href};
use pulldown_cmark::html::push_html;
use trim_in_place::TrimInPlace;

mod lookahead;
use crate::lookahead::Lookahead;

pub struct PlayMd<'a, P> {
    parser: P,
    is_in_paragraph: bool,
    previous: Option<Event<'a>>,
    _marker: PhantomData<&'a P>,
}

impl<'a, P> PlayMd<'a, P>
where
    P: Iterator<Item=Event<'a>>,
{
    pub fn new(parser: P) -> Self {
        PlayMd {
            parser: parser,
            is_in_paragraph: false,
            previous: None,
            _marker: PhantomData,
        }
    }
}

impl<'a, P> Iterator for PlayMd<'a, P>
where
    P: Iterator<Item=Event<'a>>,
{
    type Item=Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(event) = self.parser.next() {
            let ret = match event.clone() {
                Event::Start(Tag::Paragraph) => {
                    self.is_in_paragraph = true;

                    Event::Start(Tag::Paragraph)
                },
                Event::End(Tag::Paragraph) => {
                    self.is_in_paragraph = false;
                    eprint!("! ");

                    Event::End(Tag::Paragraph)
                },
                Event::Text(text) if self.is_in_paragraph => {
                    match split_name(&text) {
                        (Some(role), line) => {
                            eprintln!("  PREV: {:?}", self.previous);
                            eprintln!("  ROLE: {}", role);

                            let mut buf = String::new();

                            match self.previous {
                                Some(Event::Start(Tag::Paragraph)) => {},
                                Some(Event::SoftBreak) => {
                                    writeln!(buf, "</p>").unwrap();
                                    write!(buf, "<p>").unwrap();
                                },
                                _ => unreachable!(),
                            }

                            write!(buf, "<span class=\"role\">").unwrap();
                            escape_html(&mut buf, role).unwrap();
                            write!(buf, "</span>").unwrap();
                            escape_html(&mut buf, line).unwrap();

                            Event::Html(buf.into())
                        },
                        (None, line) => {
                            Event::Text(line.to_owned().into())
                        },
                    }
                },
                e => e,
            };

            if self.is_in_paragraph {
                eprint!("* ");
            }

            eprintln!("{:?}", event);

            self.previous.replace(event);

            Some(ret)
        } else {
            None
        }
    }
}

fn split_name(s: &str) -> (Option<&str>, &str) {
    match s.find(':') {
        Some(pos) => (Some(&s[..pos]), s[pos+1..].trim()),
        None => (None, s),
    }
}

fn distil<'a>(terms: Vec<LineTerm<'a>>) -> Vec<Event<'a>> {
    let mut events = Vec::new();

    let mut trim_start = false;

    for term in terms.into_iter() {
        match term {
            LineTerm::Role(role) => {
                let mut buf = "<span class=\"role\">".to_owned();
                escape_html(&mut buf, role.as_str());
                buf += "</span>";
                trim_start = true;

                events.push(Event::Html(buf.into()));
            },
            LineTerm::Text(mut text) => {
                if trim_start {
                    TrimInPlace::trim_start_in_place(&mut text);
                }

                if text.len() > 0 {
                    events.push(Event::Text(text.into()));
                }
            },
            LineTerm::DirectionStart => {
                match events.pop() {
                    Some(Event::Text(text)) => {
                        let mut text = text.into_string();
                        TrimInPlace::trim_end_in_place(&mut text);
                        events.push(Event::Text(text.into()));
                    },
                    Some(e) => {
                        events.push(e);
                    },
                    None => {},
                }

                trim_start = true;
                events.push(Event::Html("<span class=\"direction\">".into()));
            },
            LineTerm::DirectionEnd => {
                match events.pop() {
                    Some(Event::Text(text)) => {
                        let mut text = text.into_string();
                        TrimInPlace::trim_end_in_place(&mut text);
                        events.push(Event::Text(text.into()));
                    },
                    Some(e) => {
                        events.push(e);
                    },
                    None => {},
                }

                trim_start = true;
                events.push(Event::Html("</span>".into()));
            },
            LineTerm::Event(e) => {
                trim_start = false;
                events.push(e);
            },
        }
    }

    events
}

#[derive(Debug,Clone,PartialEq)]
enum LineTerm<'a> {
    DirectionStart,
    DirectionEnd,
    Role(String),
    Text(String),
    Event(Event<'a>),
}

fn tokens_to_terms<'a>(tokens: Vec<Token<'a>>, escape: bool) -> impl Iterator<Item=LineTerm<'a>> {
    tokens.into_iter()
        .map(move |t| match t {
            Token::Event(e) => LineTerm::Event(e),
            Token::Text(tt) => LineTerm::Text(tt.into_string(escape)),
        })
}

fn parse_play_line<'a>(line: Vec<Token<'a>>) -> Vec<LineTerm<'a>> {
    let mut terms = Vec::new();

    if line.len() <= 1 {
        return tokens_to_terms(line, false).collect();
    }

    match (&line[0], &line[1]) {
        (Token::Text(TextToken::Text(name)), Token::Text(TextToken::Rangle)) => {
            terms.push(LineTerm::Role(name.clone()));
        },
        _ => {
            return tokens_to_terms(line, false).collect();
        },
    }

    let mut i = 2;
    while i < line.len() {
        match &line[i] {
            Token::Event(e) => {
                terms.push(LineTerm::Event(e.clone()));
            },
            Token::Text(TextToken::Left) => {
                let mut j = i;
                let mut right_pos: Option<usize> = None;
                while j < line.len() {
                    match &line[j] {
                        Token::Text(TextToken::Right) => {
                            right_pos.replace(j);
                            break;
                        },
                        _ => {},
                    }
                    j = j + 1;
                }

                if let Some(right_pos) = right_pos {
                    terms.push(LineTerm::DirectionStart);

                    tokens_to_terms(line[i+1..j].to_vec(), true)
                        .for_each(|t| { terms.push(t); });

                    terms.push(LineTerm::DirectionEnd);

                    i = right_pos + 1;
                    continue;
                } else {
                    terms.push(LineTerm::Text("(".to_owned()));
                }
            },
            Token::Text(t) => {
                terms.push(LineTerm::Text(t.clone().into_string(true)));
            },
        }

        i = i + 1;
    }

    terms
}


struct PlayLines<'a, I>
where
    I: Iterator<Item=Token<'a>>,
{
    iter: Lookahead<I>,
}

impl<'a, I> PlayLines<'a, I>
where
    I: Iterator<Item=Token<'a>>,
{
    fn new(iter: I) -> Self {
        Self {
            iter: Lookahead::new(iter, 2),
        }
    }

    fn into_inner(self) -> I {
        self.iter.into_inner()
    }
}

impl<'a, I> Iterator for PlayLines<'a, I>
where
    I: Iterator<Item=Token<'a>>,
{
    type Item = Vec<Token<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = Vec::new();

        loop {
            let token = self.iter.next();
            let breakable = match (token.as_ref(), self.iter.ahead(0), self.iter.ahead(1)) {
                (Some(&Token::Event(Event::SoftBreak)), Some(&Token::Text(TextToken::Text(_))), Some(&Token::Text(TextToken::Rangle))) => true,
                (_, Some(&Token::Event(Event::End(Tag::Paragraph))), _) => true,
                (None, _, _) => true,
                _ => false,
            };

            if let Some(token) = token {
                line.push(token);
            }

            if breakable {
                break;
            }
        }

        if line.len() == 0 {
            None
        } else {
            Some(line)
        }
    }
}

#[derive(Debug,Clone,PartialEq)]
enum Token<'a> {
    Text(TextToken),
    Event(Event<'a>),
}

#[derive(Debug,Clone,PartialEq)]
enum TextToken {
    Text(String),
    Rangle,
    RangleBlock(usize),
    Left,
    LeftBlock(usize),
    Right,
    RightBlock(usize),
}

fn repeat_char(c: char, n: usize) -> String {
    let mut s = String::new();

    for _ in 0..n {
        s.push(c);
    }

    s
}

impl TextToken {
    fn into_string(self, escaped: bool) -> String {
        use TextToken::*;
        match (self, escaped) {
            (Rangle, true) | (Left, true) | (Right, true) => "".to_owned(),
            (Rangle, false) => ">".to_owned(),
            (Left,   false) => "(".to_owned(),
            (Right,  false) => ")".to_owned(),
            (RangleBlock(n), true)  => repeat_char('>', n-1),
            (RangleBlock(n), false) => repeat_char('>', n),
            (LeftBlock(n), true)  => repeat_char('(', n-1),
            (LeftBlock(n), false) => repeat_char('(', n),
            (RightBlock(n), true)  => repeat_char(')', n-1),
            (RightBlock(n), false) => repeat_char(')', n),
            (Text(s), _) => s,
        }
    }
}

struct EventTokener<'a, P> {
    parser: P,
    queue: VecDeque<Token<'a>>,
    nest_level: usize,
    _phantom: PhantomData<&'a P>,
}

impl<'a, P> EventTokener<'a, P>
where
    P: Iterator<Item=Event<'a>>,
{
    fn new(parser: P) -> EventTokener<'a, P> {
        Self {
            parser: parser,
            queue: VecDeque::new(),
            nest_level: 0,
            _phantom: PhantomData,
        }
    }

    fn into_inner(self) -> P {
        self.parser
    }
}

impl<'a, P> Iterator for EventTokener<'a, P>
where
    P: Iterator<Item=Event<'a>>,
{
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(token) = self.queue.pop_front() {
            Some(token)
        } else if let Some(event) = self.parser.next() {
            match (event, self.nest_level) {
                (Event::Start(tag), l) => {
                    self.nest_level = l + 1;
                    Some(Token::Event(Event::Start(tag)))
                },
                (Event::End(tag), l) => {
                    if l > 0 {
                        self.nest_level = l - 1;
                    }
                    Some(Token::Event(Event::End(tag)))
                },
                (Event::Text(text), 0) => {
                    for t in TextTokener::new(&text) {
                        self.queue.push_back(Token::Text(t));
                    }

                    self.queue.pop_front()
                },
                (e, _) => Some(Token::Event(e)),
            }
        } else {
            None
        }
    }
}

struct TextTokener<'a> {
    s: &'a str,
}

impl<'a> TextTokener<'a> {
    fn new(s: &'a str) -> Self {
        TextTokener {
            s: s,
        }
    }
}

impl<'a> Iterator for TextTokener<'a> {
    type Item = TextToken;

    fn next(&mut self) -> Option<Self::Item> {
        if self.s.len() == 0 {
            None
        } else {
            match find_one_of(self.s, ">()") {
                Some((index, c)) if index == 0 => {
                    let (puncts, text) = find_puncts_end(self.s, c);
                    self.s = text;
                    match (c, puncts.len()) {
                        ('>', 1) => Some(TextToken::Rangle),
                        ('>', n) => Some(TextToken::RangleBlock(n)),
                        ('(', 1) => Some(TextToken::Left),
                        ('(', n) => Some(TextToken::LeftBlock(n)),
                        (')', 1) => Some(TextToken::Right),
                        (')', n) => Some(TextToken::RightBlock(n)),
                        _ => unreachable!(),
                    }
                },
                Some((index, _)) => {
                    let ret = TextToken::Text(self.s[..index].to_owned());
                    self.s = &self.s[index..];
                    Some(ret)
                },
                None => {
                    let ret = TextToken::Text(self.s.to_owned());
                    self.s = "";
                    Some(ret)
                },
            }
        }
    }
}


fn find_one_of(s: &str, ps: &str) -> Option<(usize, char)> {
    for (index, c) in s.char_indices() {
        if ps.contains(c) {
            return Some((index, c));
        }
    }

    None
}


#[derive(Debug,Clone,PartialEq)]
enum FindPuncts<'a> {
    Found(&'a str, &'a str, &'a str),
    NotFound(&'a str),
}

impl<'a> FindPuncts<'a> {
    fn position(&self) -> Option<usize> {
        match self {
            FindPuncts::Found(before, _, _) => Some(before.len()),
            FindPuncts::NotFound(_) => None,
        }
    }
}

fn find_puncts(s: &str, p: char) -> FindPuncts {
    match find_puncts_begin(s, p) {
        PunctsBegin::Found(before, after) => {
            let (puncts, after) = find_puncts_end(after, p);
            FindPuncts::Found(before, puncts, after)
        },
        PunctsBegin::NotFound(s) => FindPuncts::NotFound(s),
    }
}

#[derive(Debug,Clone,PartialEq)]
enum PunctsBegin<'a> {
    Found(&'a str, &'a str),
    NotFound(&'a str),
}

fn find_puncts_begin(s: &str, p: char) -> PunctsBegin {
    for (index, c) in s.char_indices() {
        if c == p {
            return PunctsBegin::Found(&s[..index], &s[index..]);
        }
    }

    PunctsBegin::NotFound(s)
}

fn find_puncts_end(s: &str, p: char) -> (&str, &str) {
    assert!(s.starts_with(p));

    for (index, c) in s.char_indices() {
        if c != p {
            return (&s[..index], &s[index..]);
        }
    }

    (s, "")
}

#[cfg(test)]
mod test {
    use pulldown_cmark::Parser;
    use super::*;

    #[test]
    fn puncts_begin() {
        let p = '>';
        let s = "AAA> BBB";
        assert_eq!(find_puncts_begin(s, p), PunctsBegin::Found("AAA", "> BBB"));
        let s = "xxx>>> xxx";
        assert_eq!(find_puncts_begin(s, p), PunctsBegin::Found("xxx", ">>> xxx"));
        let s = "Hello";
        assert_eq!(find_puncts_begin(s, p), PunctsBegin::NotFound(s));
        let s = "First> Second>>";
        assert_eq!(find_puncts_begin(s, p), PunctsBegin::Found("First", "> Second>>"));
    }

    #[test]
    fn puncts_end() {
        let p = '>';
        let s = "> BBB";
        assert_eq!(find_puncts_end(s, p), (">", " BBB"));
        let s = ">>> xxx";
        assert_eq!(find_puncts_end(s, p), (">>>", " xxx"));
        let s = "> Second>>";
        assert_eq!(find_puncts_end(s, p), (">", " Second>>"));
        let s = ">>>>";
        assert_eq!(find_puncts_end(s, p), (s, ""));
    }

    #[test]
    fn puncts() {
        let p = '>';
        let s = "Hérode> Qu'est-ce que cela veut dire? Le Sauveur du monde?";
        assert_eq!(find_puncts(s, p), FindPuncts::Found("Hérode", ">", &s[8..]));
        let s = "It holds: A >> B.";
        assert_eq!(find_puncts(s, p), FindPuncts::Found("It holds: A ", ">>", " B."));
        let s = "Without angles.";
        assert_eq!(find_puncts(s, p), FindPuncts::NotFound(s));
        let s = "First> Second>>";
        assert_eq!(find_puncts(s, p), FindPuncts::Found("First", ">", " Second>>"));
        let p = '(';
        let s = "Text (direction)";
        assert_eq!(find_puncts(s, p), FindPuncts::Found("Text ", "(", "direction)"));
        let s = "Text ((paren))";
        assert_eq!(find_puncts(s, p), FindPuncts::Found("Text ", "((", "paren))"));
    }

    #[test]
    fn text_tokener() {
        let s = "AAA> xxx ((yy)) (ddd)";
        let token = TextTokener::new(s).collect::<Vec<TextToken>>();
        assert_eq!(token, vec![
            TextToken::Text("AAA".to_owned()),
            TextToken::Rangle,
            TextToken::Text(" xxx ".to_owned()),
            TextToken::LeftBlock(2),
            TextToken::Text("yy".to_owned()),
            TextToken::RightBlock(2),
            TextToken::Text(" ".to_owned()),
            TextToken::Left,
            TextToken::Text("ddd".to_owned()),
            TextToken::Right,
        ]);
    }

    #[test]
    fn tokener() {
        let s = "AAA> xxx (*E)M*((yyy)) zzz)\nxxx";

        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(Event::Start(Tag::Paragraph)));

        let mut parser = EventTokener::new(&mut parser);
        assert_eq!(parser.next(), Some(Token::Text(TextToken::Text("AAA".to_owned()))));
        assert_eq!(parser.next(), Some(Token::Text(TextToken::Rangle)));
        assert_eq!(parser.next(), Some(Token::Text(TextToken::Text(" xxx ".to_owned()))));
        assert_eq!(parser.next(), Some(Token::Text(TextToken::Left)));
        assert_eq!(parser.next(), Some(Token::Event(Event::Start(Tag::Emphasis))));
        assert_eq!(parser.next(), Some(Token::Event(Event::Text("E)M".into()))));
        assert_eq!(parser.next(), Some(Token::Event(Event::End(Tag::Emphasis))));
        assert_eq!(parser.next(), Some(Token::Text(TextToken::LeftBlock(2))));
        assert_eq!(parser.next(), Some(Token::Text(TextToken::Text("yyy".to_owned()))));
        assert_eq!(parser.next(), Some(Token::Text(TextToken::RightBlock(2))));
        assert_eq!(parser.next(), Some(Token::Text(TextToken::Text(" zzz".to_owned()))));
        assert_eq!(parser.next(), Some(Token::Text(TextToken::Right)));
        assert_eq!(parser.next(), Some(Token::Event(Event::SoftBreak)));
        assert_eq!(parser.next(), Some(Token::Text(TextToken::Text("xxx".to_owned()))));
        assert_eq!(parser.next(), Some(Token::Event(Event::End(Tag::Paragraph))));
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn play_line() {
        let s = r#"A> Hello!
(Turning to audience)
B> Bye!
A> What? (__Turning (x)__)"#;
        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(Event::Start(Tag::Paragraph)));
        let mut parser = EventTokener::new(parser);
        let mut lines = PlayLines::new(&mut parser);
        assert_eq!(lines.next(), Some(vec![
            Token::Text(TextToken::Text("A".to_owned())),
            Token::Text(TextToken::Rangle),
            Token::Text(TextToken::Text(" Hello!".to_owned())),
            Token::Event(Event::SoftBreak),
            Token::Text(TextToken::Left),
            Token::Text(TextToken::Text("Turning to audience".to_owned())),
            Token::Text(TextToken::Right),
            Token::Event(Event::SoftBreak),
        ]));
        assert_eq!(lines.next(), Some(vec![
            Token::Text(TextToken::Text("B".to_owned())),
            Token::Text(TextToken::Rangle),
            Token::Text(TextToken::Text(" Bye!".to_owned())),
            Token::Event(Event::SoftBreak),
        ]));
        assert_eq!(lines.next(), Some(vec![
            Token::Text(TextToken::Text("A".to_owned())),
            Token::Text(TextToken::Rangle),
            Token::Text(TextToken::Text(" What? ".to_owned())),
            Token::Text(TextToken::Left),
            Token::Event(Event::Start(Tag::Strong)),
            Token::Event(Event::Text("Turning (x)".into())),
            Token::Event(Event::End(Tag::Strong)),
            Token::Text(TextToken::Right),
        ]));
    }

    fn make_play_lines<'a>(s: &'a str) -> PlayLines<'a, EventTokener<'a, Parser<'a>>> {
        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(Event::Start(Tag::Paragraph)));
        let parser = EventTokener::new(parser);
        PlayLines::new(parser)
    }

    #[test]
    fn one_line_term() {
        let s = "Hello!";
        let mut lines = make_play_lines(s);
        let terms = parse_play_line(lines.next().unwrap());
        assert_eq!(terms, vec![
            LineTerm::Text("Hello!".to_owned()),
        ]);
    }

    #[test]
    fn only_role_name() {
        let s = "Young Syrian>";
        let mut lines = make_play_lines(s);
        let terms = parse_play_line(lines.next().unwrap());
        assert_eq!(terms, vec![
            LineTerm::Role("Young Syrian".to_owned()),
        ]);
    }

    #[test]
    fn with_direction() {
        let s = "A> (Running) Hello!";
        let mut lines = make_play_lines(s);
        let terms = parse_play_line(lines.next().unwrap());
        assert_eq!(terms, vec![
            LineTerm::Role("A".to_owned()),
            LineTerm::Text(" ".to_owned()),
            LineTerm::DirectionStart,
            LineTerm::Text("Running".to_owned()),
            LineTerm::DirectionEnd,
            LineTerm::Text(" Hello!".to_owned()),
        ]);
    }

    #[test]
    fn with_code_in_direction() {
        let s = "A> (Writing `x`) What?";
        let mut lines = make_play_lines(s);
        let terms = parse_play_line(lines.next().unwrap());
        assert_eq!(terms, vec![
            LineTerm::Role("A".to_owned()),
            LineTerm::Text(" ".to_owned()),
            LineTerm::DirectionStart,
            LineTerm::Text("Writing ".to_owned()),
            LineTerm::Event(Event::Code("x".into())),
            LineTerm::DirectionEnd,
            LineTerm::Text(" What?".to_owned()),
        ]);
    }

    #[test]
    fn with_em_in_direction() {
        let s = "A> (Writing *x*) What?";
        let mut lines = make_play_lines(s);
        let terms = parse_play_line(lines.next().unwrap());
        assert_eq!(terms, vec![
            LineTerm::Role("A".to_owned()),
            LineTerm::Text(" ".to_owned()),
            LineTerm::DirectionStart,
            LineTerm::Text("Writing ".to_owned()),
            LineTerm::Event(Event::Start(Tag::Emphasis)),
            LineTerm::Event(Event::Text("x".into())),
            LineTerm::Event(Event::End(Tag::Emphasis)),
            LineTerm::DirectionEnd,
            LineTerm::Text(" What?".to_owned()),
        ]);
    }

    #[test]
    fn distilled_play_line_only_role() {
        let s = "Young Syrian>";
        let mut lines = make_play_lines(s);
        let terms = parse_play_line(lines.next().unwrap());
        let events = distil(terms);
        assert_eq!(events, vec![
            Event::Html(r#"<span class="role">Young Syrian</span>"#.into()),
        ]);
    }

    #[test]
    fn multiple_play_lines() {
        let s = r#"A> Hello!
( Turning to audience )
B> Bye!
A> What? (__Turning (x)__)  "#;
        let mut lines = make_play_lines(s);
        let events = distil(parse_play_line(lines.next().unwrap()));
        assert_eq!(events, vec![
            Event::Html(r#"<span class="role">A</span>"#.into()),
            Event::Text("Hello!".into()),
            Event::SoftBreak,
            Event::Html(r#"<span class="direction">"#.into()),
            Event::Text("Turning to audience".into()),
            Event::Html("</span>".into()),
            Event::SoftBreak,
        ]);
        let events = distil(parse_play_line(lines.next().unwrap()));
        assert_eq!(events, vec![
            Event::Html(r#"<span class="role">B</span>"#.into()),
            Event::Text("Bye!".into()),
            Event::SoftBreak,
        ]);
        let events = distil(parse_play_line(lines.next().unwrap()));
        assert_eq!(events, vec![
            Event::Html(r#"<span class="role">A</span>"#.into()),
            Event::Text("What?".into()),
            Event::Html(r#"<span class="direction">"#.into()),
            Event::Start(Tag::Strong),
            Event::Text("Turning (x)".into()),
            Event::End(Tag::Strong),
            Event::Html("</span>".into()),
        ]);

        let parser = lines.into_inner();
        let mut parser = parser.into_inner();
        // `End(Paragraph)` is consumed.
        assert_eq!(parser.next(), None);
    }
}
