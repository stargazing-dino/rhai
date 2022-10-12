use std::{env, fs::File, io::Write};

const WRITE_ERROR: &str = "cannot write to `config.rs`";

fn main() {
    // Tell Cargo that if the given environment variable changes, to rerun this build script.
    println!("cargo:rerun-if-env-changed=RHAI_AHASH_SEED");

    let mut f = File::create("src/config.rs").expect("cannot create `config.rs`");

    f.write_fmt(format_args!(
        "//! Configuration settings for this Rhai build

"
    ))
    .expect(WRITE_ERROR);

    let seed = env::var("RHAI_AHASH_SEED").map_or_else(|_| "None".into(), |s| format!("Some({s})"));

    f.write_fmt(format_args!(
        "pub const AHASH_SEED: Option<[u64; 4]> = {seed};\n"
    ))
    .expect(WRITE_ERROR);

    f.flush().expect("cannot flush `config.rs`");
}
