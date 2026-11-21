//! Разбор структур `libproc` — чистые функции над байтами.
//!
//! FFI живёт в модуле `sys` и собирается только под macOS. Разбор самих
//! структур вынесен сюда, чтобы он проверялся тестами на любой платформе.

/// Максимальная длина имени процесса (`MAXCOMLEN`).
pub const MAXCOMLEN: usize = 16;

/// Данные из `proc_bsdinfo`.
#[derive(Debug, Clone, PartialEq)]
pub struct BsdInfo {
    pub pid: u32,
    pub ppid: u32,
    pub uid: u32,
    pub gid: u32,
    pub name: String,
    pub comm: String,
    pub start_seconds: u64,
}

/// Данные из `proc_taskinfo`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TaskInfo {
    pub virtual_size: u64,
    pub resident_size: u64,
    /// Пользовательское время CPU, наносекунды.
    pub total_user: u64,
    /// Системное время CPU, наносекунды.
    pub total_system: u64,
    pub thread_count: i32,
}

fn u32_at(buffer: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let bytes = buffer.get(offset..end)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn u64_at(buffer: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(8)?;
    let bytes = buffer.get(offset..end)?;
    let mut array = [0u8; 8];
    array.copy_from_slice(bytes);
    Some(u64::from_le_bytes(array))
}

fn i32_at(buffer: &[u8], offset: usize) -> Option<i32> {
    u32_at(buffer, offset).map(|value| value as i32)
}

/// Читает C-строку из фиксированного поля.
fn c_string(buffer: &[u8], offset: usize, len: usize) -> String {
    let slice = buffer.get(offset..offset + len).unwrap_or(&[]);
    let end = slice
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).to_string()
}

/// Разбирает `proc_bsdinfo`.
pub fn parse_bsdinfo(buffer: &[u8]) -> Option<BsdInfo> {
    // Смещения — по `struct proc_bsdinfo` из `<libproc.h>`.
    let pid = u32_at(buffer, 0x0C)?;
    let ppid = u32_at(buffer, 0x10)?;
    let uid = u32_at(buffer, 0x14)?;
    let gid = u32_at(buffer, 0x18)?;
    let comm = c_string(buffer, 0x30, MAXCOMLEN);
    let name = c_string(buffer, 0x40, 2 * MAXCOMLEN);
    let start_seconds = u64_at(buffer, 0x78).unwrap_or(0);

    Some(BsdInfo {
        pid,
        ppid,
        uid,
        gid,
        name,
        comm,
        start_seconds,
    })
}

/// Разбирает `proc_taskinfo`.
pub fn parse_taskinfo(buffer: &[u8]) -> Option<TaskInfo> {
    // Смещения — по `struct proc_taskinfo` из `<libproc.h>`.
    Some(TaskInfo {
        virtual_size: u64_at(buffer, 0x00)?,
        resident_size: u64_at(buffer, 0x08)?,
        total_user: u64_at(buffer, 0x10)?,
        total_system: u64_at(buffer, 0x18)?,
        thread_count: i32_at(buffer, 0x50)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_bsdinfo() -> Vec<u8> {
        let mut buffer = vec![0u8; 0x88];
        buffer[0x0C..0x10].copy_from_slice(&4242u32.to_le_bytes());
        buffer[0x10..0x14].copy_from_slice(&7u32.to_le_bytes());
        buffer[0x14..0x18].copy_from_slice(&501u32.to_le_bytes());
        buffer[0x18..0x1C].copy_from_slice(&20u32.to_le_bytes());
        buffer[0x30..0x30 + 7].copy_from_slice(b"launchd");
        buffer[0x40..0x40 + 12].copy_from_slice(b"/sbin/launch");
        buffer[0x78..0x80].copy_from_slice(&1_700_000_000u64.to_le_bytes());
        buffer
    }

    fn synthetic_taskinfo() -> Vec<u8> {
        let mut buffer = vec![0u8; 0x5C];
        buffer[0x00..0x08].copy_from_slice(&1_048_576u64.to_le_bytes());
        buffer[0x08..0x10].copy_from_slice(&262_144u64.to_le_bytes());
        buffer[0x10..0x18].copy_from_slice(&1_000_000u64.to_le_bytes());
        buffer[0x18..0x20].copy_from_slice(&2_000_000u64.to_le_bytes());
        buffer[0x50..0x54].copy_from_slice(&9i32.to_le_bytes());
        buffer
    }

    #[test]
    fn parses_bsdinfo() {
        let info = parse_bsdinfo(&synthetic_bsdinfo()).expect("bsdinfo разобран");
        assert_eq!(info.pid, 4242);
        assert_eq!(info.ppid, 7);
        assert_eq!(info.uid, 501);
        assert_eq!(info.gid, 20);
        assert_eq!(info.comm, "launchd");
        assert_eq!(info.name, "/sbin/launch");
        assert_eq!(info.start_seconds, 1_700_000_000);
    }

    #[test]
    fn parses_taskinfo() {
        let info = parse_taskinfo(&synthetic_taskinfo()).expect("taskinfo разобран");
        assert_eq!(info.virtual_size, 1_048_576);
        assert_eq!(info.resident_size, 262_144);
        assert_eq!(info.total_user, 1_000_000);
        assert_eq!(info.total_system, 2_000_000);
        assert_eq!(info.thread_count, 9);
    }

    #[test]
    fn short_buffers_are_rejected() {
        assert!(parse_bsdinfo(&[0u8; 4]).is_none());
        assert!(parse_taskinfo(&[0u8; 4]).is_none());
    }

    #[test]
    fn names_stop_at_nul() {
        let mut buffer = vec![0u8; 0x88];
        buffer[0x40..0x44].copy_from_slice(b"abc\0");
        let info = parse_bsdinfo(&buffer).expect("bsdinfo разобран");
        assert_eq!(info.name, "abc");
    }
}
