//! `mcpad-core` — единое ядро модели данных MCPad.
//!
//! Ядро не зависит ни от GUI, ни от терминала, ни от конкретной ОС: только
//! `serde` и `std`. Здесь живут:
//!
//! * [`Counter`] — значение с provenance: источник, уровень уверенности и
//!   причина недоступности. Неизвестное значение — это `None`, а не ноль.
//! * [`ProcessKey`] — устойчивая идентичность процесса, переживающая
//!   переиспользование PID.
//! * [`Snapshot`] — снимок системы: системная сводка плюс список процессов.
//!
//! Правило проекта: **неизвестное значение никогда не подменяется нулём**.
//! Если счётчик недоступен, у него есть причина — и она видна в UI, в JSON и
//! в CLI.

pub mod counter;
pub mod error;
pub mod module;
pub mod process;
pub mod snapshot;
pub mod system;
pub mod tree;
pub mod units;

pub use counter::{Confidence, Counter, Source, UnavailableReason};
pub use error::Error;
pub use module::ModuleLink;
pub use process::{Priority, ProcessDetails, ProcessInfo, ProcessKey};
pub use snapshot::{SCHEMA_VERSION, Snapshot};
pub use system::SystemInfo;
pub use tree::ProcessTree;
