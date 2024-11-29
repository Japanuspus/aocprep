use anyhow::{Context, Result};
use itertools::Itertools;
use reqwest;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
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
        self.day_name[3..]
            .parse()
            .with_context(|| format!("Unable to parse day number from {}", self.day_name))
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

fn retrieve_aoc(config: &Config, day_number: usize, postfix: &str) -> Result<String> {
    let url = format!(
        "https://adventofcode.com/{}/day/{}{}",
        config.year, day_number, postfix
    );
    let client = reqwest::blocking::Client::new();
    Ok(client
        .get(&url)
        .header("Cookie", format!("session={}", config.session))
        // https://old.reddit.com/r/adventofcode/comments/z9dhtd/please_include_your_contact_info_in_the_useragent/
        .header(
            "User-Agent",
            "https://github.com/Japanuspus/aocprep by janus@insignificancegalore.net",
        )
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

    let input = retrieve_aoc(&run.aoc_config()?, run.day_number()?, "/input")?;
    fs::write(&input_file, input)?;

    Ok(())
}

fn expand_cargo_toml(run: &RunContext, src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    println!("Expanding Cargo.toml with day name");
    let mut cargo: toml::Value = fs::read_to_string(src)
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

    fs::write(dst, cargo.to_string())?;
    Ok(())
}

fn copy_dir_recursive(
    run: &RunContext,
    src: impl AsRef<Path>,
    dst: impl AsRef<Path>,
    do_expand_cargo_toml: bool,
) -> Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_recursive(
                run,
                entry.path(),
                dst.as_ref().join(entry.file_name()),
                false,
            )?;
        } else if do_expand_cargo_toml && entry.file_name().eq("Cargo.toml") {
            expand_cargo_toml(run, entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn copy_skeleton(run: &RunContext) -> Result<()> {
    let day_folder = run.day_folder();
    let skeleton_folder = run.base_folder.join("skeleton");

    if day_folder.exists() {
        println!("Day folder exists, not copying skeleton");
    } else {
        println!(
            "No day folder exists for {}, will copy skeleton with Cargo.toml expansion",
            &run.day_name
        );
        copy_dir_recursive(run, skeleton_folder, day_folder, true)?;
    }

    Ok(())
}

fn parse_tests(html: &str) -> Result<Vec<String>> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("pre>code").unwrap();
    let tests = document
        .select(&selector)
        .map(|el| el.text().join(""))
        .collect();
    Ok(tests)
}

#[test]
fn test_parse_tests() {
    let html = r##"<!DOCTYPE html>
    <html lang="en-us">
    <head>
    <meta charset="utf-8"/>
    <title>Day 7 - Advent of Code 2021</title>
    <link rel="shortcut icon" href="/favicon.png"/>
    </head><!--
    Oh, hello!  Funny seeing you here.
    -->
    <body>
    <p>For example, consider the following horizontal positions:</p>
    <pre><code>16,1,2,0,4,2,7,1,2,14</code></pre>
    <p>This means there's a crab with horizontal position <code>16</code>, a crab with horizontal position <code>1</code>, and so on.</p>
    </body>
    </html>
    "##;
    let v = parse_tests(&html).unwrap();
    assert!(v.len() == 1);
    assert!(v[0] == "16,1,2,0,4,2,7,1,2,14");
}

fn get_tests(run: &RunContext) -> Result<()> {
    let html = retrieve_aoc(&run.aoc_config()?, run.day_number()?, "")?;
    let tests = parse_tests(&html)?;

    for (i, s) in tests.iter().enumerate() {
        let dst = run.day_folder().join(format!("test{:02}.txt", i));
        if dst.exists() {
            println!("Test file {:?} exists", dst);
        } else {
            println!("Writing test file {:?}", dst);
            fs::write(&dst, s)?;
        }
    }
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
        let run = RunContext {
            day_name,
            base_folder: std::env::current_dir()?,
        };
        copy_skeleton(&run)
    } else {
        let current_folder = std::env::current_dir()?;
        let base_folder = current_folder
            .parent()
            .expect("No parent folder")
            .to_owned();
        let day_name = current_folder
            .file_name()
            .unwrap()
            .to_str()
            .expect("Invalid folder name")
            .to_owned();
        let run = RunContext {
            base_folder,
            day_name,
        };
        run.aoc_config()?;
        get_inputs(&run)?;
        get_tests(&run)?;
        Ok(())
    }
}
