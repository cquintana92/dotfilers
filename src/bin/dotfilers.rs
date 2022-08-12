#[macro_use]
extern crate tracing;

use anyhow::{Context, Result};
use clap::{App, Arg};
use dotfilers::{Config, Executor};

const CONFIG_FILE_ARG: &str = "config";
const DRY_RUN_ARG: &str = "dry-run";
const SECTIONS_ARG: &str = "sections";
const DEFAULT_FILE_NAME: &str = "dotfilers.yaml";

const VERSION: &str = git_version::git_version!(
    args = ["--tags", "--always", "--abbrev=1", "--dirty=-modified"],
    fallback = clap::crate_version!()
);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub struct IsoTime;
impl tracing_subscriber::fmt::time::FormatTime for IsoTime {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        let now = chrono::Local::now().naive_utc();
        let formatted = now.format("%Y-%m-%d %H:%M:%S%.3f");
        write!(w, "[{}] ", formatted)
    }
}

pub fn setup_logging(log_level: &str) {
    let log_level_value = format!("dotfilers={0}", log_level.to_lowercase());
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_timer(IsoTime)
        .with_env_filter(log_level_value)
        .finish();

    tracing::subscriber::set_global_default(subscriber).unwrap();
}

fn main() -> Result<()> {
    let app = App::new("dotfilers")
        .author("Carlos Quintana <carlos@cquintana.dev>")
        .about("Yet another dotfiles manager")
        .version(VERSION)
        .arg(
            Arg::with_name(CONFIG_FILE_ARG)
                .short("c")
                .long("config")
                .help("Config file to be executed")
                .takes_value(true)
                .default_value(DEFAULT_FILE_NAME)
                .required(true),
        )
        .arg(
            Arg::with_name(DRY_RUN_ARG)
                .short("d")
                .long("dry-run")
                .help("Do not actually perform any operation")
                .takes_value(false),
        )
        .arg(
            Arg::with_name(SECTIONS_ARG)
                .help("Which sections to deploy (if not specified, all of them will be deployed)")
                .display_order(3)
                .multiple(true)
                .required(false),
        )
        .get_matches();

    let config_file = app.value_of(CONFIG_FILE_ARG).context("config argument should be present")?;
    let dry_run = app.is_present(DRY_RUN_ARG);

    let config = Config::from_file(config_file).context("Error loading config")?;
    setup_logging(&config.program.log_level);
    let executor = if dry_run {
        Executor::dry_run(&config.program.shell, config.program.conflict_strategy)
    } else {
        Executor::new(&config.program.shell, config.program.conflict_strategy)
    };

    let root_dir = std::env::current_dir().context("Error getting current dir")?;

    if let Some(sections) = app.values_of(SECTIONS_ARG) {
        for section_name in sections {
            match config.state_config.states.get(section_name) {
                Some(directives) => {
                    executor.execute(&root_dir, section_name, directives)?;
                }
                None => {
                    error!("Could not find a section named {}", section_name);
                }
            }
        }
    } else {
        for (name, directives) in config.state_config.states {
            executor.execute(&root_dir, &name, &directives)?;
        }
    }

    Ok(())
}
