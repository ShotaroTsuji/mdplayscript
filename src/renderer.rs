use pulldown_cmark::Event;
use crate::speech::{Speech, Heading, Direction, Inline};

#[derive(Debug)]
pub struct HtmlRenderer {
    pub speech_class: &'static str,
    pub character_class: &'static str,
    pub direction_class: &'static str,
    pub replace_softbreak: Option<&'static str>,
}

impl Default for HtmlRenderer {
    fn default() -> Self {
        Self {
            speech_class: "speech",
            character_class: "character",
            direction_class: "direction",
            replace_softbreak: Some(" "),
        }
    }
}

impl HtmlRenderer {
    pub fn render_direction<'a>(&self, direction: Direction<'a>) -> Vec<Event<'a>> {
        let direction = direction.0;
        let len = direction.len();
        let mut events = Vec::with_capacity(len+2);
        let span_begin = format!("<span class=\"{}\">", self.direction_class);
        let span_end = "</span>";
        events.push(Event::Html(span_begin.into()));

        for (index, inline) in direction.into_iter().enumerate() {
            match inline {
                Event::Text(s) => {
                    let mut s: &str = s.as_ref();
                    if index == 0 {
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

        events
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pulldown_cmark::Tag;

    #[test]
    fn render_direction_to_html() {
        let input = vec![
            Event::Text(" running ".into()),
        ];
        let output = vec![
            Event::Html(r#"<span class="direction">"#.into()),
            Event::Text("running".into()),
            Event::Html("</span>".into()),
        ];
        let r = HtmlRenderer::default();
        assert_eq!(r.render_direction(Direction(input)), output);
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
        let output = vec![
            Event::Html(r#"<span class="direction">"#.into()),
            Event::Text("aaa ".into()),
            Event::Start(Tag::Emphasis),
            Event::Text("bbb".into()),
            Event::End(Tag::Emphasis),
            Event::Text(" ccc".into()),
            Event::Html("</span>".into()),
        ];
        let r = HtmlRenderer::default();
        assert_eq!(r.render_direction(Direction(input)), output);
    }
}
