extern crate toml;
extern crate rustc_serialize;
extern crate tempdir;

#[macro_use]
extern crate clap;

#[macro_use]
extern crate lazy_static;

use std::path::Path;
use std::io::{self, Write};
use std::process::Command;
use std::env;
use tempdir::TempDir;
use rustc_serialize::json::Json;

use rt_result::RtResult;
use dependencies::dependency_trees;
use tags::{update_tags, create_tags, move_tags};
use config::Config;

mod rt_result;
mod dependencies;
mod dirs;
mod tags;
mod types;
mod config;

fn main() {
    execute().unwrap_or_else(|err| {
        writeln!(&mut io::stderr(), "{}", err).unwrap();
        std::process::exit(1);
    });
}

fn execute() -> RtResult<()> {
    let config = try!(Config::from_command_args());
    try!(update_all_tags(&config));
    let _ = try!(config.close_temp_dirs());
    Ok(())
}

fn update_all_tags(config: &Config) -> RtResult<()> {
    let metadata = try!(fetch_source_and_metadata(&config));
    try!(update_std_lib_tags(&config));

    let dep_trees = try!(dependency_trees(&config, &metadata));
    for tree in &dep_trees {
        if ! config.quiet {
            println!("Creating tags for '{}' ...", tree.source.name);
        }

        try!(update_tags(&config, &tree));
    }

    Ok(())
}

fn fetch_source_and_metadata(config: &Config) -> RtResult<Json> {
    if ! config.quiet {
        println!("Fetching source and metadata ...");
    }

    try!(env::set_current_dir(&config.start_dir));

    let mut cmd = Command::new("cargo");
    cmd.arg("metadata");

    let output = try!(cmd.output()
        .map_err(|err| format!("'cargo' execution failed: {}\nIs 'cargo' correctly installed?", err)));

    if ! output.status.success() {
        let mut msg = String::from_utf8_lossy(&output.stderr).into_owned();
        if msg.is_empty() {
            msg = String::from_utf8_lossy(&output.stdout).into_owned();
        }

        return Err(msg.into());
    }

    Ok(try!(Json::from_str(&String::from_utf8_lossy(&output.stdout))))
}

fn update_std_lib_tags(config: &Config) -> RtResult<()> {
    let src_path_str = env::var("RUST_SRC_PATH");
    if ! src_path_str.is_ok() {
        return Ok(());
    }

    let src_path_str = src_path_str.unwrap();
    let src_path = Path::new(&src_path_str);
    if ! src_path.is_dir() {
        return Err(format!("Missing rust source code at '{}'!", src_path.display()).into());
    }

    let std_lib_tags = src_path.join(config.tags_spec.file_name());
    if std_lib_tags.is_file() && ! config.force_recreate {
        return Ok(());
    }

    let possible_src_dirs = [
        "liballoc",
        "libarena",
        "libbacktrace",
        "libcollections",
        "libcore",
        "libflate",
        "libfmt_macros",
        "libgetopts",
        "libgraphviz",
        "liblog",
        "librand",
        "librbml",
        "libserialize",
        "libstd",
        "libsyntax",
        "libterm"
    ];

    let mut src_dirs = Vec::new();
    for dir in &possible_src_dirs {
        let src_dir = src_path.join(&dir);
        if src_dir.is_dir() {
            src_dirs.push(src_dir);
        }
    }

    let temp_dir = try!(TempDir::new_in(&src_path, "std-lib-temp-dir"));
    let tmp_std_lib_tags = temp_dir.path().join("std_lib_tags");

    if ! config.quiet {
        println!("Creating tags for the standard library ...");
    }

    try!(create_tags(config, &src_dirs, &tmp_std_lib_tags));
    try!(move_tags(config, &tmp_std_lib_tags, &std_lib_tags));

    try!(temp_dir.close());

    Ok(())
}
