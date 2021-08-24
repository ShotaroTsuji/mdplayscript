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

#[derive(Debug,Clone)]
pub struct Options {
    replace_softbreaks_with: Option<String>,
    disabled_in_default: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            replace_softbreaks_with: Some(" ".to_owned()),
            disabled_in_default: false,
        }
    }
}

impl Options {
    pub fn default_ja() -> Self {
        Self {
            replace_softbreaks_with: Some("".to_owned()),
            disabled_in_default: false,
        }
    }
}

#[derive(Debug,Default,Clone)]
pub struct Params {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
}

pub struct MdPlayScriptBuilder {
    options: Option<Options>,
    params: Option<Params>,
    make_title: Option<Box<dyn FnMut(&Params) -> String>>,
}

impl MdPlayScriptBuilder {
    pub fn new() -> Self {
        Self {
            options: None,
            params: None,
            make_title: None,
        }
    }

    pub fn options(self, opt: Options) -> Self {
        Self {
            options: Some(opt),
            ..self
        }
    }

    pub fn params(self, p: Params) -> Self {
        Self {
            params: Some(p),
            ..self
        }
    }

    pub fn make_title(self, val: Box<dyn FnMut(&Params) -> String>) -> Self {
        Self {
            make_title: Some(val),
            ..self
        }
    }

    pub fn build<'a, I>(self, iter: I) -> MdPlayScript<'a, I>
        where
            I: Iterator<Item=Event<'a>>,
    {
        let options = self.options.unwrap();
        let renderer = HtmlRenderer {
            replace_softbreak: options.replace_softbreaks_with,
            ..Default::default()
        };
        let mode = if options.disabled_in_default {
            Mode::Nop
        } else {
            Mode::PlayScript
        };

        MdPlayScript {
            iter: Some(iter),
            queue: VecDeque::new(),
            mode: mode,
            params: self.params.unwrap_or(Params::default()),
            renderer: renderer,
            make_title: self.make_title,
        }
    }
}

pub struct MdPlayScript<'a, I> {
    iter: Option<I>,
    queue: VecDeque<Event<'a>>,
    mode: Mode,
    params: Params,
    renderer: HtmlRenderer,
    make_title: Option<Box<dyn FnMut(&Params) -> String>>,
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
            renderer: Default::default(),
            make_title: None,
        }
    }

    pub fn into_inner(self) -> I {
        self.iter.unwrap()
    }

    fn dispatch_directive(&mut self, s: &str) {
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
            Some(Directive::SubTitle) => {
                emit_subtitle(&self.params, &mut self.queue);
            },
            Some(Directive::Authors) => {
                emit_authors(&self.params, &mut self.queue);
            },
            Some(Directive::MakeTitle) => {
                if let Some(make_title) = self.make_title.as_mut() {
                    let cover = (make_title)(&self.params);
                    self.queue.push_back(Event::Html(cover.into()));
                }
            },
            None => {},
        }
    }

    fn append_events(&mut self, events: Vec<Event<'a>>) {
        for ev in events.into_iter() {
            self.queue.push_back(ev);
        }
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
                self.dispatch_directive(&s);
                self.queue.push_back(Event::Html(s));
            },
            Some(Event::Start(Tag::Paragraph)) if !self.mode.is_off() => {
                let mut speeches = Speeches::new(FuseOnParagraphEnd::new(iter));

                while let Some(speech) = speeches.next() {
                    let output = match parse_speech(speech) {
                        Ok(speech) => {
                            let mut html = Vec::new();
                            self.renderer.render_speech(speech, &mut html);
                            html.push(Event::SoftBreak);

                            html
                        },
                        Err(para) => {
                            if self.mode.is_monologue() {
                                let monologue = parse_body(para);
                                let mut html = Vec::new();
                                self.renderer.render_body(monologue, &mut html);
                                wrap_by_div_speech(html)
                            } else {
                                let mut output = Vec::new();
                                self.renderer.render_events(para, &mut output);
                                wrap_by_paragraph_tag(output)
                            }
                        },
                    };
                    self.append_events(output);
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

fn wrap_events_by<'a, 'b: 'a>(mut events: Vec<Event<'a>>, start: Event<'b>, end: Event<'b>) -> Vec<Event<'a>> {
    let mut output = Vec::with_capacity(events.len() + 2);

    output.push(start);
    output.append(&mut events);
    output.push(end);

    output
}

fn wrap_by_paragraph_tag<'a>(events: Vec<Event<'a>>) -> Vec<Event<'a>> {
    wrap_events_by(
        events,
        Event::Start(Tag::Paragraph),
        Event::End(Tag::Paragraph),
    )
}

fn wrap_by_div_speech<'a>(events: Vec<Event<'a>>) -> Vec<Event<'a>> {
    wrap_events_by(
        events,
        Event::Html("<div class=\"speech\">".into()),
        Event::Html("</div>".into()),
    )
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
    MakeTitle,
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
        "playscript-make-title" => Some(Directive::MakeTitle),
        _ => None,
    }
}

fn emit_title<'a>(params: &Params, queue: &mut VecDeque<Event<'a>>) {
    let p_start = "<h1 class=\"cover-title\">";
    let p_end = "</h1>";

    if let Some(content) = params.title.as_ref().cloned() {
        queue.push_back(Event::Html(p_start.into()));
        queue.push_back(Event::Text(content.into()));
        queue.push_back(Event::Html(p_end.into()));
    }
}

fn emit_subtitle<'a>(params: &Params, queue: &mut VecDeque<Event<'a>>) {
    let p_start = "<h2 class=\"cover-title\">";
    let p_end = "</h2>";

    if let Some(content) = params.subtitle.as_ref().cloned() {
        queue.push_back(Event::Html(p_start.into()));
        queue.push_back(Event::Text(content.into()));
        queue.push_back(Event::Html(p_end.into()));
    }
}

fn emit_authors<'a>(params: &Params, queue: &mut VecDeque<Event<'a>>) {
    let div_start = "<div class=\"authors\">";
    let div_end = "</div>";
    let p_start = "<p class=\"cover-author\">";
    let p_end = "</p>";

    if params.authors.is_empty() {
        return;
    }

    queue.push_back(Event::Html(div_start.into()));

    for author in params.authors.iter().cloned() {
        queue.push_back(Event::Html(p_start.into()));
        queue.push_back(Event::Text(author.into()));
        queue.push_back(Event::Html(p_end.into()));
    }

    queue.push_back(Event::Html(div_end.into()));
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
