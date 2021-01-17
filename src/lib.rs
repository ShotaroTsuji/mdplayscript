use std::marker::PhantomData;
use std::collections::VecDeque;
use pulldown_cmark::{Event, Tag};
use pulldown_cmark::escape::escape_html;
use trim_in_place::TrimInPlace;

pub struct MdPlay<'a, P> {
    parser: Option<P>,
    queue: VecDeque<Event<'a>>,
    _marker: PhantomData<&'a P>,
}

impl<'a, P> MdPlay<'a, P>
where
    P: Iterator<Item=Event<'a>>,
{
    pub fn new(parser: P) -> Self {
        Self {
            parser: Some(parser),
            queue: VecDeque::new(),
            _marker: PhantomData,
        }
    }
}

impl<'a, P> Iterator for MdPlay<'a, P>
where
    P: Iterator<Item=Event<'a>>,
{
    type Item = Event<'a>;

    fn next<'s>(&'s mut self) -> Option<Event<'a>> {
        if let Some(event) = self.queue.pop_front() {
            return Some(event);
        }

        let event = self.parser.as_mut().unwrap().next();
        match event {
            Some(Event::Start(Tag::Paragraph)) => {
                let parser = self.parser.take().unwrap();
                let tokener = EventTokener::new(parser);
                let mut dialogues = Dialogues::new(tokener);
                while let Some(diag) = dialogues.next() {
                    let events = distil(parse_dialogues(diag));
                    for e in events.into_iter() {
                        self.queue.push_back(e);
                    }
                }
                let tokener = dialogues.into_inner();
                let parser = tokener.into_inner();
                let _ = self.parser.replace(parser);

                self.queue.pop_front()
            },
            Some(event) => Some(event),
            None => None,
        }
    }
}


fn distil<'a>(terms: Vec<Term<'a>>) -> Vec<Event<'a>> {
    if terms.len() == 0 {
        return vec![];
    }

    let (mut events, mut close) = match terms.get(0) {
        Some(Term::Character(_)) => (vec![Event::Html("<p class=\"dialogue\">".into())],
            vec![Event::Html("</p>".into()), Event::SoftBreak]),
        _ => (vec![Event::Start(Tag::Paragraph)], vec![Event::End(Tag::Paragraph)]),
    };

    let mut trim_start = false;

    for term in terms.into_iter() {
        match term {
            Term::Character(character) => {
                let mut buf = "<span class=\"character\">".to_owned();
                escape_html(&mut buf, character.as_str()).unwrap();
                buf += "</span>";
                trim_start = true;

                events.push(Event::Html(buf.into()));
            },
            Term::Text(mut text) => {
                if trim_start {
                    TrimInPlace::trim_start_in_place(&mut text);
                }

                if text.len() > 0 {
                    events.push(Event::Text(text.into()));
                }
            },
            Term::DirectionStart => {
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
            Term::DirectionEnd => {
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
            Term::Event(e) => {
                trim_start = false;
                events.push(e);
            },
        }
    }

    events.append(&mut close);

    events
}

#[derive(Debug,Clone,PartialEq)]
enum Term<'a> {
    DirectionStart,
    DirectionEnd,
    Character(String),
    Text(String),
    Event(Event<'a>),
}

fn token_to_term<'a>(token: Token<'a>, escape: bool) -> Term<'a> {
    match token {
        Token::Event(e) => Term::Event(e),
        Token::Text(tt) => Term::Text(tt.into_string(escape)),
    }
}

fn tokens_to_terms<'a>(tokens: Vec<Token<'a>>, escape: bool) -> impl Iterator<Item=Term<'a>> {
    tokens.into_iter()
        .map(move |t| token_to_term(t, escape))
}

fn parse_normal_line<'a>(line: Vec<Token<'a>>) -> Vec<Term<'a>> {
    tokens_to_terms(line, false)
        .collect()
}

fn parse_direction_in_dialogue<'a, I>(terms: &mut Vec<Term<'a>>, line: &mut I)
where
    I: Iterator<Item=Token<'a>>,
{
    let mut direction = vec![Term::DirectionStart];

    while let Some(token) = line.next() {
        match token {
            Token::Text(TextToken::Right) => {
                direction.push(Term::DirectionEnd);
                break;
            },
            token => {
                direction.push(token_to_term(token, true));
            },
        }
    }

    match direction.last() {
        Some(Term::DirectionEnd) => {
            terms.append(&mut direction);
        },
        _ => {
            for term in direction.into_iter().skip(1) {
                terms.push(term);
            }
        },
    }
}

fn parse_dialogue_line<'a>(line: Vec<Token<'a>>) -> Vec<Term<'a>> {
    let mut terms = Vec::new();
    let mut line = line.into_iter();

    let character = match line.next() {
        Some(Token::Text(TextToken::Text(mut name))) => std::mem::take(&mut name),
        _ => unreachable!(),
    };

    terms.push(Term::Character(character));

    // Consume the right angle.
    assert_eq!(line.next(), Some(Token::Text(TextToken::Rangle)));

    while let Some(token) = line.next() {
        match token {
            Token::Text(TextToken::Left) => {
                parse_direction_in_dialogue(&mut terms, &mut line);
            },
            t => {
                terms.push(token_to_term(t, true));
            },
        }
    }

    terms
}

fn line_starts_with_dialogue<'a>(line: &[Token<'a>]) -> bool {
    match (line.get(0), line.get(1)) {
        (Some(Token::Text(TextToken::Text(_))), Some(Token::Text(TextToken::Rangle))) => true,
        _ => false,
    }
}

fn parse_dialogues<'a>(line: Vec<Token<'a>>) -> Vec<Term<'a>> {
    if line_starts_with_dialogue(&line) {
        parse_dialogue_line(line)
    } else {
        parse_normal_line(line)
    }
}


struct Dialogues<'a, I>
where
    I: Iterator<Item=Token<'a>>,
{
    iter: I,
    fused: bool,
    cache: Vec<Token<'a>>,
}

impl<'a, I> Dialogues<'a, I>
where
    I: Iterator<Item=Token<'a>>,
{
    fn new(iter: I) -> Self {
        Self {
            iter: iter,
            fused: false,
            cache: Vec::new(),
        }
    }

    fn into_inner(self) -> I {
        self.iter
    }
}

fn vec_to_option_if_empty<T>(vec: Vec<T>) -> Option<Vec<T>> {
    if vec.is_empty() {
        None
    } else {
        Some(vec)
    }
}

impl<'a, I> Iterator for Dialogues<'a, I>
where
    I: Iterator<Item=Token<'a>>,
{
    type Item = Vec<Token<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.fused {
            return None;
        }

        let mut line = Vec::new();
        line.append(&mut self.cache);

        while let Some(token) = self.iter.next() {
            match token {
                Token::Event(Event::End(Tag::Paragraph)) => {
                    self.fused = true;
                    return vec_to_option_if_empty(line);
                },
                rangle @ Token::Text(TextToken::Rangle) => {
                    let text = match line.pop() {
                        Some(Token::Text(TextToken::Text(text))) => text,
                        Some(token) => {
                            line.push(token);
                            continue;
                        },
                        None => continue,
                    };

                    let is_first = match line.pop() {
                        None => true,
                        Some(sb @ Token::Event(Event::SoftBreak)) => {
                            line.push(sb);
                            false
                        },
                        Some(token) => {
                            line.push(Token::Text(TextToken::Text(text)));
                            line.push(token);
                            continue;
                        },
                    };

                    if is_first {
                        line.push(Token::Text(TextToken::Text(text)));
                        line.push(rangle);
                    } else {
                        self.cache.push(Token::Text(TextToken::Text(text)));
                        self.cache.push(rangle);
                        return vec_to_option_if_empty(line);
                    }
                },
                t => {
                    line.push(t);
                },
            }
        }

        vec_to_option_if_empty(line)
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
    s.char_indices()
        .find(|(_, c)| ps.contains(*c))
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
    fn dialogues() {
        let s = r#"A> Hello!
(Turning to audience)
B> Bye!
A> What? (__Turning (x)__)"#;
        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(Event::Start(Tag::Paragraph)));
        let mut parser = EventTokener::new(parser);
        let mut lines = Dialogues::new(&mut parser);
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

    #[test]
    fn end_of_dialogues() {
        let s = "Simple Line";
        let mut dialogues = make_dialogues(s);
        assert_eq!(dialogues.next(), Some(vec![
                Token::Text(TextToken::Text("Simple Line".to_owned())),
        ]));
    }

    #[test]
    fn end_of_terms() {
        let s = "Simple Line";
        let mut diag = make_dialogues(s);
        let terms = parse_dialogues(diag.next().unwrap());
        assert_eq!(terms, vec![Term::Text("Simple Line".to_owned())]);
        assert_eq!(diag.next(), None);
    }

    fn make_dialogues<'a>(s: &'a str) -> Dialogues<'a, EventTokener<'a, Parser<'a>>> {
        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(Event::Start(Tag::Paragraph)));
        let parser = EventTokener::new(parser);
        Dialogues::new(parser)
    }

    #[test]
    fn one_term() {
        let s = "Hello!";
        let mut lines = make_dialogues(s);
        let terms = parse_dialogues(lines.next().unwrap());
        assert_eq!(terms, vec![
            Term::Text("Hello!".to_owned()),
        ]);
    }

    #[test]
    fn dialogue_with_only_character_name() {
        let s = "Young Syrian>";

        let mut lines = make_dialogues(s);
        assert_eq!(lines.next(), Some(vec![
                Token::Text(TextToken::Text("Young Syrian".to_owned())),
                Token::Text(TextToken::Rangle),
        ]));

        let mut lines = make_dialogues(s);
        let terms = parse_dialogues(lines.next().unwrap());
        assert_eq!(terms, vec![
            Term::Character("Young Syrian".to_owned()),
        ]);
    }

    #[test]
    fn with_direction() {
        let s = "A> (Running) Hello!";
        let mut lines = make_dialogues(s);
        let terms = parse_dialogues(lines.next().unwrap());
        assert_eq!(terms, vec![
            Term::Character("A".to_owned()),
            Term::Text(" ".to_owned()),
            Term::DirectionStart,
            Term::Text("Running".to_owned()),
            Term::DirectionEnd,
            Term::Text(" Hello!".to_owned()),
        ]);
    }

    #[test]
    fn with_code_in_direction() {
        let s = "A> (Writing `x`) What?";
        let mut lines = make_dialogues(s);
        let terms = parse_dialogues(lines.next().unwrap());
        assert_eq!(terms, vec![
            Term::Character("A".to_owned()),
            Term::Text(" ".to_owned()),
            Term::DirectionStart,
            Term::Text("Writing ".to_owned()),
            Term::Event(Event::Code("x".into())),
            Term::DirectionEnd,
            Term::Text(" What?".to_owned()),
        ]);
    }

    #[test]
    fn with_em_in_direction() {
        let s = "A> (Writing *x*) What?";
        let mut lines = make_dialogues(s);
        let terms = parse_dialogues(lines.next().unwrap());
        assert_eq!(terms, vec![
            Term::Character("A".to_owned()),
            Term::Text(" ".to_owned()),
            Term::DirectionStart,
            Term::Text("Writing ".to_owned()),
            Term::Event(Event::Start(Tag::Emphasis)),
            Term::Event(Event::Text("x".into())),
            Term::Event(Event::End(Tag::Emphasis)),
            Term::DirectionEnd,
            Term::Text(" What?".to_owned()),
        ]);
    }

    #[test]
    fn distilled_dialogues_only_character() {
        let s = "Young Syrian>";
        let mut lines = make_dialogues(s);
        let terms = parse_dialogues(lines.next().unwrap());
        let events = distil(terms);
        assert_eq!(events, vec![
            Event::Html(r#"<p class="dialogue">"#.into()),
            Event::Html(r#"<span class="character">Young Syrian</span>"#.into()),
            Event::Html(r#"</p>"#.into()),
            Event::SoftBreak,
        ]);
    }

    #[test]
    fn multiple_dialogues() {
        let s = r#"A> Hello!
( Turning to audience )
B> Bye!
A> What? (__Turning (x)__)  "#;
        let mut lines = make_dialogues(s);
        let events = distil(parse_dialogues(lines.next().unwrap()));
        assert_eq!(events, vec![
            Event::Html(r#"<p class="dialogue">"#.into()),
            Event::Html(r#"<span class="character">A</span>"#.into()),
            Event::Text("Hello!".into()),
            Event::SoftBreak,
            Event::Html(r#"<span class="direction">"#.into()),
            Event::Text("Turning to audience".into()),
            Event::Html("</span>".into()),
            Event::SoftBreak,
            Event::Html(r#"</p>"#.into()),
            Event::SoftBreak,
        ]);
        let events = distil(parse_dialogues(lines.next().unwrap()));
        assert_eq!(events, vec![
            Event::Html(r#"<p class="dialogue">"#.into()),
            Event::Html(r#"<span class="character">B</span>"#.into()),
            Event::Text("Bye!".into()),
            Event::SoftBreak,
            Event::Html(r#"</p>"#.into()),
            Event::SoftBreak,
        ]);
        let events = distil(parse_dialogues(lines.next().unwrap()));
        assert_eq!(events, vec![
            Event::Html(r#"<p class="dialogue">"#.into()),
            Event::Html(r#"<span class="character">A</span>"#.into()),
            Event::Text("What?".into()),
            Event::Html(r#"<span class="direction">"#.into()),
            Event::Start(Tag::Strong),
            Event::Text("Turning (x)".into()),
            Event::End(Tag::Strong),
            Event::Html("</span>".into()),
            Event::Html(r#"</p>"#.into()),
            Event::SoftBreak,
        ]);

        let parser = lines.into_inner();
        let mut parser = parser.into_inner();
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn parse_multiple_paragraphs() {
        let s = r#"A> Hello!
B> Hello!

Independent Paragraph"#;
        let mut lines = make_dialogues(s);
        let _a_hello = distil(parse_dialogues(lines.next().unwrap()));
        let _b_hello = distil(parse_dialogues(lines.next().unwrap()));
        let parser = lines.into_inner();
        let mut parser = parser.into_inner();
        assert_eq!(parser.next(), Some(Event::Start(Tag::Paragraph)));
    }

    #[test]
    fn multiple_paragraphs_dialogues_end() {
        let s = r#"A> Hello!
B> Hello!

Independent Paragraph"#;
        let mut lines = make_dialogues(s);
        let _a_hello = distil(parse_dialogues(lines.next().unwrap()));
        let _b_hello = distil(parse_dialogues(lines.next().unwrap()));
        assert_eq!(lines.next(), None);
    }
}
