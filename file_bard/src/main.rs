use same_file::is_same_file;
use std::{
    env,
    fs::{self, DirEntry},
    io,
    path::{Path, PathBuf},
};

fn contains_loop<P: AsRef<Path>>(path: P) -> io::Result<Option<(PathBuf, PathBuf)>> {
    let path = path.as_ref();
    let mut path_buf = path.to_path_buf();
    while path_buf.pop() {
        if is_same_file(&path_buf, path)? {
            return Ok(Some((path_buf, path.to_path_buf())));
        } else if let Some(looped_paths) = contains_loop(&path_buf)? {
            return Ok(Some(looped_paths));
        }
    }
    return Ok(None);
}

fn walk_dir(dir: &PathBuf, func: fn(&DirEntry)) {
    for entry in fs::read_dir(dir).expect("We should be able to read this directory") {
        let entry = entry.expect("Entry should have a value");
        let path = entry.path();

        let metadata = fs::metadata(&path).expect("Path should have metadata");
        if metadata.is_dir() && contains_loop(&path).unwrap_or(None) != None {
            walk_dir(&path, func);
        }
        func(&entry);
    }
}

fn print_entry<'a>(entry: &'a DirEntry) {
    let path = entry.path();
    let metadata = fs::metadata(&path).expect("Path should have metadata");
    let last_modified = metadata
        .modified()
        .expect("Should have last modified track")
        .elapsed()
        .expect("Should be able to resolved elapsed time")
        .as_secs();

    if last_modified < 24 * 3600 && metadata.is_file() {
        println!(
            "Last modified: {:?} seconds, is read only: {:?}, size: {:?} bytes, filename: {:?}",
            last_modified,
            metadata.permissions().readonly(),
            metadata.len(),
            path.file_name()
                .ok_or("No filename")
                .expect("Should resolve str for filename")
        );
    }
}

fn main() {
    let current_dir = env::current_dir().expect("We should have a working directory");
    walk_dir(&current_dir, print_entry);
}
