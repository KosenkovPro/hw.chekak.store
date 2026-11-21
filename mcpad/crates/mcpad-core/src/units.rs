//! Единицы измерения.
//!
//! В ядре зафиксированы: **байты**, **наносекунды**, **проценты**, **штуки**.
//! Переводы живут здесь, чтобы бэкенды не переписывали их каждый сам.

/// Переводит тики CPU (Linux `SC_CLK_TCK`) в наносекунды.
pub fn ticks_to_nanos(ticks: u64, ticks_per_second: u64) -> u64 {
    if ticks_per_second == 0 {
        return 0;
    }
    ticks.saturating_mul(1_000_000_000) / ticks_per_second
}

/// Переводит килобайты (как в `/proc`) в байты.
pub fn kb_to_bytes(kb: u64) -> u64 {
    kb.saturating_mul(1024)
}

/// Переводит страницы в байты.
pub fn pages_to_bytes(pages: u64, page_size: u64) -> u64 {
    pages.saturating_mul(page_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_convert_to_nanos() {
        assert_eq!(ticks_to_nanos(100, 100), 1_000_000_000);
        assert_eq!(ticks_to_nanos(50, 100), 500_000_000);
        assert_eq!(ticks_to_nanos(10, 0), 0);
    }

    #[test]
    fn kb_and_pages_convert_to_bytes() {
        assert_eq!(kb_to_bytes(3000), 3_072_000);
        assert_eq!(pages_to_bytes(789, 4096), 789 * 4096);
    }
}
