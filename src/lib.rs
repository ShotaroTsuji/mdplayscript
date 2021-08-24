#![doc = include_str!("../README.md")]
pub mod parser;
pub mod speech;
pub mod renderer;
pub mod interface;

pub use interface::{MdPlayScript, Options, Params};

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
