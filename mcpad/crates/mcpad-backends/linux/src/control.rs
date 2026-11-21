//! Управление процессами: завершение и приоритет.
//!
//! Завершение — это обычный `SIGTERM`: процесс получает шанс завершиться
//! сам, как в `kill` и в диспетчерах задач Linux и macOS. Приоритет —
//! обычное `nice`, где меньше значит выше.

use mcpad_core::{Error, Priority};

/// Проверяет PID перед тем, как сигналить ему.
///
/// `kill(0, …)` бьёт по всей группе процессов, а `kill(-1, …)` — по всем
/// процессам пользователя. В снимке таких PID не бывает, но проверка
/// обязательна: одна ошибка в преобразовании — и сигнал уходит не туда.
fn checked_pid(pid: u32) -> Result<u32, Error> {
    if pid == 0 {
        return Err(Error::Io(
            "PID 0 означает всю группу процессов — такой завершать нельзя".to_string(),
        ));
    }
    if pid > i32::MAX as u32 {
        return Err(Error::Io(format!("PID {pid} вне диапазона")));
    }
    Ok(pid)
}

/// Завершает процесс.
pub fn terminate(pid: u32) -> Result<(), Error> {
    let pid = checked_pid(pid)?;
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
    let pid = checked_pid(pid)?;
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

    /// PID, которого заведомо не существует: максимум pid_t заведомо выше
    /// любого реального PID (на Linux `pid_max` ≤ 4194304, на macOS ≤ 99999).
    const MISSING_PID: u32 = i32::MAX as u32;

    #[test]
    fn terminating_unknown_process_fails_with_reason() {
        let error = terminate(MISSING_PID).expect_err("процесса не существует");
        assert!(matches!(
            error,
            Error::Io(_) | Error::PermissionDenied(_) | Error::NotFound(_)
        ));
    }

    #[test]
    fn priority_change_of_unknown_process_fails() {
        let error =
            set_priority(MISSING_PID, Priority::Normal).expect_err("процесса не существует");
        assert!(matches!(
            error,
            Error::Io(_) | Error::PermissionDenied(_) | Error::NotFound(_)
        ));
    }

    #[test]
    fn group_and_broadcast_pids_are_refused() {
        // PID 0 — это вся группа процессов, а u32::MAX превращается в -1 —
        // это «все процессы пользователя». Ни то, ни другое завершать нельзя.
        assert!(terminate(0).is_err());
        assert!(terminate(u32::MAX).is_err());
        assert!(set_priority(0, Priority::Normal).is_err());
        assert!(set_priority(u32::MAX, Priority::Normal).is_err());
    }
}
