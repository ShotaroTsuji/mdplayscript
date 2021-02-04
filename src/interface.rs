use std::collections::VecDeque;
use pulldown_cmark::{Event, Tag};
use crate::parser::{FuseOnParagraphEnd, Speeches};
use crate::speech::parse_speech;
use crate::renderer::HtmlRenderer;

#[derive(Debug)]
pub struct MdPlayScript<'a, I> {
    iter: Option<I>,
    queue: VecDeque<Event<'a>>,
}

impl<'a, I> MdPlayScript<'a, I>
where
    I: Iterator<Item=Event<'a>>,
{
    pub fn new(iter: I) -> Self {
        Self {
            iter: Some(iter),
            queue: VecDeque::new(),
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
            Some(Event::Start(Tag::Paragraph)) => {
                let mut speeches = Speeches::new(FuseOnParagraphEnd::new(iter));

                while let Some(speech) = speeches.next() {
                    let output = match parse_speech(speech) {
                        Ok(speech) => {
                            let r = HtmlRenderer::default();
                            let mut html = Vec::new();
                            r.render_speech(speech, &mut html);

                            html
                        },
                        Err(para) => {
                            wrap_by_paragraph_tag(para)
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
}
