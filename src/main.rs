mod render;
mod system;

use std::env;
use std::process::ExitCode;

use render::{render_static, run_live, AnimationMode};
use system::SystemSampler;

struct Args {
    live: bool,
    donut: bool,
    logo: Option<String>,
    help: bool,
    version: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = env::args().skip(1);
    let mut parsed = Args {
        live: false,
        donut: false,
        logo: None,
        help: false,
        version: false,
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-l" | "-live" | "--live" => parsed.live = true,
            "-d" | "-donut" | "--donut" => {
                parsed.live = true;
                parsed.donut = true;
            }
            "--logo" => {
                parsed.logo = Some(
                    args.next()
                        .ok_or_else(|| "--logo requires a distro or logo name".to_string())?,
                );
            }
            "-h" | "--help" => parsed.help = true,
            "-V" | "--version" => parsed.version = true,
            other => return Err(format!("unknown option: {other}")),
        }
    }

    Ok(parsed)
}

fn print_help() {
    println!(
        "\
SmartFetch - fast system information with animated 3D terminal rendering

USAGE:
  sfetch                  Show system information
  sfetch --live            3D distro logo with live system statistics
  sfetch -live             Short alias for --live
  sfetch --live --donut    Rotating 3D donut with live statistics
  sfetch --donut           Shortcut for --live --donut
  sfetch -donut            Short alias for --donut
  sfetch --logo <name>     Select a built-in distro logo
  sfetch --help            Show this help
  sfetch --version         Show version

LIVE CONTROLS:
  Q / Esc     Quit and restore the terminal
  Space       Pause or resume the animation
  R           Refresh system information

The live view uses the alternate terminal screen and restores it on exit."
    );
}

fn run() -> Result<(), String> {
    let args = parse_args()?;

    if args.help {
        print_help();
        return Ok(());
    }

    if args.version {
        println!("SmartFetch {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let mut sampler = SystemSampler::new()?;
    let snapshot = sampler.refresh();

    let mode = if args.donut {
        AnimationMode::Donut
    } else {
        AnimationMode::DistroLogo
    };

    let logo = args.logo.as_deref().unwrap_or(&snapshot.distro_id);
    if args.live {
        run_live(&mut sampler, mode, logo).map_err(|error| error.to_string())
    } else {
        print!("{}", render_static(&snapshot, logo));
        Ok(())
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("sfetch: {error}");
            ExitCode::FAILURE
        }
    }
}
