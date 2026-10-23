#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process;

use clap::Parser;

slint::include_modules!();

mod cli;
mod gui;

fn main() {
    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let code = if error.kind() == clap::error::ErrorKind::DisplayHelp
                || error.kind() == clap::error::ErrorKind::DisplayVersion
            {
                println!("{error}");
                0
            } else {
                eprintln!("{error}");
                1
            };
            process::exit(code);
        }
    };

    if !cli.gui
        && cli.inputs.is_empty()
        && cli.output.is_none()
        && cli.output_dir.is_none()
        && cli.name.is_none()
    {
        gui::run();
        return;
    }

    process::exit(cli::run(cli));
}
