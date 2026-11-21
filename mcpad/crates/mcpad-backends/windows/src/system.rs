//! Чтение списка процессов Windows через `NtQuerySystemInformation`.
//!
//! Один вызов отдаёт всё: имена, родителей, время CPU, память, потоки,
//! дескрипторы и счётчики ввода-вывода. Это тот же источник, что использует
//! штатный диспетчер Windows.

use std::mem::offset_of;

use mcpad_core::ProcessInfo;

use crate::parse::{utf16_to_string, windows_duration_to_nanos, windows_time_to_unix_seconds};

/// Класс `SystemProcessInformation` для `NtQuerySystemInformation`.
const SYSTEM_PROCESS_INFORMATION: u32 = 5;

/// `STATUS_INFO_LENGTH_MISMATCH` — ожидаемый ответ на запрос размера буфера.
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004u32 as i32;

/// `UNICODE_STRING` в 64-битном виде: длина, максимальная длина, указатель.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct UnicodeString {
    pub(crate) length: u16,
    pub(crate) maximum_length: u16,
    pub(crate) buffer: *const u16,
}

/// `SYSTEM_PROCESS_INFORMATION` в 64-битном виде.
///
/// Порядок полей и их выравнивание повторяют определение в `phnt.h`; тест
/// `layout_matches_documented_offsets` фиксирует ожидаемые смещения, чтобы
/// случайная правка структуры не осталась незамеченной. Смещения сверены с
/// живой машиной: `WorkingSetSize`, `PagefileUsage`, `PrivatePageCount` и
/// счётчики ввода-вывода совпадают с `GetProcessMemoryInfo` и
/// `GetProcessIoCounters`.
#[repr(C)]
struct SystemProcessInformation {
    next_entry_offset: u32,
    number_of_threads: u32,
    working_set_private_size: u64,
    hard_fault_count: u32,
    number_of_threads_high_watermark: u32,
    cycle_time: u64,
    create_time: i64,
    user_time: i64,
    kernel_time: i64,
    image_name: UnicodeString,
    base_priority: i32,
    unique_process_id: usize,
    inherited_from_unique_process_id: usize,
    handle_count: u32,
    session_id: u32,
    unique_process_key: usize,
    peak_virtual_size: u64,
    virtual_size: u64,
    page_fault_count: u32,
    peak_working_set_size: u64,
    working_set_size: u64,
    quota_peak_paged_pool_usage: u64,
    quota_paged_pool_usage: u64,
    quota_peak_non_paged_pool_usage: u64,
    quota_non_paged_pool_usage: u64,
    pagefile_usage: u64,
    peak_pagefile_usage: u64,
    private_page_count: u64,
    read_operation_count: u64,
    write_operation_count: u64,
    other_operation_count: u64,
    read_transfer_count: u64,
    write_transfer_count: u64,
    other_transfer_count: u64,
}

/// Запись о процессе, разобранная из структуры ядра.
pub struct RawProcess {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub create_time_unix: Option<u64>,
    pub cpu_time_nanos: Option<u64>,
    pub working_set: Option<u64>,
    pub private_bytes: Option<u64>,
    pub thread_count: Option<u32>,
    pub handle_count: Option<u32>,
    pub io_read: Option<u64>,
    pub io_write: Option<u64>,
}

/// Запрашивает у ядра список процессов.
pub fn query_processes() -> Result<Vec<RawProcess>, mcpad_core::Error> {
    let mut needed: u32 = 0;
    // SAFETY: нулевой буфер и длина 0 — штатный способ узнать требуемый размер;
    // указатель `needed` указывает на живую переменную.
    let status = unsafe {
        ntdll::NtQuerySystemInformation(
            SYSTEM_PROCESS_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        )
    };
    if status != STATUS_INFO_LENGTH_MISMATCH && needed == 0 {
        return Err(mcpad_core::Error::Io(format!(
            "NtQuerySystemInformation не вернул размер буфера (статус {status:#x})"
        )));
    }

    // Буфер может вырасти между запросом размера и чтением — запас в 1/8.
    let capacity = (needed as usize)
        .saturating_add(needed as usize / 8)
        .max(4096);
    let mut buffer = vec![0u8; capacity];

    // SAFETY: буфер выделен и живёт до конца вызова, длина передаётся явно
    // и равна фактической длине буфера, `needed` — живая переменная.
    let status = unsafe {
        ntdll::NtQuerySystemInformation(
            SYSTEM_PROCESS_INFORMATION,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
            &mut needed,
        )
    };
    if status != 0 {
        return Err(mcpad_core::Error::Io(format!(
            "NtQuerySystemInformation завершился статусом {status:#x}"
        )));
    }

    parse_processes(&buffer[..needed as usize])
}

/// Разбирает буфер с записями `SYSTEM_PROCESS_INFORMATION`.
fn parse_processes(buffer: &[u8]) -> Result<Vec<RawProcess>, mcpad_core::Error> {
    let mut processes = Vec::new();
    let mut offset = 0usize;
    let mut guard = 0usize;

    while offset + std::mem::size_of::<SystemProcessInformation>() <= buffer.len() {
        guard += 1;
        if guard > 100_000 {
            return Err(mcpad_core::Error::Parse(
                "слишком много записей в буфере процессов".to_string(),
            ));
        }

        let entry = &buffer[offset..];
        let image_name =
            read_unicode_string(entry, offset_of!(SystemProcessInformation, image_name))?;

        let pid = read_pid(entry);
        // Ядро не отдаёт имя для PID 0 — это «System Idle Process», он же
        // «простой процесс». Его время CPU — это время простоя.
        let name = image_name.unwrap_or_else(|| {
            if pid == 0 {
                "System Idle Process".to_string()
            } else {
                format!("pid-{pid}")
            }
        });

        let user_time = read_i64(entry, offset_of!(SystemProcessInformation, user_time));
        let kernel_time = read_i64(entry, offset_of!(SystemProcessInformation, kernel_time));
        // Время процесса — это длительность (интервалы по 100 нс), а не
        // отметка времени: эпоху отсюда вычитать нельзя.
        let cpu_time_nanos = match (user_time, kernel_time) {
            (Some(u), Some(k)) => u.checked_add(k).and_then(windows_duration_to_nanos),
            _ => None,
        };

        processes.push(RawProcess {
            pid,
            parent_pid: read_parent_pid(entry),
            name,
            create_time_unix: windows_time_to_unix_seconds(
                read_i64(entry, offset_of!(SystemProcessInformation, create_time)).unwrap_or(0),
            ),
            cpu_time_nanos,
            working_set: read_i64(
                entry,
                offset_of!(SystemProcessInformation, working_set_size),
            )
            .map(|v| v.max(0) as u64),
            private_bytes: read_i64(
                entry,
                offset_of!(SystemProcessInformation, private_page_count),
            )
            .map(|v| v.max(0) as u64),
            thread_count: Some(read_u32(
                entry,
                offset_of!(SystemProcessInformation, number_of_threads),
            )),
            handle_count: Some(read_u32(
                entry,
                offset_of!(SystemProcessInformation, handle_count),
            )),
            io_read: read_i64(
                entry,
                offset_of!(SystemProcessInformation, read_transfer_count),
            )
            .map(|v| v.max(0) as u64),
            io_write: read_i64(
                entry,
                offset_of!(SystemProcessInformation, write_transfer_count),
            )
            .map(|v| v.max(0) as u64),
        });

        let next = read_u32(
            entry,
            offset_of!(SystemProcessInformation, next_entry_offset),
        );
        if next == 0 {
            break;
        }
        let next = next as usize;
        if next < std::mem::size_of::<SystemProcessInformation>() {
            return Err(mcpad_core::Error::Parse(
                "некорректная длина записи в буфере процессов".to_string(),
            ));
        }
        offset += next;
    }

    Ok(processes)
}

/// Превращает сырые записи ядра в модель ядра.
pub fn into_process_info(raw: RawProcess, boot_id: Option<u64>) -> ProcessInfo {
    let key = mcpad_core::ProcessKey::new(raw.pid, raw.create_time_unix, boot_id);
    let mut info = ProcessInfo::new(key, raw.name);

    info.parent_pid = (raw.parent_pid != 0).then_some(raw.parent_pid);
    // Признак защищённого процесса (PPL) живёт в `PROCESS_EXTENDED_BASIC_INFORMATION`
    // и требует запроса к каждому процессу отдельно. В горячем пути это дорого,
    // поэтому значение остаётся неизвестным, а не ложным.

    info.cpu_time_total = match raw.cpu_time_nanos {
        Some(value) => {
            mcpad_core::Counter::measured(value, mcpad_core::Source::WindowsSystemInformation)
        }
        None => mcpad_core::Counter::unavailable(mcpad_core::UnavailableReason::NotMeasured),
    };
    info.memory_working_set = match raw.working_set {
        Some(value) => {
            mcpad_core::Counter::measured(value, mcpad_core::Source::WindowsSystemInformation)
        }
        None => mcpad_core::Counter::unavailable(mcpad_core::UnavailableReason::NotMeasured),
    };
    info.memory_private = match raw.private_bytes {
        Some(value) => {
            mcpad_core::Counter::measured(value, mcpad_core::Source::WindowsSystemInformation)
        }
        None => mcpad_core::Counter::unavailable(mcpad_core::UnavailableReason::NotMeasured),
    };
    info.thread_count = match raw.thread_count {
        Some(value) => {
            mcpad_core::Counter::measured(value, mcpad_core::Source::WindowsSystemInformation)
        }
        None => mcpad_core::Counter::unavailable(mcpad_core::UnavailableReason::NotMeasured),
    };
    info.handle_count = match raw.handle_count {
        Some(value) => {
            mcpad_core::Counter::measured(value, mcpad_core::Source::WindowsSystemInformation)
        }
        None => mcpad_core::Counter::unavailable(mcpad_core::UnavailableReason::NotMeasured),
    };
    info.io_read_bytes = match raw.io_read {
        Some(value) => {
            mcpad_core::Counter::measured(value, mcpad_core::Source::WindowsSystemInformation)
        }
        None => mcpad_core::Counter::unavailable(mcpad_core::UnavailableReason::NotMeasured),
    };
    info.io_write_bytes = match raw.io_write {
        Some(value) => {
            mcpad_core::Counter::measured(value, mcpad_core::Source::WindowsSystemInformation)
        }
        None => mcpad_core::Counter::unavailable(mcpad_core::UnavailableReason::NotMeasured),
    };

    info
}

fn read_u32(buffer: &[u8], offset: usize) -> u32 {
    let end = offset + 4;
    if end > buffer.len() {
        return 0;
    }
    u32::from_le_bytes([
        buffer[offset],
        buffer[offset + 1],
        buffer[offset + 2],
        buffer[offset + 3],
    ])
}

fn read_i64(buffer: &[u8], offset: usize) -> Option<i64> {
    let end = offset + 8;
    if end > buffer.len() {
        return None;
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buffer[offset..end]);
    Some(i64::from_le_bytes(bytes))
}

fn read_pid(buffer: &[u8]) -> u32 {
    let offset = offset_of!(SystemProcessInformation, unique_process_id);
    if offset + 8 > buffer.len() {
        return 0;
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buffer[offset..offset + 8]);
    u64::from_le_bytes(bytes) as u32
}

fn read_parent_pid(buffer: &[u8]) -> u32 {
    let offset = offset_of!(SystemProcessInformation, inherited_from_unique_process_id);
    if offset + 8 > buffer.len() {
        return 0;
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buffer[offset..offset + 8]);
    u64::from_le_bytes(bytes) as u32
}

fn read_unicode_string(buffer: &[u8], offset: usize) -> Result<Option<String>, mcpad_core::Error> {
    if offset + 16 > buffer.len() {
        return Ok(None);
    }
    let length = u16::from_le_bytes([buffer[offset], buffer[offset + 1]]) as usize;
    let ptr_offset = offset + 8;
    let mut ptr_bytes = [0u8; 8];
    ptr_bytes.copy_from_slice(&buffer[ptr_offset..ptr_offset + 8]);
    let ptr = usize::from_le_bytes(ptr_bytes) as *const u16;

    if length == 0 || ptr.is_null() {
        return Ok(None);
    }
    Ok(utf16_to_string(ptr, length))
}

/// Системные вызовы ntdll, которых нет в привязках `windows`.
pub(crate) mod ntdll {
    use windows::Win32::Foundation::HANDLE;

    #[link(name = "ntdll")]
    unsafe extern "system" {
        pub fn NtQuerySystemInformation(
            class: u32,
            info: *mut core::ffi::c_void,
            info_length: u32,
            return_length: *mut u32,
        ) -> i32;

        pub fn NtQueryInformationProcess(
            process_handle: HANDLE,
            information_class: u32,
            information: *mut core::ffi::c_void,
            information_length: u32,
            return_length: *mut u32,
        ) -> i32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_documented_offsets() {
        // Смещения зафиксированы по документированному определению
        // SYSTEM_PROCESS_INFORMATION (phnt.h) для 64-битной Windows и сверены
        // с GetProcessMemoryInfo / GetProcessIoCounters на живой машине.
        assert_eq!(offset_of!(SystemProcessInformation, create_time), 0x20);
        assert_eq!(offset_of!(SystemProcessInformation, user_time), 0x28);
        assert_eq!(offset_of!(SystemProcessInformation, kernel_time), 0x30);
        assert_eq!(offset_of!(SystemProcessInformation, image_name), 0x38);
        assert_eq!(
            offset_of!(SystemProcessInformation, unique_process_id),
            0x50
        );
        assert_eq!(
            offset_of!(SystemProcessInformation, inherited_from_unique_process_id),
            0x58
        );
        assert_eq!(offset_of!(SystemProcessInformation, handle_count), 0x60);
        assert_eq!(offset_of!(SystemProcessInformation, session_id), 0x64);
        assert_eq!(
            offset_of!(SystemProcessInformation, peak_working_set_size),
            0x88
        );
        assert_eq!(offset_of!(SystemProcessInformation, working_set_size), 0x90);
        assert_eq!(offset_of!(SystemProcessInformation, pagefile_usage), 0xB8);
        assert_eq!(
            offset_of!(SystemProcessInformation, peak_pagefile_usage),
            0xC0
        );
        assert_eq!(
            offset_of!(SystemProcessInformation, private_page_count),
            0xC8
        );
        assert_eq!(
            offset_of!(SystemProcessInformation, read_transfer_count),
            0xE8
        );
        assert_eq!(
            offset_of!(SystemProcessInformation, write_transfer_count),
            0xF0
        );
        assert_eq!(
            offset_of!(SystemProcessInformation, other_transfer_count),
            0xF8
        );
    }

    #[test]
    fn unicode_string_pointer_is_at_offset_eight() {
        assert_eq!(offset_of!(UnicodeString, buffer), 8);
    }

    #[test]
    fn reads_fields_from_synthetic_buffer() {
        let mut buffer = vec![0u8; std::mem::size_of::<SystemProcessInformation>()];
        let pid_offset = offset_of!(SystemProcessInformation, unique_process_id);
        buffer[pid_offset..pid_offset + 8].copy_from_slice(&42u64.to_le_bytes());

        let handle_offset = offset_of!(SystemProcessInformation, handle_count);
        buffer[handle_offset..handle_offset + 4].copy_from_slice(&7u32.to_le_bytes());

        assert_eq!(read_pid(&buffer), 42);
        assert_eq!(
            read_u32(&buffer, offset_of!(SystemProcessInformation, handle_count)),
            7
        );
    }

    #[test]
    fn out_of_bounds_reads_are_safe() {
        let buffer = [0u8; 4];
        assert_eq!(read_u32(&buffer, 100), 0);
        assert_eq!(read_i64(&buffer, 100), None);
        assert_eq!(read_pid(&buffer), 0);
    }
}
