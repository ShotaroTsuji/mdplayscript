use playmd::MdPlay;
use pulldown_cmark::Parser;

const SAMPLE: &'static str = r#"
# Heading A
This is a sentence of English.

日本語の段落

Dottore> Blah, blah, blah!
Ha, ha, ha!
Arlecchino> Che dice? Cosa sento?

Dottore> Eh? Cosa sento? (*Laughing* at Arlecchino.)

## Heading B {#section_b}

>> Double right angles

This is an example of list.

- List Item
  1. A
  2. B
- Second Item
  * C
  * D

A rust code of `fn function`.

```rust
fn function() -> usize {
    3 + 4
}
```

    const func = (x) => {
        x * x
    };

Broken *emphasize

<div>

Text in `div` element.

***Three astrisks.***

^text surrounded by carets^ and ^again^

</div>
"#;

/*
fn convert_original(s: &str) {
    println!("{}", s);
    let mut buf = String::new();
    let parser = Parser::new(s);
    pulldown_cmark::html::push_html(&mut buf, parser);
    println!("{}", buf);
}

fn convert_to_html(s: &str) {
    println!("{}", s);
    let mut buf = String::new();
    let parser = Parser::new(s);
    let parser = PlayMd::new(parser);
    pulldown_cmark::html::push_html(&mut buf, parser);
    println!("{}", buf);
}
*/

fn main() -> eyre::Result<()> {
    let parser = Parser::new(&SAMPLE);
    for event in parser.take(20) {
        println!("{:?}", event);
    }
    println!("--");

    let parser = Parser::new(&SAMPLE);
    let parser = MdPlay::new(parser, || Parser::new(""));

    let mut buf = String::new();
    pulldown_cmark::html::push_html(&mut buf, parser);
    println!("{}", buf);

    Ok(())
}
