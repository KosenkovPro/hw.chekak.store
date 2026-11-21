//! Подробности процесса по запросу: путь к исполняемому файлу, командная
//! строка, владелец.
//!
//! Этот путь дорогой: он открывает процесс и читает его токен. Поэтому он
//! вызывается из `mcpad info <pid>`, а не из сэмплера.

use mcpad_core::{ProcessDetails, ProcessKey};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{
    GetTokenInformation, LookupAccountSidW, SID_NAME_USE, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows::Win32::System::Threading::{
    OpenProcess, OpenProcessToken, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::core::PWSTR;

use crate::system::{UnicodeString, ntdll};

/// Класс `ProcessCommandLineInformation` для `NtQueryInformationProcess`.
const PROCESS_COMMAND_LINE_INFORMATION: u32 = 60;

/// `STATUS_INFO_LENGTH_MISMATCH` — ядру нужен буфер большего размера.
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004u32 as i32;

const PROCESS_NAME_FORMAT_WIN32: PROCESS_NAME_FORMAT = PROCESS_NAME_FORMAT(0);

/// Собирает подробности процесса. Отсутствующие поля остаются `None` —
/// это «нет данных», а не «пусто».
pub fn query_details(pid: u32) -> ProcessDetails {
    let mut details = ProcessDetails {
        key: ProcessKey::from_pid(pid),
        exe_path: None,
        command_line: None,
        user: None,
    };

    let Ok(handle) = open_process(pid) else {
        return details;
    };

    details.exe_path = query_image_path(handle);
    details.command_line = query_command_line(handle);
    details.user = query_owner(handle);

    // SAFETY: дескриптор получен из OpenProcess и закрывается ровно один раз.
    let _ = unsafe { CloseHandle(handle) };
    details
}

fn open_process(pid: u32) -> Result<HANDLE, ()> {
    // Хватает минимальных прав: и путь к образу, и командная строка, и токен
    // читаются с PROCESS_QUERY_LIMITED_INFORMATION. Просить больше прав не
    // нужно — иначе доступ к части процессов закрывался бы зря.
    let access = PROCESS_QUERY_LIMITED_INFORMATION;
    // SAFETY: права запрошены минимальные, дескриптор проверяется вызывающим кодом.
    unsafe { OpenProcess(access, false, pid) }.map_err(|_| ())
}

/// Полный путь к образу процесса по открытому дескриптору.
pub(crate) fn query_image_path(handle: HANDLE) -> Option<String> {
    let mut buffer = [0u16; 1024];
    let mut size = buffer.len() as u32;
    // SAFETY: буфер выделен на стеке, длина передаётся явно, указатель валиден.
    unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
    }
    .ok()?;
    let text = String::from_utf16_lossy(&buffer[..size as usize]);
    let text = text.trim_end_matches('\0').to_string();
    (!text.is_empty()).then_some(text)
}

fn query_command_line(handle: HANDLE) -> Option<String> {
    // Сначала пробуем скромный буфер, при нехватке места — запрашиваем размер
    // у ядра и повторяем с нужным объёмом.
    let mut capacity = 4096usize;
    let mut buffer = vec![0u8; capacity];
    let mut returned: u32 = 0;

    for attempt in 0..2 {
        // SAFETY: буфер выделен и живёт дольше вызова; длина указана явно.
        let status = unsafe {
            ntdll::NtQueryInformationProcess(
                handle,
                PROCESS_COMMAND_LINE_INFORMATION,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut returned,
            )
        };

        if status == 0 {
            break;
        }

        // STATUS_INFO_LENGTH_MISMATCH: ядро сообщило нужный размер.
        if status == STATUS_INFO_LENGTH_MISMATCH
            && attempt == 0
            && (returned as usize) > buffer.len()
        {
            capacity = (returned as usize).min(1024 * 1024);
            buffer = vec![0u8; capacity];
            returned = 0;
            continue;
        }

        return None;
    }

    if (returned as usize) < std::mem::size_of::<UnicodeString>() {
        return None;
    }

    // SAFETY: в начале буфера лежит UNICODE_STRING, длина возврата проверена
    // выше. Читаем без выравнивания: буфер выровнен по байту, а в структуре
    // есть указатель.
    let unicode = unsafe { std::ptr::read_unaligned(buffer.as_ptr() as *const UnicodeString) };
    let length = unicode.length as usize;
    if length == 0 || unicode.buffer.is_null() || !length.is_multiple_of(2) {
        return None;
    }

    // Ядро копирует строку в наш буфер и указывает на неё указателем, поэтому
    // читаем из своего буфера, а не по адресу из ядра.
    let base = buffer.as_ptr() as usize;
    let start = unicode.buffer as usize;
    if start < base || start + length > base + buffer.len() {
        return None;
    }
    let offset = start - base;

    let mut words = Vec::with_capacity(length / 2);
    for index in 0..length / 2 {
        let position = offset + index * 2;
        words.push(u16::from_le_bytes([buffer[position], buffer[position + 1]]));
    }
    Some(String::from_utf16_lossy(&words))
}

fn query_owner(handle: HANDLE) -> Option<String> {
    let mut token: HANDLE = HANDLE(std::ptr::null_mut());
    // SAFETY: токен открыт только для чтения и закрывается ниже.
    unsafe { OpenProcessToken(handle, TOKEN_QUERY, &mut token) }.ok()?;

    let mut buffer = vec![0u8; 1024];
    let mut returned: u32 = 0;
    // SAFETY: буфер выделен, длина указана, указатель передаётся как Option<*mut c_void>.
    let status = unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            buffer.len() as u32,
            &mut returned,
        )
    };
    if status.is_err() || (returned as usize) < std::mem::size_of::<TOKEN_USER>() {
        // SAFETY: токен открыт выше, закрывается здесь при раннем выходе.
        let _ = unsafe { CloseHandle(token) };
        return None;
    }

    // SAFETY: длина возврата проверена; читаем без выравнивания, потому что
    // буфер выровнен по байту, а в структуре есть указатель.
    let user = unsafe { std::ptr::read_unaligned(buffer.as_ptr() as *const TOKEN_USER) };
    let name = lookup_account_name(&user);

    // SAFETY: токен открыт выше, закрывается ровно один раз.
    let _ = unsafe { CloseHandle(token) };
    name
}

fn lookup_account_name(user: &TOKEN_USER) -> Option<String> {
    // Имя и домен могут не поместиться: тогда API требует большего буфера.
    // Поэтому начинаем с 256 символов и при нехватке места пробуем вдвое
    // больше — до четырёх попыток.
    let mut capacity = 256usize;

    for _ in 0..4 {
        let mut name_buffer = vec![0u16; capacity];
        let mut name_len = name_buffer.len() as u32;
        let mut domain_buffer = vec![0u16; capacity];
        let mut domain_len = domain_buffer.len() as u32;
        // API пишет в этот указатель тип имени, поэтому передать NULL нельзя:
        // на практике это приводит к падению. Документация называет параметр
        // необязательным, но ведёт себя иначе.
        let mut name_use = SID_NAME_USE(0);

        // SAFETY: буферы имени и домена выделены и живут до конца вызова,
        // длины передаются явно и соответствуют буферам, `name_use` — указатель
        // на живую переменную.
        let result = unsafe {
            LookupAccountSidW(
                None,
                user.User.Sid,
                Some(PWSTR(name_buffer.as_mut_ptr())),
                &mut name_len,
                Some(PWSTR(domain_buffer.as_mut_ptr())),
                &mut domain_len,
                &mut name_use,
            )
        };

        if result.is_ok() {
            let name_len = (name_len as usize).min(name_buffer.len());
            let domain_len = (domain_len as usize).min(domain_buffer.len());
            let name = String::from_utf16_lossy(&name_buffer[..name_len]);
            let domain = String::from_utf16_lossy(&domain_buffer[..domain_len]);
            let name = name.trim_end_matches('\0').to_string();
            let domain = domain.trim_end_matches('\0').to_string();

            if name.is_empty() {
                return None;
            }
            return if domain.is_empty() {
                Some(name)
            } else {
                Some(format!("{domain}\\{name}"))
            };
        }

        capacity *= 2;
    }

    None
}
