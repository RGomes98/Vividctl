use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Entry {
    vendor:   String,
    product:  String,
    serial:   String,
    vibrance: i16,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Config {
    version: u32,
    entries: Vec<Entry>,
}

fn load(text: &str) -> Result<Config, toml::de::Error> {
    toml::from_str(text)
}

fn main() {
    println!(
        "{:?}",
        load(concat!(
            "version = 1\n",
            "[[entries]]\n",
            "vendor = \"PHL\"\n",
            "product = \"PHL 242V8\"\n",
            "serial = \"0x00008fac\"\n",
            "vibrance = 512\n"
        ))
    );
}
