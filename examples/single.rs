use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::Read;
use structopt::StructOpt;
use pulldown_cmark::Parser;
use mdplayscript::interface::{MdPlayScriptBuilder, Options};

fn html_prelude(title: &str, lang: &str) -> String {
    let cssfile = if lang == "ja" {
        "play_ja.css"
    } else {
        "play.css"
    };

    format!(
r#"<html>
<head>
  <title>{title}</title>
  <meta charset="utf-8" />
  <link href="./{cssfile}" rel="stylesheet" />
</head>
<body>
<div class="play">"#,
    title=title,
    cssfile=cssfile
    )
}

const HTML_POSTLUDE: &'static str =
r#"</div>
</body>
</html>
"#;


#[derive(Debug,StructOpt)]
struct Opt {
    #[structopt(long,short,default_value="Example of mdPlay")]
    title: String,
    #[structopt(long,short,default_value="")]
    language: String,
    #[structopt(parse(from_os_str))]
    input: PathBuf,
}

fn read_file<P: AsRef<Path>>(path: P) -> String {
    let mut text = String::new();

    let mut file = File::open(path.as_ref()).unwrap();
    file.read_to_string(&mut text).unwrap();

    text
}

fn convert_play(text: &str, lang: &str) -> String {
    let mut output = String::new();

    let parser = Parser::new(&text);

    let options = if lang == "ja" {
        Options::default_ja()
    } else {
        Options::default()
    };

    let parser = MdPlayScriptBuilder::new()
        .options(options)
        .build(parser);
    pulldown_cmark::html::push_html(&mut output, parser);

    output
}

fn main() {
    let opt = Opt::from_args();

    let text = read_file(&opt.input);
    let output = convert_play(&text, &opt.language);

    println!("{}", html_prelude(&opt.title, &opt.language));
    println!("{}", output);
    println!("{}", HTML_POSTLUDE);
}
