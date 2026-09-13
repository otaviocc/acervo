// SPDX-License-Identifier: MIT
mod cli;
mod fsops;
mod movies;
mod naming;
mod sim;
mod titles;
mod tokens;
mod tv;
mod tvmaze;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Tv { root, apply, minimal, sub_lang, bare_number_episodes } => {
            tv::run(&tv::Args { root, apply, minimal, sub_lang, bare_number_episodes })
        }
        Command::Movies { root, apply, no_editions, sub_lang } => {
            movies::run(&movies::Args { root, apply, no_editions, sub_lang })
        }
        Command::Titles { root, apply, multi_ep_first, threshold, interactive, timeout } => {
            titles::run(&titles::Args { root, apply, multi_ep_first, threshold, interactive, timeout })
        }
    };
    match code {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
