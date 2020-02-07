use chrono::{DateTime, NaiveDateTime, Utc};
use git2::{DiffOptions, Error, ObjectType, Repository, Tree};
use std::{
    collections::{HashMap, HashSet},
    env,
    fs::File,
    io::Write,
    iter::FromIterator,
    path::Path,
};

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    let path = match args.get(1) {
        Some(path) => Path::new(path),
        None => panic!("missing path arg"),
    };

    let repo = Repository::open(path)?;

    let mut walker = repo.revwalk()?;
    walker.push_head()?;

    let mut filenames = Vec::new();
    let head_tree = repo.head()?.peel_to_commit()?.tree()?;
    recursively_collect_filenames(&repo, &head_tree, None, &mut filenames)?;

    let mut modified_times = HashMap::new();
    let mut unaccounted_filenames: HashSet<String> = HashSet::from_iter(filenames);

    let mut newer_tree = head_tree;
    for oid in walker.skip(1) {
        if unaccounted_filenames.is_empty() {
            break;
        }

        let oid = oid?;
        let commit = repo.find_commit(oid)?;

        let old_tree = commit.tree()?;

        let mut diff_options = DiffOptions::new();
        if unaccounted_filenames.len() < 50 {
            diff_options.disable_pathspec_match(true);
            for filename in unaccounted_filenames.iter() {
                diff_options.pathspec(filename);
            }
        }
        let diff =
            repo.diff_tree_to_tree(Some(&old_tree), Some(&newer_tree), Some(&mut diff_options))?;

        for delta in diff.deltas() {
            let file = delta.new_file();
            let path = match file.path() {
                Some(path) => path.to_string_lossy().to_owned().to_string(),
                None => continue,
            };

            if !unaccounted_filenames.remove(&path) {
                continue;
            }
            let datetime = DateTime::<Utc>::from_utc(
                NaiveDateTime::from_timestamp(commit.committer().when().seconds(), 0),
                Utc,
            );
            let time_str = datetime.to_rfc3339();
            modified_times.entry(path).or_insert(time_str);
        }

        newer_tree = old_tree;
    }

    assert!(unaccounted_filenames.is_empty());

    let mut file = match File::create("times.json") {
        Err(why) => panic!("couldn't create: {}", why),
        Ok(file) => file,
    };

    match file.write_all(format!("{:#?}", modified_times).as_bytes()) {
        Err(why) => panic!("couldn't write: {}", why),
        Ok(_) => println!("successfully wrote"),
    };

    Ok(())
}

fn recursively_collect_filenames(
    repo: &Repository,
    tree: &Tree,
    path: Option<String>,
    filenames: &mut Vec<String>,
) -> Result<(), Error> {
    for entry in tree.iter() {
        let name = match entry.name() {
            Some(name) => name.to_owned(),
            None => continue,
        };
        let current_path = path
            .clone()
            .and_then(|prefix| Some(format!("{}/{}", prefix, name)))
            .unwrap_or(name);

        match entry.kind() {
            Some(ObjectType::Blob) => {
                filenames.push(current_path);
                continue;
            }
            Some(ObjectType::Tree) => {
                let object = entry.to_object(repo)?;
                let sub_tree = match object.as_tree() {
                    Some(tree) => tree,
                    None => continue,
                };
                recursively_collect_filenames(repo, &sub_tree, Some(current_path), filenames)?;
            }
            _ => continue,
        };
    }
    Ok(())
}
