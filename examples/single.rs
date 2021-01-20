use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::Read;
use structopt::StructOpt;
use pulldown_cmark::Parser;
use playmd::MdPlay;

const HTML_PRELUDE: &'static str =
r#"<html>
<head>
  <title>Le Mariage de Figaro</title>
  <meta charset="utf-8" />
  <link href="./play.css" rel="stylesheet" />
</head>
<body>
<div class="play">"#;

const HTML_POSTLUDE: &'static str =
r#"</div>
</body>
</html>
"#;


#[derive(Debug,StructOpt)]
struct Opt {
    #[structopt(parse(from_os_str))]
    input: PathBuf,
}

fn read_file<P: AsRef<Path>>(path: P) -> String {
    let mut text = String::new();

    let mut file = File::open(path.as_ref()).unwrap();
    file.read_to_string(&mut text).unwrap();

    text
}

fn convert_play(text: &str) -> String {
    let mut output = String::new();

    let parser = Parser::new(&text);
    let parser = MdPlay::new(parser);
    pulldown_cmark::html::push_html(&mut output, parser);

    output
}

fn main() {
    let opt = Opt::from_args();

    let text = read_file(&opt.input);
    let output = convert_play(&text);

    println!("{}", HTML_PRELUDE);
    println!("{}", output);
    println!("{}", HTML_POSTLUDE);
}
