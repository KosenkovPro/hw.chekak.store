//! Управление процессами: завершение и приоритет.
//!
//! Завершение — это обычный `SIGTERM`: процесс получает шанс завершиться
//! сам, как в `kill` и в диспетчерах задач Linux и macOS. Приоритет —
//! обычное `nice`, где меньше значит выше.

use mcpad_core::{Error, Priority};

/// Завершает процесс.
pub fn terminate(pid: u32) -> Result<(), Error> {
    // SAFETY: kill с корректным pid и SIGTERM; сигнал безопасен для любого
    // процесса, который пользователь имеет право завершить.
    let status = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    if status == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        return Err(Error::NotFound(format!("процесс {pid} не найден")));
    }
    if error.raw_os_error() == Some(libc::EPERM) {
        return Err(Error::PermissionDenied(format!(
            "нет прав на процесс {pid}"
        )));
    }
    Err(Error::Io(format!("kill({pid}): {error}")))
}

/// Меняет приоритет процесса.
pub fn set_priority(pid: u32, priority: Priority) -> Result<(), Error> {
    let nice = match priority {
        Priority::Low => 19,
        Priority::BelowNormal => 10,
        Priority::Normal => 0,
        Priority::AboveNormal => -5,
        Priority::High => -10,
        Priority::RealTime => -20,
    };
    // SAFETY: setpriority с корректным pid и значением nice в диапазоне
    // [-20, 19]; при нехватке прав вернётся ошибка, а не изменение.
    // Второй аргумент — `id_t`, и на glibc, и на musl это беззнаковый u32,
    // поэтому pid передаётся как есть.
    let status = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid, nice) };
    if status == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::EPERM) {
        return Err(Error::PermissionDenied(format!(
            "нет прав изменить приоритет {pid}: {error}"
        )));
    }
    if error.raw_os_error() == Some(libc::ESRCH) {
        return Err(Error::NotFound(format!("процесс {pid} не найден")));
    }
    Err(Error::Io(format!("setpriority({pid}): {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminating_unknown_process_fails_with_reason() {
        let error = terminate(u32::MAX).expect_err("процесса не существует");
        assert!(matches!(
            error,
            Error::Io(_) | Error::PermissionDenied(_) | Error::NotFound(_)
        ));
    }

    #[test]
    fn priority_change_of_unknown_process_fails() {
        let error = set_priority(u32::MAX, Priority::Normal).expect_err("процесса не существует");
        assert!(matches!(
            error,
            Error::Io(_) | Error::PermissionDenied(_) | Error::NotFound(_)
        ));
    }
}
