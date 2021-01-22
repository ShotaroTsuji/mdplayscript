use toml::Value;
use std::process::Command;
use std::fs::File;

struct SinglePageExample<'a> {
    input: &'a str,
    output: &'a str,
    title: &'a str,
    lang: &'a str,
}

impl<'a> SinglePageExample<'a> {
    fn run(&self) {
        assert_ne!(self.input, self.output);
        assert_ne!(self.input.len(), 0);
        assert_ne!(self.output.len(), 0);

        let mut cmd = Command::new("cargo");
        cmd.args(&["run", "--example", "single", "--"]);
        if self.title.len() > 0 {
            cmd.arg("-t").arg(self.title);
        }
        if self.lang.len() > 0 {
            cmd.arg("-l").arg(self.lang);
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
    assert_eq!(name, "mdplay");

    SinglePageExample {
        input: "examples/figaro.md",
        output: "examples/figaro.html",
        title: "Le Mariage de Figaro",
        lang: "",
    }.run();

    SinglePageExample {
        input: "examples/yushima.md",
        output: "examples/yushima.html",
        title: "湯島の境内 - 泉鏡花",
        lang: "ja",
    }.run();
}
