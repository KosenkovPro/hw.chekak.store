//! `mcpad` — командная строка MCPad.
//!
//! Подкоманды:
//!
//! * `mcpad list` — список процессов (текст или JSON);
//! * `mcpad once` — один снимок системы;
//! * `mcpad info <pid>` — подробности процесса;
//! * `mcpad capabilities` — что умеет текущая платформа.

mod render;

use clap::{Parser, Subcommand};
use mcpad_core::Snapshot;
use mcpad_probe::{Capability, PlatformProbe, Support};
use mcpad_sampler::Sampler;
use std::sync::Arc;
use std::time::Instant;

use render::Sort;

#[derive(Parser)]
#[command(
    name = "mcpad",
    version,
    about = "MCPad — кроссплатформенный диспетчер задач",
    long_about = "Список процессов, снимки системы и подробности процесса.\n\
                  Недоступные значения помечаются прочерком, а не нулём."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Список процессов.
    List {
        /// Машиночитаемый вывод.
        #[arg(long)]
        json: bool,
        /// По чему сортировать.
        #[arg(long, value_enum, default_value_t = Sort::Cpu)]
        sort: Sort,
        /// Сколько строк показать.
        #[arg(long, default_value_t = 50)]
        limit: usize,
        /// Окно измерения доли CPU, миллисекунды. `0` — без второго замера.
        #[arg(long, default_value_t = 300)]
        interval_ms: u64,
    },
    /// Один снимок системы.
    Once {
        /// Машиночитаемый вывод.
        #[arg(long)]
        json: bool,
    },
    /// Подробности одного процесса.
    Info {
        /// Идентификатор процесса.
        pid: u32,
        /// Машиночитаемый вывод.
        #[arg(long)]
        json: bool,
    },
    /// Возможности платформы.
    Capabilities {
        /// Машиночитаемый вывод.
        #[arg(long)]
        json: bool,
    },
    /// Модули, загруженные в процесс.
    Modules {
        /// Идентификатор процесса.
        pid: u32,
        /// Машиночитаемый вывод.
        #[arg(long)]
        json: bool,
    },
    /// Замер времени снимка: сколько занимает сбор данных о процессах.
    Bench {
        /// Сколько замеров сделать.
        #[arg(long, default_value_t = 20)]
        samples: usize,
        /// Машиночитаемый вывод.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::List {
            json,
            sort,
            limit,
            interval_ms,
        } => command_list(json, sort, limit, interval_ms),
        Command::Once { json } => command_once(json),
        Command::Info { pid, json } => command_info(pid, json),
        Command::Modules { pid, json } => command_modules(pid, json),
        Command::Capabilities { json } => command_capabilities(json),
        Command::Bench { samples, json } => command_bench(samples, json),
    };

    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mcpad: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Проб для текущей платформы.
fn platform_probe() -> Result<Box<dyn PlatformProbe>, mcpad_core::Error> {
    #[cfg(target_os = "windows")]
    {
        mcpad_backends_windows::platform_probe()
    }
    #[cfg(target_os = "linux")]
    {
        mcpad_backends_linux::platform_probe()
    }
    #[cfg(target_os = "macos")]
    {
        mcpad_backends_macos::platform_probe()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Err(mcpad_core::Error::Unsupported(
            "платформа не поддерживается".to_string(),
        ))
    }
}

fn command_list(
    json: bool,
    sort: Sort,
    limit: usize,
    interval_ms: u64,
) -> Result<(), mcpad_core::Error> {
    let sampler = Sampler::new(platform_probe()?);
    let snapshot = sample(&sampler, interval_ms)?;

    if json {
        println!("{}", to_json(&snapshot)?);
    } else {
        print!("{}", render::render_processes(&snapshot, sort, limit));
        print!("{}", render::render_system(&snapshot));
    }
    Ok(())
}

/// Снимает снимок, при необходимости делая второй замер через `interval_ms`.
///
/// Доля CPU — это дельта между двумя замерами, поэтому один замер не даёт
/// ничего: в этом случае значение остаётся неизвестным, а не нулевым.
fn sample(sampler: &Sampler, interval_ms: u64) -> Result<Arc<Snapshot>, mcpad_core::Error> {
    let first = sampler.refresh()?;
    if interval_ms == 0 {
        return Ok(first);
    }
    std::thread::sleep(std::time::Duration::from_millis(interval_ms));
    sampler.refresh()
}

fn command_once(json: bool) -> Result<(), mcpad_core::Error> {
    let sampler = Sampler::new(platform_probe()?);
    let snapshot = sampler.refresh()?;

    if json {
        println!("{}", to_json(&snapshot)?);
    } else {
        print!("{}", render::render_system(&snapshot));
        println!("{}", render::render_processes(&snapshot, Sort::Pid, 20));
    }
    Ok(())
}

fn command_info(pid: u32, json: bool) -> Result<(), mcpad_core::Error> {
    let probe = platform_probe()?;
    let details = probe.process_details(pid)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&details).unwrap_or_default()
        );
        return Ok(());
    }

    println!("PID: {}", details.key.pid);
    println!("Путь: {}", details.exe_path.as_deref().unwrap_or("—"));
    println!(
        "Команда: {}",
        details.command_line.as_deref().unwrap_or("—")
    );
    println!("Владелец: {}", details.user.as_deref().unwrap_or("—"));
    Ok(())
}

/// Модули, загруженные в процесс.
fn command_modules(pid: u32, json: bool) -> Result<(), mcpad_core::Error> {
    let probe = platform_probe()?;
    let links = probe.module_links(pid)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&links).unwrap_or_default()
        );
        return Ok(());
    }

    println!("Модули процесса {pid}: {}", links.len());
    for link in &links {
        match &link.path {
            Some(path) => println!("  {:<32} {path}", link.module_label),
            None => println!("  {:<32} —", link.module_label),
        }
    }
    Ok(())
}

fn command_capabilities(json: bool) -> Result<(), mcpad_core::Error> {
    let probe = platform_probe()?;

    let mut rows: Vec<(String, String)> = Vec::new();
    for capability in Capability::all() {
        let support = probe.support(*capability);
        let value = match support {
            Support::Full => "full".to_string(),
            Support::Partial => "partial".to_string(),
            Support::Unsupported(reason) => format!("unsupported: {reason}"),
        };
        rows.push((capability.as_str().to_string(), value));
    }

    if json {
        let map: serde_json::Map<String, serde_json::Value> = rows
            .into_iter()
            .map(|(key, value)| (key, serde_json::Value::String(value)))
            .collect();
        let mut root = serde_json::Map::new();
        root.insert(
            "platform".to_string(),
            serde_json::Value::String(probe.platform().as_str().to_string()),
        );
        root.insert("capabilities".to_string(), serde_json::Value::Object(map));
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::Value::Object(root)).unwrap_or_default()
        );
        return Ok(());
    }

    println!("Платформа: {}", probe.platform());
    for (capability, support) in rows {
        println!("{capability:<24} {support}");
    }
    Ok(())
}

/// Замер времени снимка.
///
/// Считает время полного сбора данных: обращение к ОС плюс разбор. Первый
/// вызов — прогрев, его результат не учитывается: он поднимает кэши и
/// прогружает библиотеки.
fn command_bench(samples: usize, json: bool) -> Result<(), mcpad_core::Error> {
    let probe = platform_probe()?;

    // Прогрев: первый вызов поднимает кэши и прогружает DLL.
    let _ = probe.collect()?;

    let count = samples.max(1);
    let mut durations: Vec<u128> = Vec::with_capacity(count);
    let mut processes = 0usize;

    for _ in 0..count {
        let start = Instant::now();
        let snapshot = probe.collect()?;
        durations.push(start.elapsed().as_micros());
        processes = snapshot.processes.len();
    }

    durations.sort_unstable();
    let min = durations[0];
    let max = durations[count - 1];
    let median = durations[count / 2];
    let mean = durations.iter().sum::<u128>() / count as u128;
    // Оценка «в лоб»: стоимость на один процесс, умноженная на 1000.
    // Фиксированная часть снимка (буфер ядра, разбор) из оценки не вычитается,
    // поэтому оценка заведомо верхняя.
    let per_process = if processes > 0 {
        median / processes as u128
    } else {
        0
    };
    let projected = per_process * 1000;

    if json {
        let mut root = serde_json::Map::new();
        root.insert("samples".to_string(), serde_json::Value::from(count));
        root.insert("processes".to_string(), serde_json::Value::from(processes));
        root.insert("min_us".to_string(), serde_json::Value::from(min as u64));
        root.insert(
            "median_us".to_string(),
            serde_json::Value::from(median as u64),
        );
        root.insert("mean_us".to_string(), serde_json::Value::from(mean as u64));
        root.insert("max_us".to_string(), serde_json::Value::from(max as u64));
        root.insert(
            "per_process_us".to_string(),
            serde_json::Value::from(per_process as u64),
        );
        root.insert(
            "projected_1000_us".to_string(),
            serde_json::Value::from(projected as u64),
        );
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::Value::Object(root)).unwrap_or_default()
        );
        return Ok(());
    }

    println!("Замер снимка: {count} замеров, {processes} процессов");
    println!("  минимум:  {:.3} мс", min as f64 / 1000.0);
    println!("  медиана:  {:.3} мс", median as f64 / 1000.0);
    println!("  среднее:  {:.3} мс", mean as f64 / 1000.0);
    println!("  максимум: {:.3} мс", max as f64 / 1000.0);
    println!("  на процесс: {:.2} мкс", per_process as f64);
    println!(
        "  оценка на 1000 процессов: {:.3} мс",
        projected as f64 / 1000.0
    );
    Ok(())
}

fn to_json(snapshot: &Snapshot) -> Result<String, mcpad_core::Error> {
    serde_json::to_string(snapshot).map_err(|e| mcpad_core::Error::Parse(e.to_string()))
}
