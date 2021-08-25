use std::cell::RefCell;
use pulldown_cmark::Event;
use crate::speech::{Speech, Heading, Direction, Inline};

#[derive(Debug)]
pub struct HtmlRenderer {
    pub speech_class: &'static str,
    pub character_class: &'static str,
    pub direction_class: &'static str,
    pub heading_anchor_class: &'static str,
    pub heading_id_counter: RefCell<usize>,
    pub replace_softbreak: Option<String>,
}

impl Default for HtmlRenderer {
    fn default() -> Self {
        Self {
            speech_class: "speech",
            character_class: "character",
            direction_class: "direction",
            heading_anchor_class: "header",
            heading_id_counter: RefCell::new(0),
            replace_softbreak: Some(" ".to_owned()),
        }
    }
}

impl HtmlRenderer {
    pub fn render_speech<'a>(&self, speech: Speech<'a>, events: &mut Vec<Event<'a>>) {
        let div_start = format!("<div class=\"{}\">", self.speech_class);
        let div_end = "</div>";
        
        events.push(Event::Html(div_start.into()));

        self.render_heading(speech.heading, events);
        self.render_body(speech.body, events);

        events.push(Event::Html(div_end.into()));
    }

    pub fn render_heading<'a>(&self, heading: Heading<'a>, events: &mut Vec<Event<'a>>) {
        let mut counter = self.heading_id_counter.borrow_mut();

        let h_start = format!(r#"<h5 id="D{id}">"#,
            id = counter,
        );
        let a_start = format!(r##"<a class="{class}" href="#D{id}">"##,
            class = self.heading_anchor_class,
            id = counter,
        );
        let span_start = format!(r#"<span class="{}">"#, self.character_class);
        let span_end = "</span>";
        let a_end = "</a>";
        let h_end = "</h5>";

        *counter = *counter + 1;

        events.push(Event::Html(h_start.into()));
        events.push(Event::Html(a_start.into()));
        events.push(Event::Html(span_start.into()));
        events.push(Event::Text(heading.character));
        events.push(Event::Html(span_end.into()));
        self.render_direction(heading.direction, false, events);
        events.push(Event::Html(a_end.into()));
        events.push(Event::Html(h_end.into()));
    }

    pub fn render_direction<'a>(&self, direction: Direction<'a>, trim_start: bool, events: &mut Vec<Event<'a>>) {
        let direction = direction.0;
        let len = direction.len();

        if len == 0 {
            return;
        }

        let span_begin = format!("<span class=\"{}\">", self.direction_class);
        let span_end = "</span>";

        events.push(Event::Html(span_begin.into()));

        for (index, inline) in direction.into_iter().enumerate() {
            match inline {
                Event::Text(s) => {
                    let mut s: &str = s.as_ref();
                    if index == 0 && trim_start {
                        s = s.trim_start();
                    }
                    if index + 1 == len {
                        s = s.trim_end();
                    }
                    let s = s.to_owned();
                    events.push(Event::Text(s.into()));
                },
                event => {
                    events.push(event);
                },
            }
        }

        events.push(Event::Html(span_end.into()));
    }

    pub fn render_body<'a>(&self, body: Vec<Inline<'a>>, events: &mut Vec<Event<'a>>) {
        let mut to_be_trimmed_start = false;
        let mut event_count = 0usize;

        let mut body = body;
        replace_softbreaks(&mut body, self.replace_softbreak.as_ref());

        events.push(Event::Html("<p>".into()));

        for inline in body.into_iter() {
            match inline {
                Inline::Event(Event::Text(s)) if to_be_trimmed_start => {
                    if event_count == 0 {
                        events.push(Event::Html("<span>".into()));
                    }

                    let s = s.trim_start().to_owned();
                    events.push(Event::Text(s.into()));
                    to_be_trimmed_start = false;
                    event_count = event_count + 1;
                },
                Inline::Event(event) => {
                    if event_count == 0 {
                        events.push(Event::Html("<span>".into()));
                    }

                    events.push(event);
                    event_count = event_count + 1;
                },
                Inline::Direction(direction) => {
                    trim_end_of_last(events);

                    if event_count > 0 {
                        events.push(Event::Html("</span>".into()));
                    }

                    self.render_direction(direction, true, events);
                    to_be_trimmed_start = true;
                    event_count = 0;
                },
            }
        }

        if event_count > 0 {
            events.push(Event::Html("</span>".into()));
        }

        events.push(Event::Html("</p>".into()));
    }

    pub fn render_events<'a>(&self, events: Vec<Event<'a>>, output: &mut Vec<Event<'a>>) {
        let mut events: Vec<Inline<'a>> = events.into_iter()
            .map(|e| Inline::Event(e))
            .collect();

        replace_softbreaks(&mut events, self.replace_softbreak.as_ref());

        for e in events.into_iter() {
            match e {
                Inline::Event(e) => {
                    output.push(e);
                },
                _ => {},
            }
        }
    }
}

fn trim_end_of_last<'a>(events: &mut Vec<Event<'a>>) {
    match events.pop() {
        Some(Event::Text(s)) => {
            let s = s.trim_end().to_owned();
            events.push(Event::Text(s.into()));
        },
        Some(event) => {
            events.push(event);
        },
        None => {},
    }
}

pub fn replace_softbreaks<'a>(inlines: &mut Vec<Inline<'a>>, s: Option<&String>) {
    if s.is_none() {
        return;
    }
    let s = s.unwrap();

    for inline in inlines.iter_mut() {
        match *inline {
            Inline::Event(Event::SoftBreak) => {
                let s = s.clone();
                *inline = Inline::Event(Event::Text(s.into()));
            },
            _ => {},
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    use pulldown_cmark::Tag;

    #[test]
    fn render_direction_to_html() {
        let input = vec![
            Event::Text(" running ".into()),
        ];
        let expected = vec![
            Event::Html(r#"<span class="direction">"#.into()),
            Event::Text("running".into()),
            Event::Html("</span>".into()),
        ];
        let mut result = Vec::new();
        HtmlRenderer::default().render_direction(Direction(input), true, &mut result);
        assert_eq!(result, expected);
    }

    #[test]
    fn render_direction_with_em_to_html() {
        let input = vec![
            Event::Text(" aaa ".into()),
            Event::Start(Tag::Emphasis),
            Event::Text("bbb".into()),
            Event::End(Tag::Emphasis),
            Event::Text(" ccc ".into()),
        ];
        let expected = vec![
            Event::Html(r#"<span class="direction">"#.into()),
            Event::Text("aaa ".into()),
            Event::Start(Tag::Emphasis),
            Event::Text("bbb".into()),
            Event::End(Tag::Emphasis),
            Event::Text(" ccc".into()),
            Event::Html("</span>".into()),
        ];
        let mut result = Vec::new();
        HtmlRenderer::default().render_direction(Direction(input), true, &mut result);
        assert_eq!(result, expected);
    }

    #[test]
    fn render_heading_of_only_character_to_html() {
        let input = Heading {
            character: "A".into(),
            direction: Direction::new(),
        };
        let expected = vec![
            Event::Html(r#"<h5 id="D0">"#.into()),
            Event::Html(r##"<a class="header" href="#D0">"##.into()),
            Event::Html(r#"<span class="character">"#.into()),
            Event::Text("A".into()),
            Event::Html("</span>".into()),
            Event::Html("</a>".into()),
            Event::Html("</h5>".into()),
        ];
        let mut result = Vec::new();
        HtmlRenderer::default().render_heading(input, &mut result);
        assert_eq!(result, expected);
    }

    #[test]
    fn render_heading_with_direction() {
        let input = Heading {
            character: "A".into(),
            direction: Direction(vec![Event::Text("running".into())]),
        };
        let expected = vec![
            Event::Html(r#"<h5 id="D0">"#.into()),
            Event::Html(r##"<a class="header" href="#D0">"##.into()),
            Event::Html(r#"<span class="character">"#.into()),
            Event::Text("A".into()),
            Event::Html("</span>".into()),
            Event::Html(r#"<span class="direction">"#.into()),
            Event::Text("running".into()),
            Event::Html("</span>".into()),
            Event::Html("</a>".into()),
            Event::Html("</h5>".into()),
        ];
        let mut result = Vec::new();
        HtmlRenderer::default().render_heading(input, &mut result);
        assert_eq!(result, expected);
    }

    #[test]
    fn render_body_to_html() {
        let input = vec![
            Inline::Event(Event::Text("Hello! ".into())),
            Inline::Direction(Direction(vec![Event::Text("run".into())])),
            Inline::Event(Event::Text(" Hello!".into())),
        ];
        let expected = vec![
            Event::Html("<p>".into()),
            Event::Html("<span>".into()),
            Event::Text("Hello!".into()),
            Event::Html("</span>".into()),
            Event::Html(r#"<span class="direction">"#.into()),
            Event::Text("run".into()),
            Event::Html("</span>".into()),
            Event::Html("<span>".into()),
            Event::Text("Hello!".into()),
            Event::Html("</span>".into()),
            Event::Html("</p>".into()),
        ];
        let mut result = Vec::new();
        HtmlRenderer::default().render_body(input, &mut result);
        assert_eq!(result, expected);
    }

    #[test]
    fn render_body_with_softbreaks_to_html() {
        let input = vec![
            Inline::Event(Event::Text("Hello!".into())),
            Inline::Event(Event::SoftBreak),
            Inline::Event(Event::Text("Hello!".into())),
            Inline::Event(Event::SoftBreak),
            Inline::Event(Event::Text("Hello!".into())),
        ];
        let expected = vec![
            Event::Html("<p>".into()),
            Event::Html("<span>".into()),
            Event::Text("Hello!".into()),
            Event::Text(" ".into()),
            Event::Text("Hello!".into()),
            Event::Text(" ".into()),
            Event::Text("Hello!".into()),
            Event::Html("</span>".into()),
            Event::Html("</p>".into()),
        ];
        let mut result = Vec::new();
        HtmlRenderer::default().render_body(input, &mut result);
        assert_eq!(result, expected);
    }

    #[test]
    fn render_body_with_softbreaks_and_direction_to_html() {
        let input = vec![
            Inline::Event(Event::Text("Hello!".into())),
            Inline::Event(Event::SoftBreak),
            Inline::Direction(Direction(vec![Event::Text("running".into())])),
            Inline::Event(Event::SoftBreak),
            Inline::Event(Event::Text("Hello!".into())),
        ];
        let expected = vec![
            Event::Html("<p>".into()),
            Event::Html("<span>".into()),
            Event::Text("Hello!".into()),
            Event::Text("".into()),
            Event::Html("</span>".into()),
            Event::Html("<span class=\"direction\">".into()),
            Event::Text("running".into()),
            Event::Html("</span>".into()),
            Event::Html("<span>".into()),
            Event::Text("".into()),
            Event::Text("Hello!".into()),
            Event::Html("</span>".into()),
            Event::Html("</p>".into()),
        ];
        let mut result = Vec::new();
        HtmlRenderer::default().render_body(input, &mut result);
        assert_eq!(result, expected);
    }
}
