// SPDX-License-Identifier: GPL-3.0-or-later

//! Runs each scenario of a reference set through the engine on a stepped clock and
//! writes what the engine did as a trace (`trace v1`), one file per scenario, for a
//! line-by-line diff against the traces of another engine.
//!
//! ```text
//! walkit-trace <scenarios.json> <out dir> [--skins <dir>] [--defaults <zip>] [--fonts <dir>]
//! ```
//!
//! The scenario file is a JSON list; each entry names the skin (`wal`, a file in the skins
//! folder), the skin's folder name (`skin`), the songs (`playlist`), and what the run does
//! (`theme`, `view`, `show`, `mainsize`, `taps`, `noLibrary`). The other `.wal` files in the
//! skins folder are the skin's siblings, and Winamp's default element set (`--defaults`) is
//! layered under the skin, unless the entry says `noSiblings` or `noDefaults`.

mod archive;
mod host;
mod scenario;
mod trace;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!("usage: walkit-trace <scenarios.json> <out dir> [--skins <dir>] [--defaults <zip>] [--fonts <dir>]");
    eprintln!(
        "writes <out dir>/<scenario name>.trace, trace v{}",
        walkit::TRACE_VERSION
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut plain: Vec<String> = Vec::new();
    let mut skins: Option<PathBuf> = None;
    let mut defaults: Option<PathBuf> = None;
    let mut fonts: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--skins" | "--defaults" | "--fonts" => {
                let Some(v) = args.get(i + 1) else {
                    return usage();
                };
                let p = PathBuf::from(v);
                match args[i].as_str() {
                    "--skins" => skins = Some(p),
                    "--defaults" => defaults = Some(p),
                    _ => fonts = Some(p),
                }
                i += 2;
            }
            a => {
                plain.push(a.to_string());
                i += 1;
            }
        }
    }
    if plain.len() != 2 {
        return usage();
    }
    let scenarios_path = Path::new(&plain[0]);
    let out = Path::new(&plain[1]);
    let skins = skins.unwrap_or_else(|| {
        scenarios_path
            .parent()
            .map(|p| p.join("skins"))
            .unwrap_or_else(|| PathBuf::from("skins"))
    });
    let scenarios = match scenario::read(scenarios_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {e}", scenarios_path.display());
            return ExitCode::from(1);
        }
    };
    if let Err(e) = std::fs::create_dir_all(out) {
        eprintln!("{}: {e}", out.display());
        return ExitCode::from(1);
    }
    let siblings = archive::Siblings::new(&skins);
    let mut failed = 0;
    for sc in &scenarios {
        let wal = skins.join(&sc.wal);
        if !wal.exists() {
            eprintln!("{}: no {}", sc.name, sc.wal);
            failed += 1;
            continue;
        }
        let files = match archive::skin_files(&wal, &siblings, defaults.as_deref(), sc) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{}: {e}", sc.name);
                failed += 1;
                continue;
            }
        };
        let text = trace::record(sc, files, fonts.as_deref());
        let path = out.join(format!("{}.trace", sc.name));
        if let Err(e) = std::fs::write(&path, &text) {
            eprintln!("{}: {e}", path.display());
            failed += 1;
            continue;
        }
        let frames = text.lines().filter(|l| l.starts_with("frame t=")).count();
        println!(
            "{}: {} lines, {frames} frames",
            sc.name,
            text.lines().count()
        );
    }
    if failed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
