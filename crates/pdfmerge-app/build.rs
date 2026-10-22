#![allow(clippy::expect_used)]

use slint_build::CompilerConfiguration;

fn main() {
    let config = CompilerConfiguration::new().with_style("fluent-dark".into());
    slint_build::compile_with_config("ui/main.slint", config)
        .expect("compilation des fichiers Slint");
}
