#![allow(clippy::expect_used)]

use slint_build::CompilerConfiguration;

fn main() {
    let config = CompilerConfiguration::new().with_style("fluent-dark".into());
    slint_build::compile_with_config("ui/main.slint", config)
        .expect("compilation des fichiers Slint");

    #[cfg(windows)]
    {
        let mut resources = winres::WindowsResource::new();
        let icon = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/icons/pdfmerge.ico");
        resources.set_icon(icon.to_string_lossy().as_ref().to_string());
        resources
            .compile()
            .expect("compilation des resources Windows");
    }
}
