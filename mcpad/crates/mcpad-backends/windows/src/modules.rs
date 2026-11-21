//! Перечисление загруженных модулей процесса (Windows).
//!
//! Список модулей (DLL) отдаёт `EnumProcessModules`, а путь к каждому —
//! `GetModuleFileNameExW`. Обе функции требуют открыть процесс с правами
//! `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ`; для защищённых процессов
//! доступ закрыт — это честная причина отказа, а не пустой список.
//!
//! Список модулей неполон по своей природе: ядро не отдаёт модули,
//! загруженные вручную (например, через `LoadLibrary` из памяти), и не
//! показывает модули, отображённые без файла на диске.

use mcpad_core::{Error, ModuleLink};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HMODULE};
use windows::Win32::System::ProcessStatus::{
    EnumProcessModules, GetModuleBaseNameW, GetModuleFileNameExW,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};

use crate::details::query_image_path;

/// Модули, загруженные в процесс.
pub fn module_links(pid: u32) -> Result<Vec<ModuleLink>, Error> {
    // SAFETY: права минимальные для перечисления модулей; дескриптор
    // проверяется вызывающим кодом и закрывается в конце функции.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) }
        .map_err(|error| Error::Io(format!("OpenProcess({pid}): {error}")))?;

    let result = collect(pid, handle);

    // SAFETY: дескриптор получен из OpenProcess и закрывается ровно один раз.
    let _ = unsafe { CloseHandle(handle) };
    result
}

/// Собирает список модулей по уже открытому процессу.
fn collect(pid: u32, handle: HANDLE) -> Result<Vec<ModuleLink>, Error> {
    let mut needed: u32 = 0;
    // SAFETY: нулевой буфер — штатный способ узнать нужный размер;
    // `needed` указывает на живую переменную.
    let _ = unsafe { EnumProcessModules(handle, std::ptr::null_mut(), 0, &mut needed) };
    if needed == 0 {
        return Err(Error::Io(format!(
            "EnumProcessModules({pid}): список модулей недоступен"
        )));
    }

    let module_size = std::mem::size_of::<HMODULE>();
    let mut modules = vec![HMODULE(std::ptr::null_mut()); (needed as usize) / module_size];
    let mut written: u32 = 0;
    // SAFETY: буфер выделен и живёт дольше вызова, длина указана явно и
    // соответствует фактическому размеру буфера.
    unsafe { EnumProcessModules(handle, modules.as_mut_ptr(), needed, &mut written) }
        .map_err(|error| Error::Io(format!("EnumProcessModules({pid}): {error}")))?;

    modules.truncate((written as usize) / module_size);

    let label = process_label(pid, handle);
    Ok(modules
        .iter()
        .map(|module| module_link(pid, &label, handle, *module))
        .collect())
}

/// Имя процесса для подписи связей.
fn process_label(pid: u32, handle: HANDLE) -> String {
    match query_image_path(handle) {
        Some(path) => ModuleLink::file_name(&path).to_string(),
        None => format!("pid {pid}"),
    }
}

/// Связь «процесс → модуль» по одному модулю.
fn module_link(pid: u32, label: &str, handle: HANDLE, module: HMODULE) -> ModuleLink {
    match module_path(handle, module) {
        Some(path) => {
            let name = ModuleLink::file_name(&path).to_string();
            ModuleLink::new(pid, label, name, Some(path))
        }
        None => {
            // Путь недоступен — значит «неизвестно», а не «пусто».
            let name =
                module_base_name(handle, module).unwrap_or_else(|| "модуль без имени".to_string());
            ModuleLink::new(pid, label, name, None)
        }
    }
}

/// Полный путь к модулю.
fn module_path(handle: HANDLE, module: HMODULE) -> Option<String> {
    let mut buffer = [0u16; 1024];
    // SAFETY: буфер выделен на стеке, длина берётся из размера среза.
    let len = unsafe { GetModuleFileNameExW(Some(handle), Some(module), &mut buffer) };
    if len == 0 {
        return None;
    }
    let text = String::from_utf16_lossy(&buffer[..len as usize]);
    let text = text.trim_end_matches('\0').to_string();
    (!text.is_empty()).then_some(text)
}

/// Имя модуля без пути — запасной вариант, когда путь недоступен.
fn module_base_name(handle: HANDLE, module: HMODULE) -> Option<String> {
    let mut buffer = [0u16; 256];
    // SAFETY: буфер выделен на стеке, длина берётся из размера среза.
    let len = unsafe { GetModuleBaseNameW(handle, Some(module), &mut buffer) };
    if len == 0 {
        return None;
    }
    let text = String::from_utf16_lossy(&buffer[..len as usize]);
    let text = text.trim_end_matches('\0').to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Текущий процесс — тот, у которого модули точно есть.
    #[test]
    fn current_process_has_modules() {
        let pid = std::process::id();
        let links = module_links(pid).expect("модули текущего процесса доступны");
        assert!(!links.is_empty(), "у процесса есть хотя бы один модуль");

        // В списке есть образ самого процесса и ядро Windows. Имя регистра
        // не гарантировано: система пишет и «KERNEL32.DLL», и «kernel32.dll».
        let names: Vec<String> = links
            .iter()
            .map(|link| link.module_label.to_lowercase())
            .collect();
        assert!(
            names.iter().any(|name| name.contains("kernel32")),
            "ожидался kernel32, получили: {names:?}"
        );

        // У модулей есть путь, а подпись процесса — не пустая.
        assert!(links.iter().all(|link| !link.module_label.is_empty()));
        assert!(links.iter().any(|link| link.path.is_some()));
    }

    /// Несуществующий процесс — честная ошибка, а не пустой список.
    #[test]
    fn missing_process_is_an_error() {
        let result = module_links(0xFFFF_FFFE);
        assert!(result.is_err(), "несуществующий процесс должен дать ошибку");
    }
}
