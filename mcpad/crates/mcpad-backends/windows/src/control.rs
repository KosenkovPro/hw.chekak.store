//! Управление процессами: завершение и класс приоритета.
//!
//! Действия те же, что и в Диспетчере задач: «Снять задачу» и выбор
//! приоритета. Прав на чужой процесс может не быть — тогда операция
//! возвращает ошибку с причиной, а не «ничего не произошло».

use mcpad_core::{Error, Priority};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS,
    IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, OpenProcess, PROCESS_SET_INFORMATION,
    PROCESS_TERMINATE, REALTIME_PRIORITY_CLASS, SetPriorityClass, TerminateProcess,
};

/// HRESULT для `ERROR_ACCESS_DENIED`.
const ACCESS_DENIED: i32 = -2147024891;

/// Завершает процесс.
pub fn terminate(pid: u32) -> Result<(), Error> {
    // PID 0 — это «System Idle Process», его завершить нельзя.
    if pid == 0 {
        return Err(Error::Io(
            "PID 0 — это системный процесс, его завершать нельзя".to_string(),
        ));
    }
    // SAFETY: OpenProcess с PROCESS_TERMINATE только открывает дескриптор и
    // сам процесс не меняет.
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) }
        .map_err(|error| open_error(pid, error))?;
    // SAFETY: дескриптор открыт выше и передаётся API по правилам Windows.
    let result = unsafe { TerminateProcess(handle, 1) };
    // SAFETY: дескриптор открыт здесь же и закрывается ровно один раз.
    let _ = unsafe { CloseHandle(handle) };
    result.map_err(|error| Error::Io(format!("TerminateProcess({pid}): {error}")))
}

/// Меняет класс приоритета процесса.
pub fn set_priority(pid: u32, priority: Priority) -> Result<(), Error> {
    // SAFETY: OpenProcess с PROCESS_SET_INFORMATION открывает дескриптор для
    // смены приоритета и сам процесс не меняет.
    let handle = unsafe { OpenProcess(PROCESS_SET_INFORMATION, false, pid) }
        .map_err(|error| open_error(pid, error))?;
    // SAFETY: дескриптор открыт выше и передаётся API по правилам Windows.
    let result = unsafe { SetPriorityClass(handle, class(priority)) };
    // SAFETY: дескриптор открыт здесь же и закрывается ровно один раз.
    let _ = unsafe { CloseHandle(handle) };
    result.map_err(|error| Error::Io(format!("SetPriorityClass({pid}): {error}")))
}

/// Класс приоритета Windows.
fn class(priority: Priority) -> windows::Win32::System::Threading::PROCESS_CREATION_FLAGS {
    match priority {
        Priority::Low => IDLE_PRIORITY_CLASS,
        Priority::BelowNormal => BELOW_NORMAL_PRIORITY_CLASS,
        Priority::Normal => NORMAL_PRIORITY_CLASS,
        Priority::AboveNormal => ABOVE_NORMAL_PRIORITY_CLASS,
        Priority::High => HIGH_PRIORITY_CLASS,
        Priority::RealTime => REALTIME_PRIORITY_CLASS,
    }
}

/// Ошибка открытия процесса.
///
/// «Доступ запрещён» — это не поломка, а недостаток прав: так и говорим.
fn open_error(pid: u32, error: windows::core::Error) -> Error {
    if error.code().0 == ACCESS_DENIED {
        Error::PermissionDenied(format!("нет прав на процесс {pid}"))
    } else {
        Error::Io(format!("OpenProcess({pid}): {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_classes_are_distinct() {
        let classes = Priority::ALL.map(class);
        let mut unique = classes.to_vec();
        unique.sort_by_key(|value| value.0);
        unique.dedup();
        assert_eq!(unique.len(), Priority::ALL.len());
    }

    #[test]
    fn terminating_unknown_process_fails_with_reason() {
        // PID, которого почти наверняка нет: операция обязана вернуть ошибку
        // с причиной, а не «успех».
        let error = terminate(u32::MAX).expect_err("процесса не существует");
        assert!(matches!(
            error,
            Error::Io(_) | Error::PermissionDenied(_) | Error::NotFound(_)
        ));
    }

    #[test]
    fn idle_process_is_refused() {
        assert!(terminate(0).is_err());
        assert!(set_priority(0, Priority::Normal).is_err());
    }
}
