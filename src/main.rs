//! `w3i-rs`: checks map info files and shows them as text. See `usage`.

use std::process::ExitCode;

use w3i_rs::{describe, parse, text};

fn usage() {
    eprintln!("usage: w3i-rs <war3map.w3i>          check: read and write back");
    eprintln!("       w3i-rs --text <war3map.w3i>   the text form, to stdout");
    eprintln!("       w3i-rs --from-text <in.ini> <war3map.w3i>   the text form back to a file");
}

fn run(args: &[&str]) -> Result<(), String> {
    match args {
        ["--text", path] => {
            let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
            let info = parse(&bytes).map_err(|e| format!("{path}: {e}"))?;
            print!("{}", text::to_text(&info));
            Ok(())
        }
        ["--from-text", input, output] => {
            let src = std::fs::read_to_string(input).map_err(|e| format!("{input}: {e}"))?;
            let parsed = text::from_text(&src);
            let Some(info) = parsed.info else {
                let lines: Vec<String> = parsed
                    .errors
                    .iter()
                    .map(|e| format!("{input}:{}:{}: {}", e.line, e.col, e.message))
                    .collect();
                return Err(lines.join("\n"));
            };
            let bytes = info.to_bytes().map_err(|e| format!("{input}: {e}"))?;
            std::fs::write(output, bytes).map_err(|e| format!("{output}: {e}"))
        }
        [path] if !path.starts_with('-') => {
            let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
            let info = parse(&bytes).map_err(|e| format!("{path}: {e}"))?;
            if info.to_bytes().ok() != Some(bytes) {
                return Err(format!("{path}: differs after writing"));
            }
            println!(
                "{path}: version {} ({}), {} players, {} forces{}",
                info.version,
                describe(info.version),
                info.players.len(),
                info.forces.len(),
                info.cut
                    .as_ref()
                    .map_or(String::new(), |c| format!(", cut after its {}", c.after.name()))
            );
            Ok(())
        }
        _ => {
            usage();
            Err(String::new())
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{e}");
            }
            ExitCode::FAILURE
        }
    }
}
