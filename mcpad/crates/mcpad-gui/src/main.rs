//! `mcpad-gui` — запуск окна монитора.
//!
//! Вся логика окна живёт в библиотеке `mcpad_gui`: здесь только разбор
//! аргументов и запуск.

use std::path::PathBuf;

use mcpad_gui::{Lang, View, group::GroupBy};

/// Разбор аргументов командной строки.
fn parse_args() -> (Option<PathBuf>, Lang, View, GroupBy) {
    let mut theme_path: Option<PathBuf> = None;
    let mut lang = Lang::Ru;
    let mut view = View::Processes;
    let mut group_by = GroupBy::None;

    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--theme" => {
                if let Some(path) = args.next() {
                    theme_path = Some(PathBuf::from(path));
                }
            }
            "--lang" => {
                if let Some(parsed) = args.next().and_then(|code| Lang::from_code(&code)) {
                    lang = parsed;
                }
            }
            "--view" => {
                if let Some(parsed) = args.next().as_deref().and_then(View::from_code) {
                    view = parsed;
                }
            }
            "--group" => {
                if let Some(parsed) = args.next().as_deref().and_then(GroupBy::from_code) {
                    group_by = parsed;
                }
            }
            _ => {}
        }
    }

    (theme_path, lang, view, group_by)
}

fn print_usage() {
    println!("MCPad — монитор процессов и железа");
    println!();
    println!("Использование: mcpad-gui [опции]");
    println!();
    println!("  --theme <путь>   JSON-тема оформления");
    println!("  --lang <ru|en>   язык интерфейса");
    println!("  --view <раздел>  открыть сразу на разделе:");
    println!("                   processes | tree | performance | canvas | capabilities");
    println!("  --group <способ> группировать процессы в списке:");
    println!("                   none | application | parent");
}

fn main() -> eframe::Result<()> {
    if std::env::args().any(|argument| argument == "--help" || argument == "-h") {
        print_usage();
        return Ok(());
    }

    let (theme_path, lang, view, group_by) = parse_args();
    mcpad_gui::run(theme_path, lang, view, group_by)
}
