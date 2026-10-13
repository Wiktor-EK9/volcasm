//! Volcasm: sample and pattern manager for the KORG volca sample.

mod app;
mod state;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;

use app::{Options, VolcasmApp};
use eframe::egui;
use state::PartView;

const USAGE: &str = "usage: volcasm [project.volcasm | project.vsj] [--select <slot>] \
                     [--sequences] [--pattern <1-10>] [--params <part>] [--motion <part>] \
                     [--size <width>x<height>] [--screenshot <out.png>]";

fn parse_args() -> Result<(Options, Option<egui::Vec2>), String> {
    let mut options = Options::default();
    let mut size = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .ok_or_else(|| format!("{arg} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--select" => {
                options.select = Some(value()?.parse().map_err(|_| "invalid slot".to_string())?)
            }
            "--screenshot" => options.screenshot = Some(PathBuf::from(value()?)),
            "--sequences" => options.sequences = true,
            "--pattern" => {
                let n: usize = value()?
                    .parse()
                    .map_err(|_| "invalid pattern".to_string())?;
                options.pattern = Some(n.checked_sub(1).ok_or("patterns are 1-10")?);
            }
            "--params" | "--motion" => {
                let n: usize = value()?.parse().map_err(|_| "invalid part".to_string())?;
                let view = if arg == "--params" {
                    PartView::Params
                } else {
                    PartView::Motion
                };
                options
                    .expand
                    .push((n.checked_sub(1).ok_or("parts are 1-10")?, view));
            }
            "--size" => {
                let v = value()?;
                let (w, h) = v.split_once('x').ok_or("size is <width>x<height>")?;
                let parse = |s: &str| s.parse::<f32>().map_err(|_| "invalid size".to_string());
                size = Some(egui::vec2(parse(w)?, parse(h)?));
            }
            "--help" | "-h" => return Err(USAGE.into()),
            _ if !arg.starts_with('-') && options.open.is_none() => {
                options.open = Some(PathBuf::from(arg))
            }
            _ => return Err(format!("unknown argument {arg}\n{USAGE}")),
        }
    }
    Ok((options, size))
}

fn main() -> ExitCode {
    let (options, size) = match parse_args() {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("volcasm")
            .with_app_id("volcasm")
            .with_inner_size(size.unwrap_or(egui::vec2(1440.0, 900.0)))
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    match eframe::run_native(
        "volcasm",
        native,
        Box::new(|cc| Ok(Box::new(VolcasmApp::new(cc, options)))),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}
