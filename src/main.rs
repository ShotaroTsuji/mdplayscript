use structopt::StructOpt;
use mdplayscript::MdPlayScript;

#[derive(Debug,StructOpt)]
enum Opt {
    #[structopt(name="mdbook-preprocessor")]
    MdBookPreprocessor(MdBookPreprocessor),
}

#[derive(Debug,StructOpt)]
struct MdBookPreprocessor {
    #[structopt(subcommand)]
    command: Option<Command>,
}

#[derive(Debug,StructOpt)]
enum Command {
    Supports {
        #[structopt(long)]
        renderer: String,
    },
}

fn main() {
    let opt = Opt::from_args();

    println!("{:#?}", opt);
}
