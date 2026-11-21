//! Разбор файлов `/proc` — чистые функции без ввода-вывода.
//!
//! Так парсеры проверяются тестами на любой платформе: на Windows CI тоже.
//! Слой чтения файлов лежит в модуле `sys` и собирается только под Linux.

/// Переводит тики CPU (Linux) в наносекунды.
pub use mcpad_core::units::ticks_to_nanos;

/// Данные из `/proc/<pid>/stat`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcStat {
    pub pid: u32,
    pub comm: String,
    pub state: char,
    pub ppid: u32,
    pub utime_ticks: u64,
    pub stime_ticks: u64,
    pub threads: u32,
    /// Тики с момента загрузки.
    pub start_ticks: u64,
    /// Размер резидентной памяти в страницах.
    pub rss_pages: u64,
    /// Виртуальный размер в байтах.
    pub vsize_bytes: u64,
}

/// Разбирает `/proc/<pid>/stat`.
///
/// Имя процесса может содержать пробелы и скобки, поэтому оно вырезается по
/// последней закрывающей скобке, а не по пробелу.
pub fn parse_proc_stat(text: &str) -> Option<ProcStat> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close < open {
        return None;
    }

    let pid = text[..open].trim().parse::<u32>().ok()?;
    let comm = text[open + 1..close].to_string();

    let rest: Vec<&str> = text[close + 1..].split_whitespace().collect();
    // Индексы — по полям после имени процесса (state=0, ppid=1, …).
    let field = |index: usize| -> Option<&str> { rest.get(index).copied() };

    Some(ProcStat {
        pid,
        comm,
        state: field(0)?.chars().next()?,
        ppid: field(1)?.parse().ok()?,
        utime_ticks: field(11)?.parse().ok()?,
        stime_ticks: field(12)?.parse().ok()?,
        threads: field(17)?.parse().ok()?,
        start_ticks: field(19)?.parse().ok()?,
        rss_pages: field(21)?.parse().ok()?,
        vsize_bytes: field(20)?.parse().ok()?,
    })
}

/// Данные из `/proc/<pid>/status`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProcStatus {
    pub name: Option<String>,
    pub ppid: Option<u32>,
    pub threads: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub vm_rss_bytes: Option<u64>,
    pub vm_size_bytes: Option<u64>,
    pub vm_swap_bytes: Option<u64>,
}

/// Разбирает `/proc/<pid>/status`.
pub fn parse_proc_status(text: &str) -> ProcStatus {
    let mut status = ProcStatus::default();

    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();

        match key {
            "Name" => status.name = Some(value.to_string()),
            "PPid" => status.ppid = value.parse().ok(),
            "Threads" => status.threads = value.parse().ok(),
            "Uid" => status.uid = value.split_whitespace().next().and_then(|v| v.parse().ok()),
            "Gid" => status.gid = value.split_whitespace().next().and_then(|v| v.parse().ok()),
            "VmRSS" => status.vm_rss_bytes = parse_kb(value),
            "VmSize" => status.vm_size_bytes = parse_kb(value),
            "VmSwap" => status.vm_swap_bytes = parse_kb(value),
            _ => {}
        }
    }

    status
}

/// Разбирает значение вида `3000 kB` в байты.
pub fn parse_kb(value: &str) -> Option<u64> {
    let number: u64 = value.split_whitespace().next()?.parse().ok()?;
    Some(number.saturating_mul(1024))
}

/// Размеченный в памяти файл из `/proc/<pid>/maps`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedFile {
    /// Полный путь к файлу.
    pub path: String,
}

/// Разбирает `/proc/<pid>/maps` и оставляет только реальные файлы.
///
/// Анонимные отображения (`[heap]`, `[stack]`, `[anon]`), `memfd` и файлы без
/// пути — это не модули, поэтому они отбрасываются. Иначе в графе появились бы
/// узлы, которых на самом деле нет.
pub fn parse_maps(text: &str) -> Vec<MappedFile> {
    let mut files: Vec<MappedFile> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in text.lines() {
        let Some(path) = map_path(line) else {
            continue;
        };
        if seen.insert(path.clone()) {
            files.push(MappedFile { path });
        }
    }

    files
}

/// Путь к файлу из строки `maps`, если это действительно файл.
fn map_path(line: &str) -> Option<String> {
    let mut parts = line.split_whitespace();
    // Диапазон адресов.
    parts.next()?;
    // Права доступа.
    parts.next()?;
    // Смещение.
    parts.next()?;
    // Устройство.
    parts.next()?;
    // inode.
    parts.next()?;

    let path = parts.next()?;
    if !path.starts_with('/') {
        return None;
    }
    Some(path.to_string())
}

/// Данные из `/proc/meminfo`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MemInfo {
    pub mem_total_bytes: Option<u64>,
    pub mem_available_bytes: Option<u64>,
    pub swap_total_bytes: Option<u64>,
}

/// Разбирает `/proc/meminfo`.
pub fn parse_meminfo(text: &str) -> MemInfo {
    let mut info = MemInfo::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let bytes = parse_kb(value.trim());
        match key {
            "MemTotal" => info.mem_total_bytes = bytes,
            "MemAvailable" => info.mem_available_bytes = bytes,
            "SwapTotal" => info.swap_total_bytes = bytes,
            _ => {}
        }
    }
    info
}

/// Накопительное время CPU в тиках: занятое и всё.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CpuTimes {
    /// Занятое время: user + nice + system + irq + softirq + steal.
    pub busy_ticks: u64,
    /// Полное время, включая idle и iowait.
    pub total_ticks: u64,
}

/// Разбирает строку `cpu` из `/proc/stat`.
pub fn parse_cpu_line(line: &str) -> Option<CpuTimes> {
    let mut parts = line.split_whitespace();
    let tag = parts.next()?;
    if tag != "cpu" {
        return None;
    }

    let values: Vec<u64> = parts
        .take(10)
        .filter_map(|value| value.parse::<u64>().ok())
        .collect();
    if values.len() < 4 {
        return None;
    }

    let user = values[0];
    let nice = values[1];
    let system = values[2];
    // idle и iowait входят в полное время, но не в занятое — поэтому
    // отдельно они не нужны.
    let _idle = values[3];
    let _iowait = values.get(4).copied().unwrap_or(0);
    let irq = values.get(5).copied().unwrap_or(0);
    let softirq = values.get(6).copied().unwrap_or(0);
    let steal = values.get(7).copied().unwrap_or(0);

    let total = values.iter().sum::<u64>();
    let busy = user + nice + system + irq + softirq + steal;

    Some(CpuTimes {
        busy_ticks: busy,
        total_ticks: total,
    })
}

/// Разбирает `/proc/uptime` (секунды с загрузки).
pub fn parse_uptime(text: &str) -> Option<f64> {
    text.split_whitespace().next()?.parse().ok()
}

/// Превращает содержимое `/proc/<pid>/cmdline` в строку.
///
/// Аргументы разделены нулевыми байтами.
pub fn parse_cmdline(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    let text = String::from_utf8_lossy(bytes);
    let joined = text.replace('\0', " ").trim().to_string();
    (!joined.is_empty()).then_some(joined)
}

/// Хеш идентификатора загрузки (FNV-1a) — чтобы PID после ребута не склеился.
pub fn hash_boot_id(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.trim().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT: &str = "1234 (bash) S 1200 1200 1200 34816 1234 4194304 100 0 0 0 12 34 0 0 20 0 1 0 987654 123456 789 18446744073709551615 1 1 0 0 0 0 0 0 0 0 0 0 17 3 0 0 0 0 0 0 0 0 0 0 0 0";

    const STATUS: &str = "Name:\tbash\nUmask:\t0022\nState:\tS (sleeping)\nTgid:\t1234\nPid:\t1234\nPPid:\t1200\nUid:\t1000\t1000\t1000\t1000\nGid:\t1000\t1000\t1000\t1000\nVmPeak:\t   123456 kB\nVmSize:\t   111111 kB\nVmRSS:\t    3000 kB\nVmSwap:\t       0 kB\nThreads:\t1\n";

    const MEMINFO: &str = "MemTotal:       16316436 kB\nMemFree:         2000000 kB\nMemAvailable:    8000000 kB\nBuffers:          300000 kB\nCached:          5000000 kB\nSwapTotal:       2097148 kB\nSwapFree:        2097148 kB\n";

    const MAPS: &str = "55d0a0000000-55d0a0001000 r--p 00000000 08:01 1234 /usr/bin/app\n55d0a0001000-55d0a0002000 r-xp 00001000 08:01 1234 /usr/bin/app\n55d0a1000000-55d0a1002000 rw-p 00000000 00:00 0 [heap]\n7f1c00000000-7f1c00022000 r-xp 00000000 08:01 5678 /usr/lib/x86_64-linux-gnu/libc.so.6\n7f1c10000000-7f1c10001000 rw-p 00000000 00:00 0 \n";

    #[test]
    fn parses_proc_stat() {
        let stat = parse_proc_stat(STAT).expect("stat разобран");
        assert_eq!(stat.pid, 1234);
        assert_eq!(stat.comm, "bash");
        assert_eq!(stat.state, 'S');
        assert_eq!(stat.ppid, 1200);
        assert_eq!(stat.utime_ticks, 12);
        assert_eq!(stat.stime_ticks, 34);
        assert_eq!(stat.threads, 1);
        assert_eq!(stat.start_ticks, 987_654);
        assert_eq!(stat.rss_pages, 789);
        assert_eq!(stat.vsize_bytes, 123_456);
    }

    #[test]
    fn parses_process_name_with_spaces_and_parentheses() {
        let text =
            "77 (Web Content (x)) S 1 1 1 0 0 0 0 0 0 0 5 6 0 0 20 0 3 0 4242 999 10 0 0 0 0 0 0";
        let stat = parse_proc_stat(text).expect("stat разобран");
        assert_eq!(stat.comm, "Web Content (x)");
        assert_eq!(stat.threads, 3);
        assert_eq!(stat.start_ticks, 4242);
    }

    #[test]
    fn parses_proc_status() {
        let status = parse_proc_status(STATUS);
        assert_eq!(status.name.as_deref(), Some("bash"));
        assert_eq!(status.ppid, Some(1200));
        assert_eq!(status.threads, Some(1));
        assert_eq!(status.uid, Some(1000));
        assert_eq!(status.vm_rss_bytes, Some(3_000 * 1024));
        assert_eq!(status.vm_size_bytes, Some(111_111 * 1024));
        assert_eq!(status.vm_swap_bytes, Some(0));
    }

    #[test]
    fn parses_meminfo() {
        let info = parse_meminfo(MEMINFO);
        assert_eq!(info.mem_total_bytes, Some(16_316_436 * 1024));
        assert_eq!(info.mem_available_bytes, Some(8_000_000 * 1024));
        assert_eq!(info.swap_total_bytes, Some(2_097_148 * 1024));
    }

    #[test]
    fn parses_cpu_line() {
        let times = parse_cpu_line("cpu  123456 1000 78901 9876543 1000 0 500 0 0 0")
            .expect("cpu разобрана");
        assert_eq!(times.busy_ticks, 123_456 + 1_000 + 78_901 + 500);
        assert_eq!(
            times.total_ticks,
            123_456 + 1_000 + 78_901 + 9_876_543 + 1_000 + 500
        );
    }

    #[test]
    fn ignores_per_cpu_lines() {
        assert!(parse_cpu_line("cpu0 61728 500 39450 4938271 500 0 250 0 0 0").is_none());
    }

    #[test]
    fn parses_uptime_and_cmdline() {
        assert_eq!(parse_uptime("123456.78 987654.32"), Some(123_456.78));
        assert_eq!(parse_cmdline(b"bash\0-l\0"), Some("bash -l".to_string()));
        assert_eq!(parse_cmdline(b""), None);
    }

    #[test]
    fn boot_id_hash_is_stable_and_distinct() {
        let a = hash_boot_id("6f1c2b3a-0000-1111-2222-333344445555");
        let b = hash_boot_id("6f1c2b3a-0000-1111-2222-333344445555");
        let c = hash_boot_id("7a1c2b3a-0000-1111-2222-333344445555");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn kb_conversion() {
        assert_eq!(parse_kb("3000 kB"), Some(3_072_000));
        assert_eq!(parse_kb("0 kB"), Some(0));
        assert_eq!(parse_kb("broken"), None);
    }

    #[test]
    fn maps_contain_only_real_files() {
        let files = parse_maps(MAPS);
        let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(
            paths,
            vec!["/usr/bin/app", "/usr/lib/x86_64-linux-gnu/libc.so.6"]
        );
    }

    #[test]
    fn maps_deduplicate_repeated_files() {
        let files = parse_maps(MAPS);
        // Один и тот же файл размечен дважды — узел должен быть один.
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn maps_without_paths_are_ignored() {
        let files = parse_maps("7f1c10000000-7f1c10001000 rw-p 00000000 00:00 0 \n");
        assert!(files.is_empty());
    }
}
