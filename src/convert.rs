use std::io::Write;
use pulldown_cmark::{Event, Tag, CowStr};
use pulldown_cmark::escape::{WriteWrapper, StrWrite, escape_html};
use eyre::Result;

pub struct Convert<'a, W: Write> {
    context: Vec<Tag<'a>>,
    w: WriteWrapper<W>,
}

impl<'a, W: Write> Convert<'a, W> {
    pub fn new(w: W) -> Self {
        Convert {
            context: vec![],
            w: WriteWrapper(w),
        }
    }

    pub fn start_heading(&mut self, level: u32) -> Result<()> {
        if level <= 6 {
            write!(self.w, "<h{}>", level)?;
        } else {
            write!(self.w, "<h6>")?;
        }

        Ok(())
    }

    pub fn end_heading(&mut self, level: u32) -> Result<()> {
        if level <= 6 {
            write!(self.w, "</h{}>", level)?;
        } else {
            write!(self.w, "</h6>")?;
        }

        Ok(())
    }

    pub fn start_paragraph(&mut self) -> Result<()> {
        write!(self.w, "<p>")?;

        Ok(())
    }

    pub fn end_paragraph(&mut self) -> Result<()> {
        write!(self.w, "</p>")?;

        Ok(())
    }

    pub fn start_list(&mut self, number: Option<u64>) -> Result<()> {
        match number {
            Some(number) => write!(self.w, "<ol start=\"{}\">", number)?,
            None => write!(self.w, "<ul>")?,
        }

        Ok(())
    }

    pub fn end_list(&mut self, number: Option<u64>) -> Result<()> {
        match number {
            Some(_) => write!(self.w, "</ol>")?,
            None => write!(self.w, "</ul>")?,
        }

        Ok(())
    }

    pub fn start_list_item(&mut self) -> Result<()> {
        write!(self.w, "<li>")?;

        Ok(())
    }

    pub fn end_list_item(&mut self) -> Result<()> {
        write!(self.w, "</li>")?;

        Ok(())
    }

    pub fn start_tag(&mut self, tag: Tag<'a>) -> Result<()> {
        match tag {
            Tag::Heading(level) => self.start_heading(level),
            Tag::Paragraph => self.start_paragraph(),
            Tag::List(number) => self.start_list(number),
            Tag::Item => self.start_list_item(),
            _ => Ok(()),
        }
    }

    pub fn end_tag(&mut self, tag: Tag<'a>) -> Result<()> {
        match tag {
            Tag::Heading(level) => self.end_heading(level),
            Tag::Paragraph => self.end_paragraph(),
            Tag::List(number) => self.end_list(number),
            Tag::Item => self.end_list_item(),
            _ => Ok(()),
        }
    }

    pub fn text(&mut self, text: CowStr<'a>) -> Result<()> {
        escape_html(&mut self.w, &text)?;

        Ok(())
    }

    pub fn code(&mut self, code: CowStr<'a>) -> Result<()> {
        write!(self.w, "<code>")?;
        escape_html(&mut self.w, &code)?;
        write!(self.w, "</code>")?;

        Ok(())
    }

    pub fn softbreak(&mut self) -> Result<()> {
        writeln!(self.w, "")?;

        Ok(())
    }

    pub fn event(&mut self, event: Event<'a>) -> Result<()> {
        eprintln!("// CONTEXT: {:?}", self.context);
        eprintln!("// EVENT  : {:?}", event);
        match event {
            Event::Start(tag) => {
                self.context.push(tag.clone());
                self.start_tag(tag)?;
            },
            Event::End(tag) => {
                match self.context.pop() {
                    Some(x) => assert_eq!(tag, x),
                    None => panic!("Context stack is empty"),
                }
                self.end_tag(tag)?;
                writeln!(self.w, "")?;
            },
            Event::Text(text) => {
                self.text(text)?;
            },
            Event::Code(code) => {
                self.code(code)?;
            },
            Event::SoftBreak => {
                self.softbreak()?;
            },
            _ => {},
        }

        Ok(())
    }
}
