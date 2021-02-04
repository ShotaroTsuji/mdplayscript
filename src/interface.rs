use std::collections::VecDeque;
use pulldown_cmark::{Event, Tag};
use crate::parser::{FuseOnParagraphEnd, Speeches};
use crate::speech::{parse_speech, parse_body};
use crate::renderer::HtmlRenderer;

#[derive(Debug)]
enum Mode {
    Nop,
    PlayScript,
    Monologue,
}

impl Mode {
    fn is_off(&self) -> bool {
        match self {
            Mode::Nop => true,
            _ => false,
        }
    }

    fn is_monologue(&self) -> bool {
        match self {
            Mode::Monologue => true,
            _ => false,
        }
    }
}

#[derive(Debug,Default)]
struct Params {
    title: Option<String>,
    subtitle: Option<String>,
    authors: Vec<String>,
}

#[derive(Debug)]
pub struct MdPlayScript<'a, I> {
    iter: Option<I>,
    queue: VecDeque<Event<'a>>,
    mode: Mode,
    params: Params,
}

impl<'a, I> MdPlayScript<'a, I>
where
    I: Iterator<Item=Event<'a>>,
{
    pub fn new(iter: I) -> Self {
        Self {
            iter: Some(iter),
            queue: VecDeque::new(),
            mode: Mode::PlayScript,
            params: Default::default(),
        }
    }

    pub fn into_inner(self) -> I {
        self.iter.unwrap()
    }
}

impl<'a, I: 'a> Iterator for MdPlayScript<'a, I>
where
    I: Iterator<Item=Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(event) = self.queue.pop_front() {
            return Some(event);
        }

        let mut iter = self.iter.take().unwrap();

        match iter.next() {
            Some(Event::Html(s)) => {
                eprintln!("HTML: {:?} -> {:?}", s, parse_directive(&s));
                match parse_directive(&s) {
                    Some(Directive::MonologueBegin) => {
                        self.mode = Mode::Monologue;
                    },
                    Some(Directive::MonologueEnd) => {
                        self.mode = Mode::PlayScript;
                    },
                    Some(Directive::PlayScriptOn) => {
                        self.mode = Mode::PlayScript;
                    },
                    Some(Directive::PlayScriptOff) => {
                        self.mode = Mode::Nop;
                    },
                    Some(Directive::Title) => {
                        emit_title(&self.params, &mut self.queue);
                    },
                    _ => {},
                }

                self.queue.push_back(Event::Html(s));
            },
            Some(Event::Start(Tag::Paragraph)) if !self.mode.is_off() => {
                let mut speeches = Speeches::new(FuseOnParagraphEnd::new(iter));
                eprintln!("START PARAGRAPH (MODE: {:?})", self.mode);

                while let Some(speech) = speeches.next() {
                    let output = match parse_speech(speech) {
                        Ok(speech) => {
                            let r = HtmlRenderer::default();
                            let mut html = Vec::new();
                            r.render_speech(speech, &mut html);

                            html
                        },
                        Err(para) => {
                            if self.mode.is_monologue() {
                                let monologue = parse_body(para);
                                let r = HtmlRenderer::default();
                                let mut html = Vec::new();
                                r.render_body(monologue, &mut html);
                                wrap_by_div_speech(html)
                            } else {
                                wrap_by_paragraph_tag(para)
                            }
                        },
                    };
                    for ev in output.into_iter() {
                        self.queue.push_back(ev);
                    }
                }

                iter = speeches.into_inner().into_inner();
            },
            Some(event) => {
                self.queue.push_back(event);
            },
            None => {},
        }

        self.iter.replace(iter);

        self.queue.pop_front()
    }
}

fn wrap_by_paragraph_tag<'a>(mut events: Vec<Event<'a>>) -> Vec<Event<'a>> {
    let mut output = Vec::with_capacity(events.len() + 2);

    output.push(Event::Start(Tag::Paragraph));
    output.append(&mut events);
    output.push(Event::End(Tag::Paragraph));

    output
}

fn wrap_by_div_speech<'a>(mut events: Vec<Event<'a>>) -> Vec<Event<'a>> {
    let mut output = Vec::with_capacity(events.len() + 2);

    let start = "<div class=\"speech\">";
    let end = "</div>";

    output.push(Event::Html(start.into()));
    output.append(&mut events);
    output.push(Event::Html(end.into()));

    output
}

#[derive(Debug,Clone,PartialEq)]
enum Directive {
    MonologueBegin,
    MonologueEnd,
    PlayScriptOn,
    PlayScriptOff,
    Title,
    SubTitle,
    Authors,
}

fn parse_directive(s: &str) -> Option<Directive> {
    let s = s.trim()
        .strip_prefix("<!--")?
        .strip_suffix("-->")?
        .trim();

    match s {
        "playscript-monologue-begin" => Some(Directive::MonologueBegin),
        "playscript-monologue-end" => Some(Directive::MonologueEnd),
        "playscript-on" => Some(Directive::PlayScriptOn),
        "playscript-off" => Some(Directive::PlayScriptOff),
        "playscript-title" => Some(Directive::Title),
        "playscript-subtitle" => Some(Directive::SubTitle),
        "playscript-authors" => Some(Directive::Authors),
        _ => None,
    }
}

fn emit_title<'a>(params: &Params, queue: &mut VecDeque<Event<'a>>) {
    let p_start = "<p class=\"cover-title\">";
    let p_end = "</p>";

    if let Some(content) = params.title.as_ref().cloned() {
        queue.push_back(Event::Html(p_start.into()));
        queue.push_back(Event::Text(content.into()));
        queue.push_back(Event::Html(p_end.into()));
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pulldown_cmark::Parser;
    use pulldown_cmark::html::push_html;

    #[test]
    fn consume() {
        let s = r#"A> xxx
\ruby
B> Hello

This is a normal line.

<!-- playscript-monologue-begin -->
Monologue
<!-- playscript-monologue-end -->

A> What?
????
B> !!!!
A> ...."#;
        let parser = MdPlayScript::new(Parser::new(s));

        for e in parser {
            eprintln!("{:?}", e);
        }

        let mut buf = String::new();
        let parser = MdPlayScript::new(Parser::new(s));

        push_html(&mut buf, parser);

        eprintln!("{}", buf);
    }

    #[test]
    fn parse_correct_directives() {
        assert_eq!(
            parse_directive("<!-- playscript-monologue-begin -->"),
            Some(Directive::MonologueBegin));
        assert_eq!(
            parse_directive("<!-- playscript-monologue-end -->"),
            Some(Directive::MonologueEnd));
        assert_eq!(
            parse_directive("<!-- playscript-on -->"),
            Some(Directive::PlayScriptOn));
        assert_eq!(
            parse_directive("<!-- playscript-off -->"),
            Some(Directive::PlayScriptOff));
        assert_eq!(
            parse_directive("<!-- playscript-title -->"),
            Some(Directive::Title));
        assert_eq!(
            parse_directive("<!-- playscript-subtitle -->"),
            Some(Directive::SubTitle));
        assert_eq!(
            parse_directive("<!-- playscript-authors -->"),
            Some(Directive::Authors));
    }
}
