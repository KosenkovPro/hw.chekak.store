//! Разбор структур ядра Windows — чистые функции без побочных эффектов.
//!
//! Здесь живут преобразования, которые можно проверить на любой платформе:
//! перевод времени Windows (100 нс с 1601 года) в Unix, чтение UTF-16 и
//! разбор битовых флагов процесса.

/// Разница между эпохой Windows (1601-01-01) и Unix (1970-01-01) в 100 нс.
const WINDOWS_EPOCH_DELTA_100NS: i64 = 116_444_736_000_000_000;

/// Переводит время Windows (100 нс с 1601 года) в секунды Unix.
pub fn windows_time_to_unix_seconds(ft_100ns: i64) -> Option<u64> {
    let unix_100ns = ft_100ns.checked_sub(WINDOWS_EPOCH_DELTA_100NS)?;
    if unix_100ns < 0 {
        return None;
    }
    Some((unix_100ns / 10_000_000) as u64)
}

/// Переводит длительность Windows (100 нс) в наносекунды.
///
/// В отличие от времени создания, это интервал, а не отметка: вычитать из него
/// эпоху Windows нельзя, иначе результат всегда будет отрицательным.
///
/// Ноль — это честные «процесс ещё не работал», а не «нет данных».
pub fn windows_duration_to_nanos(ft_100ns: i64) -> Option<u64> {
    if ft_100ns < 0 {
        return None;
    }
    Some((ft_100ns as u64).saturating_mul(100))
}

/// Читает UTF-16LE из сырого буфера.
///
/// `ptr` должен указывать на `len_bytes / 2` элементов `u16`.
pub fn utf16_to_string(ptr: *const u16, len_bytes: usize) -> Option<String> {
    if ptr.is_null() || len_bytes == 0 || !len_bytes.is_multiple_of(2) {
        return None;
    }
    // SAFETY: указатель ненулевой, длина чётная и кратна размеру `u16`,
    // срез покрывает ровно `len_bytes / 2` элементов в границах выделения.
    let words = unsafe { std::slice::from_raw_parts(ptr, len_bytes / 2) };
    Some(String::from_utf16_lossy(words))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_epoch_maps_to_unix_epoch() {
        assert_eq!(
            windows_time_to_unix_seconds(WINDOWS_EPOCH_DELTA_100NS),
            Some(0)
        );
    }

    #[test]
    fn windows_time_converts_to_unix() {
        // 2024-01-01T00:00:00Z = 133_800_000 секунд Unix.
        let ft = WINDOWS_EPOCH_DELTA_100NS + 133_800_000 * 10_000_000;
        assert_eq!(windows_time_to_unix_seconds(ft), Some(133_800_000));
    }

    #[test]
    fn negative_windows_time_is_unknown() {
        assert_eq!(windows_time_to_unix_seconds(0), None);
    }

    #[test]
    fn utf16_is_decoded() {
        let text: Vec<u16> = "MCPad".encode_utf16().collect();
        let ptr = text.as_ptr();
        assert_eq!(
            utf16_to_string(ptr, text.len() * 2).as_deref(),
            Some("MCPad")
        );
    }

    #[test]
    fn empty_utf16_is_none() {
        assert_eq!(utf16_to_string(std::ptr::null(), 0), None);
    }

    #[test]
    fn duration_is_not_treated_as_absolute_time() {
        // 1,5 секунды — длительность, а не отметка времени: эпоха не вычитается.
        assert_eq!(windows_duration_to_nanos(15_000_000), Some(1_500_000_000));
        assert_eq!(windows_duration_to_nanos(0), Some(0));
        assert_eq!(windows_duration_to_nanos(-1), None);
    }
}
