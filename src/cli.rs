// SPDX-License-Identifier: MIT
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "acervo",
    version,
    about = "Organize movie and TV libraries into Jellyfin's naming convention, and backfill TV episode titles from TVMaze."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Organize TV files into Show Name (year)/Season NN/...")]
    Tv {
        #[arg(long, default_value = ".", help = "Library root (default: current directory)")]
        root: PathBuf,
        #[arg(long, help = "Execute the moves (default is dry run)")]
        apply: bool,
        #[arg(long, help = "Drop episode titles from filenames")]
        minimal: bool,
        #[arg(long = "sub-lang", default_value = "", help = "Language code for subtitle filenames (default: none)")]
        sub_lang: String,
        #[arg(long = "bare-number-episodes", help = "Treat a trailing bare number as an episode number")]
        bare_number_episodes: bool,
    },
    #[command(about = "Organize movie files into Movie Name (year)/...")]
    Movies {
        #[arg(long, default_value = ".", help = "Library root (default: current directory)")]
        root: PathBuf,
        #[arg(long, help = "Execute the moves (default is dry run)")]
        apply: bool,
        #[arg(long = "no-editions", help = "Do not emit {edition-...} tags")]
        no_editions: bool,
        #[arg(long = "sub-lang", default_value = "", help = "Language code for subtitle filenames (default: keep existing)")]
        sub_lang: String,
    },
    #[command(about = "Backfill episode titles (and premiere years) from TVMaze")]
    Titles {
        #[arg(long, default_value = ".", help = "TV library root (default: current directory)")]
        root: PathBuf,
        #[arg(long, help = "Execute the renames (default is dry run)")]
        apply: bool,
        #[arg(long = "multi-ep-first", help = "Use the first episode's title for multi-episode files")]
        multi_ep_first: bool,
        #[arg(long, default_value_t = 0.75, help = "Minimum TVMaze title similarity to accept (default 0.75)")]
        threshold: f64,
        #[arg(long, default_value_t = 15, help = "HTTP timeout in seconds (default 15)")]
        timeout: u64,
    },
}
