//! `mcpad-sampler` — владелец системных запросов.
//!
//! Сэмплер единственный, кто обращается к ОС: GUI, CLI, MCP и удалённый клиент
//! читают один и тот же кэш. Проценты CPU считаются здесь, из дельт между
//! соседними снимками, — правило одно для всех платформ.

pub mod rates;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mcpad_core::{Error, Snapshot};
use mcpad_probe::PlatformProbe;

/// Состояние сэмплера между опросами.
#[derive(Default)]
struct State {
    last_snapshot: Option<Arc<Snapshot>>,
    last_instant: Option<Instant>,
}

/// Сэмплер: держит проб, снимает снимки и считает дельты.
pub struct Sampler {
    probe: Box<dyn PlatformProbe>,
    state: Mutex<State>,
}

impl Sampler {
    pub fn new(probe: Box<dyn PlatformProbe>) -> Self {
        Self {
            probe,
            state: Mutex::new(State::default()),
        }
    }

    /// Проб, из которого сэмплер берёт данные.
    pub fn probe(&self) -> &dyn PlatformProbe {
        self.probe.as_ref()
    }

    /// Снимает свежий снимок и обновляет доли CPU.
    pub fn refresh(&self) -> Result<Arc<Snapshot>, Error> {
        let instant = Instant::now();
        let mut snapshot = self.probe.collect()?;

        {
            let mut state = self
                .state
                .lock()
                .expect("состояние сэмплера не заблокировано");
            if let (Some(previous), Some(last_instant)) =
                (state.last_snapshot.as_deref(), state.last_instant)
            {
                let wall = instant.saturating_duration_since(last_instant);
                rates::apply_cpu_percent(&mut snapshot, previous, wall);
            }
            state.last_snapshot = Some(Arc::new(snapshot.clone()));
            state.last_instant = Some(instant);
        }

        Ok(Arc::new(snapshot))
    }

    /// Последний снимок без обращения к ОС.
    pub fn current(&self) -> Option<Arc<Snapshot>> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.last_snapshot.clone())
    }

    /// Сколько прошло с последнего снимка — для отметки «данные устарели».
    pub fn age(&self) -> Option<Duration> {
        let state = self.state.lock().ok()?;
        state.last_instant.map(|instant| instant.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpad_core::{ProcessInfo, ProcessKey, SystemInfo};
    use mcpad_probe::{Capability, Platform};

    /// Проб с растущим счётчиком CPU — эмулирует нагрузку.
    struct GrowingProbe;

    impl PlatformProbe for GrowingProbe {
        fn platform(&self) -> Platform {
            Platform::Windows
        }

        fn capabilities(&self) -> &'static [Capability] {
            &[Capability::ProcessList, Capability::CpuTime]
        }

        fn collect(&self) -> Result<Snapshot, Error> {
            let mut process = ProcessInfo::new(ProcessKey::new(1, Some(1), Some(1)), "busy");
            process.cpu_time_total = mcpad_core::Counter::measured(0, mcpad_core::Source::Derived);

            let system = SystemInfo {
                logical_cpus: 4,
                ..SystemInfo::default()
            };

            Ok(Snapshot::new(system, vec![process]))
        }
    }

    #[test]
    fn refresh_produces_snapshot() {
        let sampler = Sampler::new(Box::new(GrowingProbe));
        let snapshot = sampler.refresh().expect("снимок получен");
        assert_eq!(snapshot.processes.len(), 1);
        assert!(sampler.current().is_some());
    }

    #[test]
    fn first_refresh_has_no_cpu_percent() {
        let sampler = Sampler::new(Box::new(GrowingProbe));
        let snapshot = sampler.refresh().expect("снимок получен");
        // Для первого снимка дельт нет — процент неизвестен, а не нулевой.
        assert!(snapshot.processes[0].cpu_percent.is_unavailable());
    }
}
