use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Organize movie and TV libraries into Jellyfin's naming convention, and
/// backfill TV episode titles from TVMaze.
#[derive(Parser)]
#[command(name = "acervo", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Organize TV files into Show Name (year)/Season NN/...
    Tv {
        /// Library root (default: current directory)
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Execute the moves (default is dry run)
        #[arg(long)]
        apply: bool,
        /// Drop episode titles from filenames
        #[arg(long)]
        minimal: bool,
        /// Language code for subtitle filenames (default: none)
        #[arg(long = "sub-lang", default_value = "")]
        sub_lang: String,
        /// Treat a trailing bare number as an episode number
        #[arg(long = "bare-number-episodes")]
        bare_number_episodes: bool,
    },
    /// Organize movie files into Movie Name (year)/...
    Movies {
        /// Library root (default: current directory)
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Execute the moves (default is dry run)
        #[arg(long)]
        apply: bool,
        /// Do not emit {edition-...} tags
        #[arg(long = "no-editions")]
        no_editions: bool,
        /// Language code for subtitle filenames (default: keep existing)
        #[arg(long = "sub-lang", default_value = "")]
        sub_lang: String,
    },
    /// Backfill episode titles (and premiere years) from TVMaze
    Titles {
        /// TV library root (default: current directory)
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Execute the renames (default is dry run)
        #[arg(long)]
        apply: bool,
        /// Use the first episode's title for multi-episode files
        #[arg(long = "multi-ep-first")]
        multi_ep_first: bool,
        /// Minimum TVMaze title similarity to accept (default 0.75)
        #[arg(long, default_value_t = 0.75)]
        threshold: f64,
        /// HTTP timeout in seconds (default 15)
        #[arg(long, default_value_t = 15)]
        timeout: u64,
    },
}
