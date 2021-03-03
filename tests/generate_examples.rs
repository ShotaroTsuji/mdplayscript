use toml::Value;
use std::process::Command;
use std::fs::File;

struct SinglePageExample<'a> {
    input: &'a str,
    output: &'a str,
    title: &'a str,
    authors: Vec<&'a str>,
    lang: &'a str,
}

impl<'a> SinglePageExample<'a> {
    fn run(&self) {
        assert_ne!(self.input, self.output);
        assert_ne!(self.input.len(), 0);
        assert_ne!(self.output.len(), 0);

        let mut cmd = Command::new("cargo");
        cmd.env("RUST_LOG", "info");
        cmd.args(&["run", "--example", "single", "--"]);
        if !self.authors.is_empty() {
            cmd.arg("--authors");
            for author in self.authors.iter() {
                cmd.arg(author);
            }
        }
        if !self.lang.is_empty() {
            cmd.arg("-l").arg(self.lang);
        }
        if self.title.len() > 0 {
            cmd.arg("-t").arg(self.title);
        }
        let status = cmd.arg(self.input)
            .stdout(File::create(self.output).unwrap())
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
        assert!(status.success());
    }
}

#[test]
fn generate_examples() {
    let cargo_toml = std::fs::read_to_string("Cargo.toml").unwrap();
    let cargo_toml: Value = toml::from_str(&cargo_toml).unwrap();
    let name = cargo_toml.get("package").unwrap()
        .get("name").unwrap()
        .as_str().unwrap();
    assert_eq!(name, "mdplayscript");

    SinglePageExample {
        input: "examples/figaro.md",
        output: "public/figaro.html",
        title: "Le Mariage de Figaro",
        authors: vec!["Beaumarchais"],
        lang: "",
    }.run();

    SinglePageExample {
        input: "examples/yushima.md",
        output: "public/yushima.html",
        title: "湯島の境内",
        authors: vec!["泉鏡花"],
        lang: "ja",
    }.run();
}
