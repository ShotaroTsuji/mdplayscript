use std::marker::PhantomData;
use pulldown_cmark::{Event, Tag};
use pulldown_cmark::escape::{StrWrite, escape_html, escape_href};
use pulldown_cmark::html::push_html;

fn find_custom_id(s: &str) -> (&str, Option<&str>) {
    let (before_brace, after_brace) = match s.find("{#") {
        Some(pos) => (&s[..pos], &s[pos+2..]),
        None => return (s, None),
    };

    let (inner_brace, _after_brace) = match after_brace.find('}') {
        Some(pos) => (&after_brace[..pos], &after_brace[pos+1..]),
        None => return (s, None),
    };

    (before_brace.trim_end(), Some(inner_brace))
}

pub struct MdFilter<'a, P> {
    parser: P,
    _marker: PhantomData<&'a P>,
}

impl<'a, P> MdFilter<'a, P>
where
    P: Iterator<Item=Event<'a>>,
{
    pub fn new(parser: P) -> Self {
        MdFilter {
            parser: parser,
            _marker: PhantomData,
        }
    }

    fn convert_heading(&mut self, level: u32) -> Event<'a> {
        // Read events until the end of heading comes.
        let mut buffer = Vec::new();

        while let Some(event) = self.parser.next() {
            match event {
                Event::End(Tag::Heading(n)) if n == level => break,
                _ => {},
            }
            buffer.push(event.clone());
        }

        // Convert the events into an HTML
        let mut html = String::new();
        let mut start_tag = String::new();

        if let Some((last, events)) = buffer.split_last() {
            push_html(&mut html, events.iter().cloned());

            match last {
                Event::Text(text) => {
                    let (text, id) = find_custom_id(text);
                    escape_html(&mut html, text).unwrap();

                    if let Some(id) = id {
                        write!(&mut start_tag, "<h{} id=\"", level).unwrap();
                        escape_href(&mut start_tag, id).unwrap();
                        write!(&mut start_tag, "\">").unwrap();
                    } else {
                        write!(&mut start_tag, "<h{}>", level).unwrap();
                    }
                },
                event => {
                    push_html(&mut html, vec![event.clone()].into_iter());
                },
            }
        } else {
            write!(&mut start_tag, "<h{}>", level).unwrap();
        }

        writeln!(&mut html, "</h{}>", level).unwrap();

        start_tag += &html;
        let html = start_tag;
        
        Event::Html(html.into())
    }
}

impl<'a, P> Iterator for MdFilter<'a, P>
where
    P: Iterator<Item=Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.parser.next() {
            Some(Event::Start(Tag::Heading(level))) => Some(self.convert_heading(level)),
            Some(event) => Some(event),
            None => None,
        }
    }
}
