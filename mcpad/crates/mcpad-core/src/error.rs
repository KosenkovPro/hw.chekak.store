//! Ошибки ядра.

use std::fmt;

/// Ошибка ядра и проб.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Платформа не поддерживает запрос.
    Unsupported(String),
    /// Не хватает прав.
    PermissionDenied(String),
    /// Объект не найден.
    NotFound(String),
    /// Ошибка чтения данных ОС.
    Io(String),
    /// Данные не разобраны.
    Parse(String),
    /// Проб не смог собрать снимок.
    Probe(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unsupported(what) => write!(f, "unsupported: {what}"),
            Error::PermissionDenied(what) => write!(f, "permission denied: {what}"),
            Error::NotFound(what) => write!(f, "not found: {what}"),
            Error::Io(what) => write!(f, "io error: {what}"),
            Error::Parse(what) => write!(f, "parse error: {what}"),
            Error::Probe(what) => write!(f, "probe error: {what}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::Io(value.to_string())
    }
}
