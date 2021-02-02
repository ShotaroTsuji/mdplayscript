//! An extension of Markdown for play scripts
//!
//! This crate is a parser of an extension of Markdown for play scripts.
//! It defines an extended grammar of texts in paragraphs.
//! It is implemented as a filter for `Parser` of pulldown-cmark crate.
//! The goal of this parser is emit an HTML document.
//! Thus it is recommended to pass the parser to `pulldown_cmark::html::push_html` or `write_html`.
//!
//! An implementation of mdbook preprocessor of this crate is
//! [mdbook-playscript](https://github.com/ShotaroTsuji/mdbook-preprocessor).
//!
//! # Example
//!
//! ## Play script format
//!
//! A line starts with a string and a right angle denotes a character's speech.
//! The text before the right angle is the character name and the text after the right angle
//! is the speech of the character.
//! 
//! ```ignore
//! A> Hello!
//! ```
//!
//! A text between a pair of parentheses in a speech denotes a direction.
//!
//! ```ignore
//! A> Hello! (some direction)
//! ```
//!
//! A direction can be placed after the character name.
//! No space is allowed between the right parenthesis and the right angle.
//!
//! ```ignore
//! A (running)> Hello!
//! ```
//!
//! Monologues are surrounded by the following directives: `<!-- playscript-monologue-begin -->`
//! and `<!-- playscript-monologue-end -->`.
//! The texts surrounded by the monologue directives are styled in the normal font style and the
//! directions between the directives are styled in italic.
//!
//! Other forms of texts are handled as normal paragraphs.
//!
//! The examples above are converted into the following HTML:
//!
//! ```
//! use pulldown_cmark::Parser;
//! use pulldown_cmark::html::push_html;
//! use mdplayscript::MdPlayScript;
//!
//! fn convert(s: &str) -> String {
//!     let p = MdPlayScript::new(Parser::new(s));
//!     let mut buf = String::new();
//!     push_html(&mut buf, p);
//!     buf
//! }
//!
//! assert_eq!(convert("A> Hello!"),
//! r#"<div class="speech"><h5><span class="character">A</span></h5><p><span>Hello!</span></p></div>
//! "#);
//! assert_eq!(convert("A> Hello! (some direction)"),
//! r#"<div class="speech"><h5><span class="character">A</span></h5><p><span>Hello!</span><span class="direction">some direction</span></p></div>
//! "#);
//! assert_eq!(convert("A (running)> Hello!"),
//! r#"<div class="speech"><h5><span class="character">A</span><span class="direction">running</span></h5><p><span>Hello!</span></p></div>
//! "#);
//! assert_eq!(convert(r#"<!-- playscript-monologue-begin -->
//! Monologue
//! (direction)
//! <!-- playscript-monologue-end -->
//! "#),
//! r#"<!-- playscript-monologue-begin -->
//! <div class="speech"><p><span>Monologue</span><span class="direction">direction</span></p></div>
//! <!-- playscript-monologue-end -->
//! "#);
//! ```
//!
//! ## CLI program
//!
//! This crate has no proper CLI program. It only has a tiny example program: `examples/single.rs`.
//! It converts a single Markdown into an HTML document.
//! The generated document has a link element which specifies a style sheet `examples/play.css`.
//! I prepared an example input file: `examples/figaro.md`.
//! The output file is
//! [`public/figaro.html`](https://shotarotsuji.github.io/mdplayscript/figaro.html).
//!
//! For Japanese play scripts, I prepared a style sheet `examples/play_ja.css`.
//! If you pass `-l ja` option to `examples/single.rs`, it uses the style sheet
//! `examples/play_ja.css`.
//! The output file is
//! [`public/yushima.html`](https://shotarotsuji.github.io/mdplayscript/yushima.html).
//!
//! # ToDo
//!
//! - [ ] Refactor test codes
//!
//! # License
//!
//! MIT License
//!
use std::marker::PhantomData;
use std::collections::VecDeque;
use pulldown_cmark::{Event, Tag, CowStr};
use trim_in_place::TrimInPlace;

pub mod parser;
pub mod speech;
pub mod renderer;

#[derive(Debug,Clone,Default)]
pub struct MdPlayScriptOption {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
}

pub struct MdPlayScript<'a, P> {
    parser: Option<P>,
    queue: VecDeque<Event<'a>>,
    is_in_monologue: bool,
    option: MdPlayScriptOption,
    _marker: PhantomData<&'a P>,
}

impl<'a, P> MdPlayScript<'a, P>
where
    P: Iterator<Item=Event<'a>>,
{
    pub fn new(parser: P) -> Self {
        Self::with_option(parser, Default::default())
    }

    pub fn with_option(parser: P, option: MdPlayScriptOption) -> Self {
        Self {
            parser: Some(parser),
            queue: VecDeque::new(),
            is_in_monologue: false,
            option: option,
            _marker: PhantomData,
        }
    }
}

impl<'a, P> Iterator for MdPlayScript<'a, P>
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
                let (tokenizer, parser) = EventTokenizer::read_paragraph(parser);
                let mut speeches = Speeches::from_vec(tokenizer);

                while let Some(speech) = speeches.next() {
                    let events = if self.is_in_monologue {
                        distil_monologue(parse_monologue(speech))
                    } else {
                        distil(parse_speech(speech))
                    };
                    for e in events.into_iter() {
                        self.queue.push_back(e);
                    }
                }

                let _ = self.parser.replace(parser);

                self.queue.pop_front()
            },
            Some(html @ Event::Html(_)) => {
                match match_directive(&html) {
                    Some(Directive::MonologueBegin) => {
                        self.is_in_monologue = true;
                    },
                    Some(Directive::MonologueEnd) => {
                        self.is_in_monologue = false;
                    },
                    Some(Directive::Title) => {
                        let s = if let Some(title) = self.option.title.as_ref() {
                            format!("<h1 class=\"cover-title\">{}</h1>\n", title)
                        } else {
                            String::new()
                        };
                        return Some(Event::Html(s.into()));
                    },
                    Some(Directive::Subtitle) => {
                        let s = if let Some(title) = self.option.subtitle.as_ref() {
                            format!("<h2 class=\"cover-title\">{}</h2>\n", title)
                        } else {
                            String::new()
                        };
                        return Some(Event::Html(s.into()));
                    },
                    Some(Directive::Authors) => {
                        if self.option.authors.len() > 0 {
                            let mut s = String::new();
                            for author in self.option.authors.iter() {
                                s += "<p class=\"cover-author\">";
                                s += author;
                                s += "</p>\n";
                            }
                            return Some(Event::Html(s.into()));
                        }
                    },
                    None => {},
                }

                Some(html)
            },
            Some(event) => Some(event),
            None => None,
        }
    }
}

#[derive(Debug,Clone,PartialEq)]
enum Directive {
    MonologueBegin,
    MonologueEnd,
    Title,
    Subtitle,
    Authors,
}

fn match_directive<'a>(event: &Event<'a>) -> Option<Directive> {
    let s = match event {
        Event::Html(s) => s.as_ref(),
        _ => return None,
    };

    let s = s.replace("<!--", "");
    let s = s.trim_start();

    let s = s.replace("playscript-", "");

    if s.starts_with("monologue-begin") {
        return Some(Directive::MonologueBegin);
    } else if s.starts_with("monologue-end") {
        return Some(Directive::MonologueEnd);
    } else if s.starts_with("title") {
        return Some(Directive::Title);
    } else if s.starts_with("subtitle") {
        return Some(Directive::Subtitle);
    } else if s.starts_with("authors") {
        return Some(Directive::Authors);
    }

    None
}

const PARA_START: Event<'static> = Event::Html(CowStr::Borrowed("<p>"));
const PARA_END: Event<'static> = Event::Html(CowStr::Borrowed("</p>"));
const P_START: Event<'static> = Event::Start(Tag::Paragraph);
const P_END: Event<'static> = Event::End(Tag::Paragraph);
const H5_START: Event<'static> = Event::Html(CowStr::Borrowed("<h5>"));
const H5_END: Event<'static> = Event::Html(CowStr::Borrowed("</h5>"));
const DIV_SPEECH: Event<'static> = Event::Html(CowStr::Borrowed(r#"<div class="speech">"#));
const DIV_END: Event<'static> = Event::Html(CowStr::Borrowed("</div>"));
const SPAN_START: Event<'static> = Event::Html(CowStr::Borrowed("<span>"));
const SPAN_CHARACTER: Event<'static> = Event::Html(CowStr::Borrowed(r#"<span class="character">"#));
const SPAN_DIRECTION: Event<'static> = Event::Html(CowStr::Borrowed(r#"<span class="direction">"#));
const SPAN_END: Event<'static> = Event::Html(CowStr::Borrowed("</span>"));

fn trim_end_of_top<'a>(events: &mut Vec<Event<'a>>) {
    match events.pop() {
        Some(Event::Text(s)) => {
            let mut s = s.into_string();
            TrimInPlace::trim_end_in_place(&mut s);
            events.push(Event::Text(s.into()));
        },
        Some(h @ Event::Html(_)) if h == SPAN_START || h == SPAN_END => {
            match events.pop() {
                Some(Event::Text(s)) => {
                    let mut s = s.into_string();
                    TrimInPlace::trim_end_in_place(&mut s);
                    events.push(Event::Text(s.into()));
                },
                Some(e) => {
                    events.push(e);
                },
                _ => {},
            }
            events.push(h);
        },
        Some(e) => {
            events.push(e);
        },
        _ => {},
    }
}

fn distil_speech<'a>(terms: Vec<Term<'a>>) -> Vec<Event<'a>> {
    let mut events = vec![DIV_SPEECH.clone()];
    let mut terms = terms.into_iter();

    let mut trim_start = false;
    let mut text_needs_span = true;

    while let Some(term) = terms.next() {
        match term {
            Term::HeadingStart => {
                events.push(H5_START.clone());
                trim_start = false;
                text_needs_span = false;
            },
            Term::HeadingEnd => {
                events.push(H5_END.clone());
                trim_start = true;
                text_needs_span = true;
            },
            Term::BodyStart => {
                events.push(PARA_START.clone());
            },
            Term::BodyEnd => {
                match events.last() {
                    Some(e) if e == &PARA_START => {
                        let _ = events.pop();
                    },
                    _ => {
                        events.push(PARA_END.clone());
                    },
                }
            },
            Term::Character(mut s) => {
                TrimInPlace::trim_in_place(&mut s);
                events.push(SPAN_CHARACTER.clone());
                events.push(Event::Text(s.into()));
                events.push(SPAN_END.clone());
                trim_start = false;
            },
            Term::DirectionStart => {
                trim_end_of_top(&mut events);
                events.push(SPAN_DIRECTION.clone());
                trim_start = false;
                text_needs_span = false;
            },
            Term::DirectionEnd => {
                trim_end_of_top(&mut events);
                events.push(SPAN_END.clone());
                trim_start = true;
                text_needs_span = true;
            },
            Term::Text(mut s) => {
                if trim_start {
                    TrimInPlace::trim_start_in_place(&mut s);
                }
                if s.len() > 0 {
                    if text_needs_span {
                        events.push(SPAN_START.clone());
                    }

                    events.push(Event::Text(s.into()));

                    if text_needs_span {
                        events.push(SPAN_END.clone());
                    }
                }
            },
            Term::Event(Event::SoftBreak) => {},
            Term::Event(e) => {
                events.push(e);
            },
        }
    }

    events.push(DIV_END.clone());
    events.push(Event::SoftBreak);

    events
}

fn distil_monologue<'a>(terms: Vec<Term<'a>>) -> Vec<Event<'a>> {
    if terms.len() == 0 {
        Vec::new()
    } else {
        distil_speech(terms)
    }
}


fn distil<'a>(terms: Vec<Term<'a>>) -> Vec<Event<'a>> {
    if terms.len() == 0 {
        return vec![];
    }

    let (mut events, mut close) = match terms.get(0) {
        Some(Term::HeadingStart) => return distil_speech(terms),
        _ => (vec![P_START.clone()], vec![P_END.clone()]),
    };

    let mut trim_start = false;

    for term in terms.into_iter() {
        match term {
            Term::HeadingStart | Term::HeadingEnd |
                Term::BodyStart | Term::BodyEnd |
            Term::Character(_) => unreachable!(),
            Term::Text(mut text) => {
                if trim_start {
                    TrimInPlace::trim_start_in_place(&mut text);
                }
                if text.len() > 0 {
                    events.push(Event::Text(text.into()));
                }
                trim_start = false;
            },
            Term::DirectionStart => {
                trim_end_of_top(&mut events);
                events.push(SPAN_DIRECTION.clone());
                trim_start = true;
            },
            Term::DirectionEnd => {
                trim_end_of_top(&mut events);
                events.push(SPAN_END.clone());
                trim_start = true;
            },
            Term::Event(Event::SoftBreak) => {},
            Term::Event(e) => {
                events.push(e);
                trim_start = false;
            },
        }
    }

    events.append(&mut close);

    events
}

#[derive(Debug,Clone,PartialEq)]
enum Term<'a> {
    HeadingStart,
    HeadingEnd,
    BodyStart,
    BodyEnd,
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

fn parse_direction_in_speech<'a>(line: &mut VecDeque<Token<'a>>, terms: &mut Vec<Term<'a>>)
{
    let mut direction = vec![Term::DirectionStart];

    while let Some(token) = line.pop_front() {
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

fn parse_speech_body<'a>(line: &mut VecDeque<Token<'a>>, terms: &mut Vec<Term<'a>>) {
    terms.push(Term::BodyStart);
    while let Some(token) = line.pop_front() {
        match token {
            Token::Text(TextToken::Left) => {
                parse_direction_in_speech(line, terms);
            },
            t => {
                terms.push(token_to_term(t, true));
            },
        }
    }
    terms.push(Term::BodyEnd);
}

fn parse_speech_line<'a>(line: Vec<Token<'a>>) -> Vec<Term<'a>> {
    let mut terms = Vec::new();
    let mut line: VecDeque<_> = line.into();

    parse_speech_heading(&mut line, &mut terms);
    parse_speech_body(&mut line, &mut terms);

    terms
}

fn parse_monologue<'a>(line: Vec<Token<'a>>) -> Vec<Term<'a>> {
    let mut terms = vec![];
    let mut line: VecDeque<_> = line.into();

    parse_speech_body(&mut line, &mut terms);

    terms
}

fn parse_speech<'a>(line: Vec<Token<'a>>) -> Vec<Term<'a>> {
    if speech_heading_kind(&line).is_some() {
        parse_speech_line(line)
    } else {
        parse_normal_line(line)
    }
}

fn parse_speech_heading<'a>(line: &mut VecDeque<Token<'a>>, terms: &mut Vec<Term<'a>>) {
    terms.push(Term::HeadingStart);

    match line.pop_front() {
        Some(Token::Text(TextToken::Text(s))) => {
            terms.push(Term::Character(s));
        },
        _ => unreachable!(),
    }

    match line.pop_front() {
        Some(Token::Text(TextToken::Rangle)) => {
            terms.push(Term::HeadingEnd);
            return;
        },
        Some(Token::Text(TextToken::Left)) => {},
        _ => unreachable!(),
    }

    match line.pop_front() {
        Some(Token::Text(TextToken::Text(s))) => {
            terms.push(Term::DirectionStart);
            terms.push(Term::Text(s));
            terms.push(Term::DirectionEnd);
        },
        _ => unreachable!(),
    }

    match line.pop_front() {
        Some(Token::Text(TextToken::Right)) => {},
        _ => unreachable!(),
    }

    match line.pop_front() {
        Some(Token::Text(TextToken::Rangle)) => {},
        _ => unreachable!(),
    }

    terms.push(Term::HeadingEnd);
}

#[derive(Debug,Clone,PartialEq)]
enum HeadingKind {
    Simple,
    WithDirection,
}

// text [ '(' text ')' ] '>'
fn speech_heading_kind<'a>(line: &[Token<'a>]) -> Option<HeadingKind> {
    match line {
        [Token::Text(TextToken::Text(_)), Token::Text(TextToken::Rangle), ..] => Some(HeadingKind::Simple),
        [Token::Text(TextToken::Text(_)), Token::Text(TextToken::Left),
         Token::Text(TextToken::Text(_)), Token::Text(TextToken::Right),
         Token::Text(TextToken::Rangle), ..] => Some(HeadingKind::WithDirection),
        _ => None,
    }
}

struct Speeches<'a> {
    tokens: Vec<Token<'a>>,
}

impl<'a> Speeches<'a> {
    fn from_vec(vec: Vec<Token<'a>>) -> Self {
        Self {
            tokens: vec,
        }
    }
}

impl<'a> Iterator for Speeches<'a> {
    type Item = Vec<Token<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.tokens.is_empty() {
            return None;
        }

        let mut current = 0;

        while let Some(pos) = self.tokens[current..].iter()
            .position(|token| {
            match token {
                Token::Event(Event::SoftBreak) => true,
                _ => false,
            }
        }) {
            let found_line_end = if let Some(slice) = self.tokens.get(current+pos+1..) {
                speech_heading_kind(slice).is_some()
            } else {
                true
            };
            if found_line_end {
                let remain = self.tokens.split_off(current+pos+1);
                return Some(std::mem::replace(&mut self.tokens, remain));
            }
            current = current + pos + 1;
        }

        Some(std::mem::take(&mut self.tokens))
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

struct EventTokenizer<'a, P> {
    parser: P,
    queue: VecDeque<Token<'a>>,
    nest_level: usize,
    fused: bool,
    _phantom: PhantomData<&'a P>,
}

impl<'a, P> EventTokenizer<'a, P>
where
    P: 'a + Iterator<Item=Event<'a>>,
{
    fn new(parser: P) -> EventTokenizer<'a, P> {
        Self {
            parser: parser,
            queue: VecDeque::new(),
            nest_level: 0,
            fused: false,
            _phantom: PhantomData,
        }
    }

    #[allow(dead_code)]
    fn into_inner(self) -> P {
        self.parser
    }

    fn read_paragraph(parser: P) -> (Vec<Token<'a>>, P) {
        let mut tokenizer = Self::new(parser);
        let mut paragraph = Vec::new();

        while let Some(token) = tokenizer.next() {
            paragraph.push(token);
        }

        (paragraph, tokenizer.parser)
    }
}

impl<'a, P> Iterator for EventTokenizer<'a, P>
where
    P: Iterator<Item=Event<'a>>,
{
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.fused {
            None
        } else if let Some(token) = self.queue.pop_front() {
            Some(token)
        } else if let Some(event) = self.parser.next() {
            match (event, self.nest_level) {
                (Event::End(Tag::Paragraph), 0) => {
                    self.fused = true;
                    None
                },
                (ret @ Event::Start(_), l) => {
                    self.nest_level = l + 1;
                    Some(Token::Event(ret))
                },
                (ret @ Event::End(_), l) => {
                    if l > 0 {
                        self.nest_level = l - 1;
                    }
                    Some(Token::Event(ret))
                },
                (Event::Text(text), 0) => {
                    for t in TextTokenizer::new(&text) {
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

struct TextTokenizer<'a> {
    s: &'a str,
}

impl<'a> TextTokenizer<'a> {
    fn new(s: &'a str) -> Self {
        TextTokenizer {
            s: s,
        }
    }
}

impl<'a> Iterator for TextTokenizer<'a> {
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


pub fn find_one_of(s: &str, ps: &str) -> Option<(usize, char)> {
    s.char_indices()
        .find(|(_, c)| ps.contains(*c))
}

pub fn find_puncts_end(s: &str, p: char) -> (&str, &str) {
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

    const PARA_TAG_START: Event<'static> = Event::Start(Tag::Paragraph);

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
    fn token_of_rangle_after_rparen() {
        let s = ")>";
        let tokens: Vec<TextToken> = TextTokenizer::new(s).collect();
        assert_eq!(tokens, vec![
            TextToken::Right,
            TextToken::Rangle,
        ]);
    }

    #[test]
    fn text_tokenizer() {
        let s = "AAA> xxx ((yy)) (ddd)";
        let token = TextTokenizer::new(s).collect::<Vec<TextToken>>();
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
    fn tokenizer() {
        let s = "AAA> xxx (*E)M*((yyy)) zzz)\nxxx";

        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(Event::Start(Tag::Paragraph)));

        let mut parser = EventTokenizer::new(&mut parser);
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
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn tokens_in_character_and_direction() {
        let s = "A (aaa)> xxx";
        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(PARA_TAG_START));

        let tokens: Vec<Token<'_>> = EventTokenizer::new(parser).collect();
        assert_eq!(tokens, vec![
            Token::Text(TextToken::Text("A ".to_owned())),
            Token::Text(TextToken::Left),
            Token::Text(TextToken::Text("aaa".to_owned())),
            Token::Text(TextToken::Right),
            Token::Text(TextToken::Rangle),
            Token::Text(TextToken::Text(" xxx".to_owned())),
        ]);
    }

    #[test]
    fn event_tokenizer_for_paragraphs() {
        let s = r#"First

Second

Third"#;
        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(PARA_TAG_START));

        let mut tokenizer = EventTokenizer::new(parser);
        assert_eq!(tokenizer.next(), Some(Token::Text(TextToken::Text("First".to_owned()))));
        assert_eq!(tokenizer.next(), None);

        let mut parser = tokenizer.into_inner();
        assert_eq!(parser.next(), Some(PARA_TAG_START));

        let mut tokenizer = EventTokenizer::new(parser);
        assert_eq!(tokenizer.next(), Some(Token::Text(TextToken::Text("Second".to_owned()))));
        assert_eq!(tokenizer.next(), None);

        let mut parser = tokenizer.into_inner();
        assert_eq!(parser.next(), Some(PARA_TAG_START));

        let mut tokenizer = EventTokenizer::new(parser);
        assert_eq!(tokenizer.next(), Some(Token::Text(TextToken::Text("Third".to_owned()))));
        assert_eq!(tokenizer.next(), None);

        let mut parser = tokenizer.into_inner();
        assert_eq!(parser.next(), None);
    }

    fn test_starts_with_speech_heading(s: &str, expected: Option<HeadingKind>) {
        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(PARA_TAG_START));
        let (tokens, mut parser) = EventTokenizer::read_paragraph(parser);
        assert_eq!(speech_heading_kind(&tokens), expected);
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn lines_start_with_speech_heading() {
        test_starts_with_speech_heading("Normal line", None);
        test_starts_with_speech_heading("A>", Some(HeadingKind::Simple));
        test_starts_with_speech_heading("A> Hello", Some(HeadingKind::Simple));
        test_starts_with_speech_heading("A (laughing)>", Some(HeadingKind::WithDirection));
        test_starts_with_speech_heading("A (running)> Hello *World*.", Some(HeadingKind::WithDirection));
    }

    #[test]
    fn speeches_from_lines() {
        let s = "A> Hello!\nB (running)> Hi!\nA> What?\nWho?\n(leave)\nB> Wait!";
        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(PARA_TAG_START));

        let (tokens, _) = EventTokenizer::read_paragraph(parser);
        let mut speeches = Speeches::from_vec(tokens);
        assert_eq!(speeches.next(), Some(vec![
                Token::Text(TextToken::Text("A".to_owned())),
                Token::Text(TextToken::Rangle),
                Token::Text(TextToken::Text(" Hello!".to_owned())),
                Token::Event(Event::SoftBreak),
        ]));
        assert_eq!(speeches.next(), Some(vec![
                Token::Text(TextToken::Text("B ".to_owned())),
                Token::Text(TextToken::Left),
                Token::Text(TextToken::Text("running".to_owned())),
                Token::Text(TextToken::Right),
                Token::Text(TextToken::Rangle),
                Token::Text(TextToken::Text(" Hi!".to_owned())),
                Token::Event(Event::SoftBreak),
        ]));
        assert_eq!(speeches.next(), Some(vec![
                Token::Text(TextToken::Text("A".to_owned())),
                Token::Text(TextToken::Rangle),
                Token::Text(TextToken::Text(" What?".to_owned())),
                Token::Event(Event::SoftBreak),
                Token::Text(TextToken::Text("Who?".to_owned())),
                Token::Event(Event::SoftBreak),
                Token::Text(TextToken::Left),
                Token::Text(TextToken::Text("leave".to_owned())),
                Token::Text(TextToken::Right),
                Token::Event(Event::SoftBreak),
        ]));
        assert_eq!(speeches.next(), Some(vec![
                Token::Text(TextToken::Text("B".to_owned())),
                Token::Text(TextToken::Rangle),
                Token::Text(TextToken::Text(" Wait!".to_owned())),
        ]));
    }

    #[test]
    fn speech_with_only_character_name() {
        let s = "Young Syrian>";

        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(PARA_TAG_START));
        let tokens: Vec<Token<'_>> = EventTokenizer::new(parser).collect();
        assert_eq!(tokens, vec![
            Token::Text(TextToken::Text("Young Syrian".to_owned())),
            Token::Text(TextToken::Rangle),
        ]);

        let mut speeches = Speeches::from_vec(tokens);

        let speech = speeches.next().unwrap();
        assert_eq!(speech, vec![
                Token::Text(TextToken::Text("Young Syrian".to_owned())),
                Token::Text(TextToken::Rangle),
        ]);

        let terms = parse_speech(speech);
        assert_eq!(terms, vec![
            Term::HeadingStart,
            Term::Character("Young Syrian".to_owned()),
            Term::HeadingEnd,
            Term::BodyStart,
            Term::BodyEnd,
        ]);

        let events = distil(terms);
        assert_eq!(events, vec![
            DIV_SPEECH,
            H5_START,
            SPAN_CHARACTER,
            Event::Text("Young Syrian".into()),
            SPAN_END,
            H5_END,
            DIV_END,
            Event::SoftBreak,
        ]);
    }

    #[test]
    fn character_with_direction() {
        let s = "A (Running)> Hello!";

        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(PARA_TAG_START));
        let tokens: Vec<Token<'_>> = EventTokenizer::new(parser).collect();
        let mut speeches = Speeches::from_vec(tokens);
        let speech = speeches.next().unwrap();

        let terms = parse_speech(speech);
        assert_eq!(terms, vec![
            Term::HeadingStart,
            Term::Character("A ".to_owned()),
            Term::DirectionStart,
            Term::Text("Running".to_owned()),
            Term::DirectionEnd,
            Term::HeadingEnd,
            Term::BodyStart,
            Term::Text(" Hello!".to_owned()),
            Term::BodyEnd,
        ]);
    }

    #[test]
    fn with_code_in_direction() {
        let s = "A> (Writing `x`) What?";
        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(PARA_TAG_START));
        let (para, _) = EventTokenizer::read_paragraph(parser);
        let mut speeches = Speeches::from_vec(para);
        let terms = parse_speech(speeches.next().unwrap());
        assert_eq!(terms, vec![
            Term::HeadingStart,
            Term::Character("A".to_owned()),
            Term::HeadingEnd,
            Term::BodyStart,
            Term::Text(" ".to_owned()),
            Term::DirectionStart,
            Term::Text("Writing ".to_owned()),
            Term::Event(Event::Code("x".into())),
            Term::DirectionEnd,
            Term::Text(" What?".to_owned()),
            Term::BodyEnd,
        ]);
    }

    #[test]
    fn with_em_in_direction() {
        let s = "A> (Writing *x*) What?";
        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(PARA_TAG_START));
        let (para, _) = EventTokenizer::read_paragraph(parser);
        let mut speeches = Speeches::from_vec(para);
        let terms = parse_speech(speeches.next().unwrap());
        assert_eq!(terms, vec![
            Term::HeadingStart,
            Term::Character("A".to_owned()),
            Term::HeadingEnd,
            Term::BodyStart,
            Term::Text(" ".to_owned()),
            Term::DirectionStart,
            Term::Text("Writing ".to_owned()),
            Term::Event(Event::Start(Tag::Emphasis)),
            Term::Event(Event::Text("x".into())),
            Term::Event(Event::End(Tag::Emphasis)),
            Term::DirectionEnd,
            Term::Text(" What?".to_owned()),
            Term::BodyEnd,
        ]);
    }

    #[test]
    fn multiple_speeches() {
        let s = r#"A> Hello!
( Turning to audience )
B> Bye!
A> What? (__Turning (x)__)  "#;
        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(PARA_TAG_START));
        let (paragraph, _) = EventTokenizer::read_paragraph(parser);
        let mut speeches = Speeches::from_vec(paragraph);
        let events = distil(parse_speech(speeches.next().unwrap()));
        assert_eq!(events, vec![
            DIV_SPEECH,
            H5_START,
            SPAN_CHARACTER,
            Event::Text("A".into()),
            SPAN_END,
            H5_END,
            PARA_START,
            SPAN_START,
            Event::Text("Hello!".into()),
            SPAN_END,
            SPAN_DIRECTION,
            Event::Text(" Turning to audience".into()),
            SPAN_END,
            PARA_END,
            DIV_END,
            Event::SoftBreak,
        ]);
        let events = distil(parse_speech(speeches.next().unwrap()));
        assert_eq!(events, vec![
            DIV_SPEECH,
            H5_START,
            SPAN_CHARACTER,
            Event::Text("B".into()),
            SPAN_END,
            H5_END,
            PARA_START,
            SPAN_START,
            Event::Text("Bye!".into()),
            SPAN_END,
            PARA_END,
            DIV_END,
            Event::SoftBreak,
        ]);
        let events = distil(parse_speech(speeches.next().unwrap()));
        assert_eq!(events, vec![
            DIV_SPEECH,
            H5_START,
            SPAN_CHARACTER,
            Event::Text("A".into()),
            SPAN_END,
            H5_END,
            PARA_START,
            SPAN_START,
            Event::Text("What?".into()),
            SPAN_END,
            SPAN_DIRECTION,
            Event::Start(Tag::Strong),
            Event::Text("Turning (x)".into()),
            Event::End(Tag::Strong),
            SPAN_END,
            PARA_END,
            DIV_END,
            Event::SoftBreak,
        ]);
    }

    #[test]
    fn heading_with_direction() {
        let s = "A (Running)> Hello!";

        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(PARA_TAG_START));
        let (tokens, _) = EventTokenizer::read_paragraph(parser);
        let mut speeches = Speeches::from_vec(tokens);

        let speech = speeches.next().unwrap();

        assert_eq!(speech, vec![
            Token::Text(TextToken::Text("A ".into())),
            Token::Text(TextToken::Left),
            Token::Text(TextToken::Text("Running".into())),
            Token::Text(TextToken::Right),
            Token::Text(TextToken::Rangle),
            Token::Text(TextToken::Text(" Hello!".into())),
        ]);

        let events = distil(parse_speech(speech));
        assert_eq!(events, vec![
            DIV_SPEECH,
            H5_START,
            SPAN_CHARACTER,
            Event::Text("A".into()),
            SPAN_END,
            SPAN_DIRECTION,
            Event::Text("Running".into()),
            SPAN_END,
            H5_END,
            PARA_START,
            SPAN_START,
            Event::Text("Hello!".into()),
            SPAN_END,
            PARA_END,
            DIV_END,
            Event::SoftBreak,
        ]);
    }

    #[test]
    fn heading_without_direction() {
        let s = "A> Hello!";

        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(PARA_TAG_START));

        let (tokens, _) = EventTokenizer::read_paragraph(parser);
        let mut speeches = Speeches::from_vec(tokens);
        let speech = speeches.next().unwrap();

        let events = distil(parse_speech(speech));
        assert_eq!(events, vec![
            DIV_SPEECH,
            H5_START,
            SPAN_CHARACTER,
            Event::Text("A".into()),
            SPAN_END,
            H5_END,
            PARA_START,
            SPAN_START,
            Event::Text("Hello!".into()),
            SPAN_END,
            PARA_END,
            DIV_END,
            Event::SoftBreak,
        ]);
    }

    #[test]
    fn heading_with_normal_line() {
        let s = "Hello!";

        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(PARA_TAG_START));

        let (tokens, _) = EventTokenizer::read_paragraph(parser);
        let mut speeches = Speeches::from_vec(tokens);
        let speech = speeches.next().unwrap();

        let events = distil(parse_speech(speech));
        assert_eq!(events, vec![
            P_START,
            Event::Text("Hello!".into()),
            P_END,
        ]);
    }

    #[test]
    fn monologue_begin_directive() {
        let s = "<!-- monologue-begin -->";

        let mut parser = Parser::new(s);
        let event = parser.next().unwrap();
        assert_eq!(match_directive(&event), Some(Directive::MonologueBegin));
    }

    #[test]
    fn monologue_end_directive() {
        let s = "<!-- monologue-end -->";

        let mut parser = Parser::new(s);
        let event = parser.next().unwrap();
        assert_eq!(match_directive(&event), Some(Directive::MonologueEnd));
    }

    #[test]
    fn monologue_example() {
        let s = r#"<!-- monologue-begin -->
Monologue 1 ( direction )

Monologue (direction) Monologue
<!-- monologue-end -->
"#;

        let mut parser = Parser::new(s);
        let begin = parser.next().unwrap();
        assert_eq!(match_directive(&begin), Some(Directive::MonologueBegin));

        assert_eq!(parser.next(), Some(PARA_TAG_START));

        let (tokens, parser) = EventTokenizer::read_paragraph(&mut parser);
        let mut speeches = Speeches::from_vec(tokens);
        let speech = speeches.next().unwrap();

        let events = distil_monologue(parse_monologue(speech));
        assert_eq!(events, vec![
            DIV_SPEECH,
            PARA_START,
            SPAN_START,
            Event::Text(CowStr::Borrowed("Monologue 1")),
            SPAN_END,
            SPAN_DIRECTION,
            Event::Text(CowStr::Borrowed(" direction")),
            SPAN_END,
            PARA_END,
            DIV_END,
            Event::SoftBreak,
        ]);

        assert_eq!(parser.next(), Some(PARA_TAG_START));

        let (tokens, parser) = EventTokenizer::read_paragraph(parser);
        let mut speeches = Speeches::from_vec(tokens);
        let speech = speeches.next().unwrap();

        let events = distil_monologue(parse_monologue(speech));
        assert_eq!(events, vec![
            DIV_SPEECH,
            PARA_START,
            SPAN_START,
            Event::Text(CowStr::Borrowed("Monologue")),
            SPAN_END,
            SPAN_DIRECTION,
            Event::Text(CowStr::Borrowed("direction")),
            SPAN_END,
            SPAN_START,
            Event::Text(CowStr::Borrowed("Monologue")),
            SPAN_END,
            PARA_END,
            DIV_END,
            Event::SoftBreak,
        ]);

        let end = parser.next().unwrap();
        assert_eq!(match_directive(&end), Some(Directive::MonologueEnd));
    }

    //#[test]
    fn speech_with_multi_lines() {
        let s = "A ( Running)> Hello!\nMaam.\nGoodbye!";

        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(PARA_TAG_START));
        let (tokens, _) = EventTokenizer::read_paragraph(parser);
        let mut speeches = Speeches::from_vec(tokens);

        let speech = speeches.next().unwrap();

        /*
        assert_eq!(speech, vec![
            Token::Text(TextToken::Text("A ".into())),
            Token::Text(TextToken::Left),
            Token::Text(TextToken::Text("Running".into())),
            Token::Text(TextToken::Right),
            Token::Text(TextToken::Rangle),
            Token::Text(TextToken::Text(" Hello!".into())),
        ]);
        */

        let events = parse_speech(speech);
        let events = distil(events);
        assert_eq!(events, vec![]);
    }

    #[test]
    fn title_and_authors() {
        let s = "<!-- playscript-title -->\n<!-- playscript-authors -->";
        let opt = MdPlayScriptOption {
            title: Some("Title".to_owned()),
            subtitle: None,
            authors: vec!["Author".to_owned(), "B".to_owned()],
        };
        let p = MdPlayScript::with_option(Parser::new(s), opt);
        let mut buf = String::new();
        pulldown_cmark::html::push_html(&mut buf, p);
        assert_eq!(buf, r#"<h1 class="cover-title">Title</h1>
<p class="cover-author">Author</p>
<p class="cover-author">B</p>
"#);
    }
}
