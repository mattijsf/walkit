// SPDX-License-Identifier: GPL-3.0-or-later

//! Runs each scenario of a reference set through the engine on a stepped clock and
//! writes what the engine did as a trace (`trace v1`), one file per scenario, for a
//! line-by-line diff against the traces of another engine.
//!
//! ```text
//! walkit-trace <scenarios.json> <out dir> [--skins <dir>] [--defaults <zip>] [--fonts <dir>] [--stats <file>]
//! ```
//!
//! The scenario file is a JSON list; each entry names the skin (`wal`, a file in the skins
//! folder), the skin's folder name (`skin`), the songs (`playlist`), and what the run does
//! (`theme`, `view`, `show`, `mainsize`, `taps`, `noLibrary`). The other `.wal` files in the
//! skins folder are the skin's siblings, and Winamp's default element set (`--defaults`) is
//! layered under the skin, unless the entry says `noSiblings` or `noDefaults`. `--fonts` names
//! the folder with the free fonts that stand in for Windows' system fonts. `--stats` writes
//! what each run took as JSON (see `walkit_trace::trace::Stats`).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use walkit_trace::archive;
use walkit_trace::trace::Stats;

fn usage() -> ExitCode {
    eprintln!(
        "usage: walkit-trace <scenarios.json> <out dir> [--skins <dir>] [--defaults <zip>] [--fonts <dir>] [--stats <file>]"
    );
    eprintln!(
        "writes <out dir>/<scenario name>.trace, trace v{}",
        walkit::TRACE_VERSION
    );
    ExitCode::from(2)
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

/// One scenario's timing as a JSON object.
fn stats_json(name: &str, s: &Stats) -> String {
    let mut ticks = s.ticks.clone();
    ticks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut draws = s.draws.clone();
    draws.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut dirty: Vec<f64> = s
        .draws
        .iter()
        .zip(s.dirty.iter())
        .filter(|(_, d)| **d)
        .map(|(t, _)| *t)
        .collect();
    dirty.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mean = |v: &[f64]| {
        if v.is_empty() {
            0.0
        } else {
            v.iter().sum::<f64>() / v.len() as f64
        }
    };
    format!(
        concat!(
            "{{\"name\":\"{}\",\"load_ms\":{:.4},\"ticks\":{},\"tick_mean_ms\":{:.5},\"tick_p95_ms\":{:.5},\"tick_max_ms\":{:.5},",
            "\"draw_mean_ms\":{:.5},\"draw_p95_ms\":{:.5},\"draw_max_ms\":{:.5},\"dirty_draws\":{},\"dirty_draw_p95_ms\":{:.5},",
            "\"frames\":{},\"total_ms\":{:.3}}}"
        ),
        name,
        s.load_ms,
        s.ticks.len(),
        mean(&s.ticks),
        percentile(&ticks, 0.95),
        ticks.last().copied().unwrap_or(0.0),
        mean(&s.draws),
        percentile(&draws, 0.95),
        draws.last().copied().unwrap_or(0.0),
        dirty.len(),
        percentile(&dirty, 0.95),
        s.frames,
        s.load_ms + s.ticks.iter().sum::<f64>() + s.draws.iter().sum::<f64>()
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut plain: Vec<String> = Vec::new();
    let mut skins: Option<PathBuf> = None;
    let mut defaults: Option<PathBuf> = None;
    let mut font_dir: Option<PathBuf> = None;
    let mut stats_path: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--skins" | "--defaults" | "--fonts" | "--stats" => {
                let Some(v) = args.get(i + 1) else {
                    return usage();
                };
                let p = PathBuf::from(v);
                match args[i].as_str() {
                    "--skins" => skins = Some(p),
                    "--defaults" => defaults = Some(p),
                    "--fonts" => font_dir = Some(p),
                    _ => stats_path = Some(p),
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
    let scenarios = match std::fs::read_to_string(scenarios_path)
        .map_err(|e| e.to_string())
        .and_then(|t| walkit_trace::scenario::parse_json(&t))
    {
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
    let fonts = font_dir.as_deref().and_then(archive::fonts);
    let mut failed = 0;
    let mut all_stats: Vec<String> = Vec::new();
    let started = std::time::Instant::now();
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
        let perf_start = std::time::Instant::now();
        let perf: walkit_trace::trace::Perf =
            Box::new(move || perf_start.elapsed().as_secs_f64() * 1000.0);
        let rec = walkit_trace::record(sc, files, archive::decoder(), fonts.clone(), perf);
        let path = out.join(format!("{}.trace", sc.name));
        if let Err(e) = std::fs::write(&path, &rec.text) {
            eprintln!("{}: {e}", path.display());
            failed += 1;
            continue;
        }
        println!(
            "{}: {} lines, {} frames",
            sc.name,
            rec.text.lines().count(),
            rec.stats.frames
        );
        all_stats.push(stats_json(&sc.name, &rec.stats));
    }
    if let Some(p) = stats_path {
        let json = format!(
            "{{\"engine\":\"walkit-trace native\",\"total_ms\":{:.1},\"scenarios\":[{}]}}\n",
            started.elapsed().as_secs_f64() * 1000.0,
            all_stats.join(",\n")
        );
        if let Err(e) = std::fs::write(&p, json) {
            eprintln!("{}: {e}", p.display());
            failed += 1;
        }
    }
    if failed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
