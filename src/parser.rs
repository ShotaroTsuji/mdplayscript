use std::marker::PhantomData;
use pulldown_cmark::{Event, Tag};

/// An iterator which fuses when a paragraph end comes.
#[derive(Debug)]
pub struct FuseOnParagraphEnd<'a, I> {
    iter: I,
    is_fused: bool,
    _marker: PhantomData<&'a I>,
}

impl<'a, I> FuseOnParagraphEnd<'a, I>
where
    I: Iterator<Item=Event<'a>>,
{
    pub fn new(parser: I) -> Self {
        Self {
            iter: parser,
            is_fused: false,
            _marker: PhantomData,
        }
    }

    pub fn into_inner(self) -> I {
        self.iter
    }
}

impl<'a, I> Iterator for FuseOnParagraphEnd<'a, I>
where
    I: Iterator<Item=Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_fused {
            return None;
        }

        match self.iter.next() {
            Some(Event::End(Tag::Paragraph)) | None => {
                self.is_fused = true;
                None
            },
            Some(e) => {
                Some(e)
            },
        }
    }
}

/// Split events with speech starting line.
#[derive(Debug)]
pub struct Speeches<'a, I> {
    iter: FuseOnParagraphEnd<'a, I>,
    is_first: bool,
    last: Option<Event<'a>>,
}

impl<'a, I> Speeches<'a, I>
where
    I: Iterator<Item=Event<'a>>,
{
    pub fn new(paragraph: FuseOnParagraphEnd<'a, I>) -> Self {
        Self {
            iter: paragraph,
            is_first: true,
            last: None,
        }
    }

    pub fn into_inner(self) -> FuseOnParagraphEnd<'a, I> {
        self.iter
    }
}

impl<'a, I> Iterator for Speeches<'a, I>
where
    I: Iterator<Item=Event<'a>>,
{
    type Item = Vec<Event<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut speech = Vec::new();

        if let Some(event) = self.last.take() {
            speech.push(event);
        }

        while let Some(event) = self.iter.next() {
            match event {
                Event::Text(s) if is_speech_start(s.as_ref()) => {
                    if self.is_first {
                        speech.push(Event::Text(s));
                        self.is_first = false;
                    } else {
                        self.last.replace(Event::Text(s));
                        break;
                    }
                },
                event => {
                    speech.push(event);
                },
            }
        }

        if speech.is_empty() {
            None
        } else {
            Some(speech)
        }
    }
}

fn find_one_char(s: &str, pat: char) -> Option<usize> {
    let start = match s.find(pat) {
        Some(pos) => pos,
        None => return None,
    };

    let after = &s[start+1..];
    if after.starts_with(pat) {
        None
    } else {
        Some(start)
    }
}

/// Split speech heading and body.
///
/// Speech format is `Character> speech`. This function splits at a first single right angle.
/// If it has no right angle, or two or more right angles, it returns `None`.
pub fn split_speech_heading(s: &str) -> Option<(&str, &str)> {
    find_one_char(s, '>')
        .map(|pos| (&s[..pos], &s[pos+1..]))
}

pub fn is_speech_start(s: &str) -> bool {
    find_one_char(s, '>').is_some()
}

#[cfg(test)]
mod test {
    use super::*;
    use pulldown_cmark::Parser;
    use pulldown_cmark::CowStr;

    fn consume_paragraph<'a>(s: &'a str) -> Parser<'a> {
        let parser = Parser::new(s);
        let mut paragraph = FuseOnParagraphEnd::new(parser);

        while let Some(_) = paragraph.next() {
        }

        paragraph.into_inner()
    }

    #[test]
    fn consume_simple_two_paragraph() {
        let s = r#"A> Hello!
How are you?

B> Hello!"#;
        assert_eq!(consume_paragraph(s).next(), Some(Event::Start(Tag::Paragraph)));
    }

    #[test]
    fn find_single_right_angle() {
        assert_eq!(find_one_char("A> xxx", '>'), Some(1));
        assert_eq!(find_one_char("AAA>", '>'), Some(3));
    }

    #[test]
    fn find_one_char_with_angles() {
        assert_eq!(find_one_char("A>> xxx", '>'), None);
        assert_eq!(find_one_char("AAA>>>", '>'), None);
    }

    #[test]
    fn split_speech_line() {
        assert_eq!(split_speech_heading("A> xxx"), Some(("A", " xxx")));
        assert_eq!(split_speech_heading("AAA>"), Some(("AAA", "")));
        assert_eq!(split_speech_heading("A (ddd)>"), Some(("A (ddd)", "")));
    }

    #[test]
    fn split_normal_line() {
        assert_eq!(split_speech_heading("A xxx"), None);
        assert_eq!(split_speech_heading("AAA"), None);
    }

    fn make_speeches_iter<'a>(s: &'a str) -> Speeches<'a, Parser<'a>> {
        let mut parser = Parser::new(s);
        assert_eq!(parser.next(), Some(Event::Start(Tag::Paragraph)));
        Speeches::new(FuseOnParagraphEnd::new(parser))
    }

    #[test]
    fn speeches_iter_with_one_line() {
        let s = "A> Hello!";
        let mut it = make_speeches_iter(s);
        assert_eq!(it.next(), Some(vec![Event::Text(CowStr::Borrowed("A> Hello!"))]));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn speeches_iter_with_three_lines() {
        let s = "A> Hello!\nHello!\nHello!";
        let mut it = make_speeches_iter(s);
        assert_eq!(it.next(), Some(vec![
                Event::Text("A> Hello!".into()),
                Event::SoftBreak,
                Event::Text("Hello!".into()),
                Event::SoftBreak,
                Event::Text("Hello!".into()),
        ]));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn speeches_iter_with_two_speeches() {
        let s = r#"A> Hello!
B> Hi!"#;
        let mut it = make_speeches_iter(s);
        assert_eq!(it.next(), Some(vec![
                Event::Text("A> Hello!".into()),
                Event::SoftBreak,
        ]));
        assert_eq!(it.next(), Some(vec![
                Event::Text("B> Hi!".into()),
        ]));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn speeches_iter_with_multi_line_three_speeches() {
        let s = r#"A> Hello!
How are you?
B> Hi! (running)
I'm fine.
And you?
A> *Good!*"#;
        let mut it = make_speeches_iter(s);
        assert_eq!(it.next(), Some(vec![
                Event::Text("A> Hello!".into()), Event::SoftBreak,
                Event::Text("How are you?".into()), Event::SoftBreak,
        ]));
        assert_eq!(it.next(), Some(vec![
                Event::Text("B> Hi! (running)".into()), Event::SoftBreak,
                Event::Text("I'm fine.".into()), Event::SoftBreak,
                Event::Text("And you?".into()), Event::SoftBreak,
        ]));
        assert_eq!(it.next(), Some(vec![
                Event::Text("A> ".into()),
                Event::Start(Tag::Emphasis),
                Event::Text("Good!".into()),
                Event::End(Tag::Emphasis),
        ]));
        assert_eq!(it.next(), None);
    }
}
