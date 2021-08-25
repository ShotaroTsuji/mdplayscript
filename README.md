# mdplayscript

An extension of Markdown for play scripts

This crate is a parser of an extension of Markdown for stage play scripts.
It defines an extended grammar of texts in paragraphs.
It is implemented as a filter for `struct Parser` of pulldown-cmark crate.
The goal of this parser is to emit an HTML document.
Thus it is recommended to pass the parser to `pulldown_cmark::html::push_html` or `write_html`.

This crate has an implementation of mdbook preprocessor:
[mdbook-playscript](https://github.com/ShotaroTsuji/mdbook-playscript).

## Example

### Play Script Format

A line starts with a string and a right angle denotes a character's speech.
The text before the right angle is the character name and the text after the right angle
is the speech of the character.

```ignore
A> Hello!
```

A text between a pair of parentheses in a speech is the content of a direction.

```ignore
A> Hello! (some direction)
```

A direction can be placed after the character name.
No space is allowed between the right parenthesis and the right angle.

```ignore
A (running)> Hello!
```

### Directives

Directives are written as HTML comments.
There are four directives:
- playscript-on
- playscript-off
- playscript-monologue-begin
- playscript-monologue-end

`<!-- playscript-on -->` and `<!-- playscript-off -->` switch the parser on and off
respectively.

Monologues are surrounded by the directives: `<!-- playscript-monologue-begin -->`
and `<!-- playscript-monologue-end -->`.
The texts surrounded by the monologue directives are styled in the normal font style and the
directions between the directives are styled in italic.

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
r##"<div class="speech"><h5 id="A-0"><a class="header" href="#A-0"><span class="character">A</span></a></h5><p><span>Hello!</span></p></div>
"##);
assert_eq!(convert("A> Hello! (some direction)"),
r##"<div class="speech"><h5 id="A-0"><a class="header" href="#A-0"><span class="character">A</span></a></h5><p><span>Hello!</span><span class="direction">some direction</span></p></div>
"##);
assert_eq!(convert("A (running)> Hello!"),
r##"<div class="speech"><h5 id="A-0"><a class="header" href="#A-0"><span class="character">A</span><span class="direction">running</span></a></h5><p><span>Hello!</span></p></div>
"##);
assert_eq!(convert(r#"<!-- playscript-monologue-begin -->
Monologue
(direction)
<!-- playscript-monologue-end -->
"#),
r#"<!-- playscript-monologue-begin -->
<div class="speech"><p><span>Monologue</span><span class="direction">direction</span></p></div><!-- playscript-monologue-end -->
"#);
```

### CLI Program

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

### Test Code

`tests/generate_examples.rs` converts example markdown files located in `examples` directory into HTML files created in the same directory.

## ToDo

- [ ] Refactor test codes

## License

This crate is licensed under MIT License except the following files:
- `examples/figaro.md`: CC-BY-SA 3.0,
- `examples/yushima.md`: Copyleft.
