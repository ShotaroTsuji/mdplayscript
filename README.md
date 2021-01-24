# mdplayscript

An extension of Markdown for play scripts

This crate is a parser of an extension of Markdown for play scripts.
It defines an extended grammar of texts in paragraphs.
It is implemented as a filter for `Parser` of pulldown-cmark crate.
The goal of this parser is emit an HTML document.
Thus it is recommended to pass the parser to `pulldown_cmark::html::push_html` or `write_html`.

## Example

### Play script format

A line starts with a string and a right angle denotes a character's speech.
The text before the right angle is the character name and the text after the right angle
is the speech of the character.

```rust
A> Hello!
```

A text between a pair of parentheses in a speech denotes a direction.

```rust
A> Hello! (some direction)
```

A direction can be placed after the character name.
No space is allowed between the right parenthesis and the right angle.

```rust
A (running)> Hello!
```

Other forms of texts are handled as normal paragraphs.

The examples above are converted into the following HTML:

```rust
use pulldown_cmark::Parser;
use pulldown_cmark::html::push_html;
use mdplayscript::MdPlayScript;

fn convert(s: &str) -> String {
    let p = MdPlayScript::new(Parser::new(s));
    let mut buf = String::new();
    push_html(&mut buf, p);
    buf
}

assert_eq!(convert("A> Hello!"),
r#"<div class="speech"><h5><span class="character">A</span></h5>
<p>Hello!</p>
</div>
"#);
assert_eq!(convert("A> Hello! (some direction)"),
r#"<div class="speech"><h5><span class="character">A</span></h5>
<p>Hello!<span class="direction">some direction</span></p>
</div>
"#);
assert_eq!(convert("A (running)> Hello!"),
r#"<div class="speech"><h5><span class="character">A</span><span class="direction">running</span></h5>
<p>Hello!</p>
</div>
"#);
```

### CLI program

This crate has no proper CLI program. It only has a tiny example program: `examples/single.rs`.
It converts a single Markdown into an HTML document.
The generated document has a link element which specifies a style sheet `examples/play.css`.
I prepared an example input file: `examples/figaro.md`.
The output file is
[`public/figaro.html`](https://shotarotsuji.github.io/mdplayscript/figaro.html).

For Japanese play scripts, I prepared a style sheet `examples/play_ja.css`.
If you pass `-l ja` option to `examples/single.rs`, it uses the style sheet
`examples/play_ja.css`.
The output file is
[`public/yushima.html`](https://shotarotsuji.github.io/mdplayscript/yushima.html).

## ToDo

- [ ] Make a function like `mdbook build`.
- [ ] Refactor test codes

## License

MIT License


License: MIT
