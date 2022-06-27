use dotfilers::Result;
use rand::{distributions::Alphanumeric, Rng};
use std::env::temp_dir;
use std::path::{Path, PathBuf};

pub fn run_with_temp_dir(cb: impl FnOnce(PathBuf) -> Result<()>) {
    let tmp = temp_dir();
    let tmp_dir = tmp.join(random_string(10));
    std::fs::create_dir_all(&tmp_dir).expect("Error creating temp dir");
    cb(tmp_dir.clone()).expect("Error running test");
    std::fs::remove_dir_all(&tmp_dir).expect("Error removing temp dir");
}

pub fn random_string(length: usize) -> String {
    rand::thread_rng().sample_iter(&Alphanumeric).take(length).map(char::from).collect()
}

pub fn write_file<P: AsRef<Path>>(dir: P, file_name: &str, contents: &str) {
    let file_path = dir.as_ref().join(file_name);
    std::fs::write(file_path, contents).expect("Error writing test file");
}
