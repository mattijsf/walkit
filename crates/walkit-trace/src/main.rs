// SPDX-License-Identifier: GPL-3.0-or-later

//! Runs each scenario of a reference set through the engine on a stepped clock and
//! writes what the engine did as a trace (`trace v1`), one file per scenario, for a
//! line-by-line diff against the traces of another engine.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: walkit-trace <scenarios.json> <out dir>");
        eprintln!(
            "writes <out dir>/<scenario name>.trace, trace v{}",
            walkit::TRACE_VERSION
        );
        return ExitCode::from(2);
    }
    eprintln!("no engine yet: nothing recorded");
    ExitCode::from(1)
}
