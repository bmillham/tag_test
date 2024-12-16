use itertools::Itertools;
use lofty::error::{ErrorKind, LoftyError};
use lofty::prelude::*;
use lofty::probe::Probe;
use serde_derive::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::process::exit;
use std::time::Duration;
use toml;
use walkdir::WalkDir;

struct TrackInfo {
    title: String,
    artist: String,
    album: String,
    genre: String,
    track: u32,
    duration: Duration,
}

#[derive(Deserialize)]
struct Config {
    general: General,
    types: Types,
    directories: Directories,
}

#[derive(Deserialize)]
struct General {
    verbose: bool,
}

#[derive(Deserialize)]
struct Directories {
    scan: Vec<String>,
}

#[derive(Deserialize)]
struct Types {
    valid: Vec<String>,
}

struct ScanStats {
    other_files: u32,
    directories: u32,
    error_files: u32,
    valid_files: u32,
    found_types: HashMap<String, u32>,
}

fn main() {
    let config_file = "config.toml";
    let config_contents = match fs::read_to_string(config_file) {
        Ok(c) => c,
        Err(_) => {
            println!("Error reading {config_file}");
            exit(1);
        }
    };
    let config: Config = match toml::from_str(&config_contents) {
        Ok(c) => c,
        Err(e) => {
            println!("Error parsing {e}");
            exit(1);
        }
    };

    // Estimate files. Mainly for later use when I get a GUI working
    let estimate = scan_dirs(&config, true);
    for key in estimate.found_types.keys().sorted() {
        println!("{:?}: {:?}", key, estimate.found_types[key]);
    }
    println!(
        "Valid {}, Other: {} Dirs: {}",
        estimate.valid_files, estimate.other_files, estimate.directories
    );

    // Do the real scan
    let scan_results = scan_dirs(&config, false);
    for key in scan_results.found_types.keys().sorted() {
        println!("{:?}: {:?}", key, scan_results.found_types[key]);
    }
    println!(
        "Valid {}, Other: {}, Error: {}, Dirs: {}",
        scan_results.valid_files,
        scan_results.other_files,
        scan_results.error_files,
        scan_results.directories
    );
}

fn scan_dirs(config: &Config, estimate: bool) -> ScanStats {
    let mut scan_stats = ScanStats {
        other_files: 0,
        directories: 0,
        error_files: 0,
        valid_files: 0,
        found_types: HashMap::new(),
    };

    for dir in &config.directories.scan {
        for entry in WalkDir::new(dir)
            .sort_by_file_name()
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() {
                scan_stats.directories += 1;
                if config.general.verbose {
                    println!(
                        "{} Dir: {:?}",
                        if estimate { "Estimating" } else { "Scanning" },
                        entry.path().to_string_lossy()
                    );
                };
                continue;
            }
            let f_name = entry.file_name().to_string_lossy();
            let f_ext = f_name.split(".").last().unwrap_or("NONE").to_lowercase();
            scan_stats
                .found_types
                .entry(f_ext.clone())
                .and_modify(|ext| *ext += 1)
                .or_insert(1);

            if config.types.valid.iter().any(|t| t == &f_ext) {
                if !estimate {
                    let res = read_metadata(&entry.path().to_string_lossy());
                    // Don't print the results just to keep everything simple.
                    let t = match res {
                        Ok(t) => t,
                        Err(e) => {
                            println!("Error {}", e);
                            scan_stats.error_files += 1;
                            continue;
                        }
                    };
                    if config.general.verbose {
                        println!(
                            "{:?} {:?} {:?} {:?} {:?} {:?}",
                            t.artist, t.title, t.album, t.genre, t.track, t.duration
                        );
                    }
                }
                scan_stats.valid_files += 1;
            } else {
                scan_stats.other_files += 1;
            }
        }
    }
    scan_stats
}

fn read_metadata(file_name: &str) -> Result<TrackInfo, LoftyError> {
    let tagged_file_result = Probe::open(file_name)?.read();

    let tagged_file = match tagged_file_result {
        Ok(tagged_file_result) => tagged_file_result,
        Err(e) => return Err(e),
    };

    let tag = match tagged_file.primary_tag() {
        Some(primary_tag) => primary_tag,
        None => {
            println!("No tags found in {file_name}");
            return Err(LoftyError::new(ErrorKind::FakeTag));
        }
    };

    let properties = tagged_file.properties();
    /*let properties = match tagged_file.properties() {
        Ok(p) => p,
        Err(e) => {
            println!("Error {e} in properties: {file_name}");
            //return Err(e);
        }
    };*/

    let t_title = match tag.title() {
        Some(title) => title.to_string(),
        None => String::from(""),
    };

    let t_genre = match tag.genre() {
        Some(genre) => genre.to_string(),
        None => String::from(""),
    };

    let t_track = match tag.track() {
        Some(track) => track,
        None => {
            println!("Bad track info in {file_name}");
            0
        }
    };

    let t_info = TrackInfo {
        title: t_title,
        artist: tag.artist().unwrap().to_string(),
        album: tag.album().unwrap().to_string(),
        genre: t_genre,
        track: t_track,
        duration: properties.duration(),
    };
    Ok(t_info)
}
