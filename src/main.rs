use indicatif::ProgressBar;
use std::{
    collections::{HashMap, HashSet},
    env,
    error::Error,
    fs::File,
    io::{BufRead, BufReader, Write},
    iter::FromIterator,
    process::{Command, Stdio},
    str::from_utf8,
};

static DATE_PREFIX: &'static str = "date:";

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    env::set_current_dir(args.get(1).expect("missing path arg"))?;

    let output = Command::new("git").arg("ls-files").output()?;
    let mut remaining_filenames: HashSet<&str> =
        HashSet::from_iter(from_utf8(output.stdout.as_ref())?.trim_end().split('\n'));

    let progress_bar = ProgressBar::new(remaining_filenames.len() as u64);

    let child = Command::new("git")
        .arg("log")
        .args(&[
            &format!("--format={}%ad", DATE_PREFIX),
            "--date=iso-strict",
            "--name-only",
        ])
        .stdout(Stdio::piped())
        .spawn()?;

    let reader = BufReader::new(child.stdout.unwrap());
    let mut timestamps = HashMap::new();
    let mut date: Option<String> = None;
    for line in reader.lines() {
        let line = line?;
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }

        if line.starts_with(DATE_PREFIX) {
            date = Some(line[DATE_PREFIX.len()..].to_owned())
        } else if remaining_filenames.remove(line) {
            progress_bar.inc(1);
            timestamps.insert(
                line.to_owned(),
                date.to_owned().unwrap_or(String::from("?")),
            );
            if remaining_filenames.is_empty() {
                break;
            }
        }
    }

    progress_bar.finish_and_clear();

    if !remaining_filenames.is_empty() {
        println!(
            "Couldn't find timestamps for {} files",
            remaining_filenames.len()
        );
    }

    let mut file = File::create("times.json")?;
    file.write_all(format!("{:#?}", timestamps).as_bytes())?;

    Ok(())
}
