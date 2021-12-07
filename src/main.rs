use anyhow::{Context, Result};
use reqwest;
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use toml;

#[derive(Deserialize, Serialize, Debug)]
struct Config {
    year: String,
    session: String,
}

struct RunContext {
    day_name: String,
    base_folder: PathBuf,
}

impl RunContext {
    fn day_number(&self) -> Result<usize> {
        self.day_name[4..].parse().with_context(|| format!("Unable to parse day number from {}",self.day_name))
    }

    fn day_folder(&self) -> PathBuf {
        self.base_folder.join(&self.day_name)
    }

    fn aoc_config(&self) -> Result<Config> {
        let config_file = self.base_folder.join("aoc.toml");
        fs::read_to_string(&config_file)
        .with_context(|| format!("Error reading config file {:?}", &config_file))
        .and_then(|s| toml::from_str::<Config>(&s).context("Parsing config file"))
    }
}

fn retrieve_input(config: &Config, day_number: usize) -> Result<String> {
    let url = format!(
        "https://adventofcode.com/{}/day/{}/input",
        config.year, day_number
    );
    let client = reqwest::blocking::Client::new();
    Ok(client
        .get(&url)
        .header("Cookie", format!("session={}", config.session))
        .send()?
        .error_for_status()
        .context("Input not available (too soon?)")?
        .text()?)
}

fn get_inputs(run: &RunContext) -> Result<()> {
    let input_file = run.day_folder().join("input.txt");

    if input_file.exists() {
        println!("Input file {:?} exists, not retrieving", &input_file);
        return Ok(());
    }

    let input = retrieve_input(&run.aoc_config()?, run.day_number()?)?;
    fs::write(&input_file, input)?;

    Ok(())
}

fn copy_skeleton(run: &RunContext) -> Result<()> {
    let day_folder = run.day_folder();
    let skeleton_folder = run.base_folder.join("skeleton");

    if day_folder.exists() {
        println!("Day folder exists, not copying skeleton");
        return Ok(());
    }

    let mut cargo: toml::Value = fs::read_to_string(skeleton_folder.join("Cargo.toml"))
        .context("Unable to read skeleton/Cargo.toml")?
        .parse()
        .context("While reading skeleton/Cargo.toml")?;

    cargo
        .get_mut("package")
        .unwrap()
        .as_table_mut()
        .unwrap()
        .insert(
            "name".to_string(),
            toml::Value::String(run.day_name.clone()),
        );

    let src_folder = day_folder.join("src");
    fs::create_dir_all(&src_folder)?;
    fs::write(day_folder.join("Cargo.toml"), cargo.to_string())?;

    fs::copy(skeleton_folder.join("main.rs"), src_folder.join("main.rs"))?;

    Ok(())
}

/// An advent of code skeleton tool
/// 
/// Run in project folder with day folder name as argument to copy skeleton
/// Run from within day folder without argument to download inputs
#[derive(StructOpt, Debug)]
struct Opt {
    /// Day name. Format should be "day##"
    day_name: Option<String>,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    if let Some(day_name) = opt.day_name {
        let run = RunContext{day_name, base_folder: std::env::current_dir()?};
        copy_skeleton(&run)
    } else {
        let current_folder = std::env::current_dir()?;
        let base_folder = current_folder.parent().expect("No parent folder").to_owned();
        let day_name = current_folder.file_name().unwrap().to_str().expect("Invalid folder name").to_owned();
        let run = RunContext{base_folder, day_name};
        run.aoc_config()?;
        get_inputs(&run)
    }
}
