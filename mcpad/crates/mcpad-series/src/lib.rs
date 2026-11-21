//! `mcpad-series` — история измерений.
//!
//! Графику нельзя рисовать по «последнему значению»: нужен ряд во времени.
//! Здесь живёт кольцевой буфер отсчётов и окно времени, к которому график
//! обращается.
//!
//! Важное правило: **пропуск измерения — это пропуск, а не ноль**. Если счётчик
//! в какой-то момент был недоступен, в ряду стоит `None`, и на графике это
//! разрыв, а не линия на нуле. Иначе график врал бы.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// Один отсчёт: время в миллисекундах Unix и значение.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub at_unix_ms: u64,
    /// `None` — измерение было недоступно.
    pub value: Option<f64>,
}

impl Sample {
    pub fn measured(at_unix_ms: u64, value: f64) -> Self {
        Self {
            at_unix_ms,
            value: Some(value),
        }
    }

    /// Отсчёт с пропуском: измерение в этот момент недоступно.
    pub fn gap(at_unix_ms: u64) -> Self {
        Self {
            at_unix_ms,
            value: None,
        }
    }
}

/// Сводка по ряду.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stats {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    /// Сколько отсчётов с измеренным значением.
    pub measured: usize,
    /// Сколько пропусков.
    pub gaps: usize,
}

/// Кольцевой буфер отсчётов.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    capacity: usize,
    samples: VecDeque<Sample>,
}

impl Series {
    /// Буфер на `capacity` отсчётов.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            samples: VecDeque::new(),
        }
    }

    /// Добавляет отсчёт; самый старый вытесняется при переполнении.
    pub fn push(&mut self, sample: Sample) {
        if self.samples.len() >= self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    /// Добавляет измеренное значение.
    pub fn push_value(&mut self, at_unix_ms: u64, value: f64) {
        self.push(Sample::measured(at_unix_ms, value));
    }

    /// Добавляет пропуск.
    pub fn push_gap(&mut self, at_unix_ms: u64) {
        self.push(Sample::gap(at_unix_ms));
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Все отсчёты.
    pub fn samples(&self) -> impl Iterator<Item = &Sample> {
        self.samples.iter()
    }

    /// Отсчёты окна `[from, to]`.
    pub fn window(&self, from_unix_ms: u64, to_unix_ms: u64) -> Vec<&Sample> {
        self.samples
            .iter()
            .filter(|sample| sample.at_unix_ms >= from_unix_ms && sample.at_unix_ms <= to_unix_ms)
            .collect()
    }

    /// Сводка по окну. Пропуски не участвуют в среднем, но учитываются в счётчике.
    pub fn stats(&self, from_unix_ms: u64, to_unix_ms: u64) -> Option<Stats> {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        let mut sum = 0.0;
        let mut measured = 0usize;
        let mut gaps = 0usize;

        for sample in self.window(from_unix_ms, to_unix_ms) {
            match sample.value {
                Some(value) => {
                    min = min.min(value);
                    max = max.max(value);
                    sum += value;
                    measured += 1;
                }
                None => gaps += 1,
            }
        }

        if measured == 0 {
            return None;
        }

        Some(Stats {
            min,
            max,
            avg: sum / measured as f64,
            measured,
            gaps,
        })
    }

    /// Последний измеренный отсчёт.
    pub fn last_value(&self) -> Option<f64> {
        self.samples.iter().rev().find_map(|sample| sample.value)
    }

    /// Значение самого свежего отсчёта.
    ///
    /// В отличие от [`Series::last_value`], здесь «известно раньше» не
    /// подменяет «неизвестно сейчас»: если последний отсчёт — пропуск, то
    /// результат `None`.
    pub fn latest_value(&self) -> Option<f64> {
        self.samples.back().and_then(|sample| sample.value)
    }

    /// Очищает буфер.
    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

impl Default for Series {
    fn default() -> Self {
        Series::with_capacity(600)
    }
}

/// Окно времени на графике.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeSpan {
    /// Минута.
    OneMinute,
    /// Две минуты.
    TwoMinutes,
    /// Пять минут.
    FiveMinutes,
}

impl TimeSpan {
    /// Длина окна в миллисекундах.
    pub fn millis(self) -> u64 {
        match self {
            TimeSpan::OneMinute => 60_000,
            TimeSpan::TwoMinutes => 120_000,
            TimeSpan::FiveMinutes => 300_000,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TimeSpan::OneMinute => "1 мин",
            TimeSpan::TwoMinutes => "2 мин",
            TimeSpan::FiveMinutes => "5 мин",
        }
    }

    /// Все варианты — для переключателя.
    pub fn all() -> [TimeSpan; 3] {
        [
            TimeSpan::OneMinute,
            TimeSpan::TwoMinutes,
            TimeSpan::FiveMinutes,
        ]
    }
}

/// Шкала времени: окно и курсор.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timeline {
    pub span: TimeSpan,
    /// Курсор на оси времени, миллисекунды Unix.
    pub cursor_unix_ms: Option<u64>,
}

impl Timeline {
    pub fn new(span: TimeSpan) -> Self {
        Self {
            span,
            cursor_unix_ms: None,
        }
    }

    /// Начало окна для момента `now`.
    pub fn start(&self, now_unix_ms: u64) -> u64 {
        now_unix_ms.saturating_sub(self.span.millis())
    }

    /// Конец окна для момента `now`.
    pub fn end(&self, now_unix_ms: u64) -> u64 {
        now_unix_ms
    }

    /// Шаг курсора при нажатии стрелки — 1% окна, но не меньше секунды.
    pub fn step(&self) -> u64 {
        (self.span.millis() / 100).max(1_000)
    }

    /// Двигает курсор на шаг вперёд или назад.
    pub fn move_cursor(&mut self, now_unix_ms: u64, forward: bool) {
        let step = self.step();
        let cursor = self.cursor_unix_ms.unwrap_or(now_unix_ms);
        let moved = if forward {
            cursor.saturating_add(step)
        } else {
            cursor.saturating_sub(step)
        };
        let start = self.start(now_unix_ms);
        let end = self.end(now_unix_ms);
        self.cursor_unix_ms = Some(moved.clamp(start, end));
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Timeline::new(TimeSpan::OneMinute)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_evicts_oldest() {
        let mut series = Series::with_capacity(3);
        for value in 0..5 {
            series.push_value(value, value as f64);
        }
        assert_eq!(series.len(), 3);
        let times: Vec<u64> = series.samples().map(|sample| sample.at_unix_ms).collect();
        assert_eq!(times, vec![2, 3, 4]);
    }

    #[test]
    fn gaps_are_not_zero() {
        let mut series = Series::with_capacity(10);
        series.push_value(1, 10.0);
        series.push_gap(2);
        series.push_value(3, 20.0);

        let stats = series.stats(0, 10).expect("сводка есть");
        assert_eq!(stats.measured, 2);
        assert_eq!(stats.gaps, 1);
        assert_eq!(stats.min, 10.0);
        assert_eq!(stats.max, 20.0);
        assert_eq!(stats.avg, 15.0);
    }

    #[test]
    fn stats_of_only_gaps_is_none() {
        let mut series = Series::with_capacity(4);
        series.push_gap(1);
        series.push_gap(2);
        assert!(series.stats(0, 10).is_none());
    }

    #[test]
    fn window_filters_by_time() {
        let mut series = Series::with_capacity(100);
        for time in 0..100 {
            series.push_value(time, time as f64);
        }
        let window = series.window(10, 20);
        assert_eq!(window.len(), 11);
        assert_eq!(window.first().unwrap().at_unix_ms, 10);
    }

    #[test]
    fn last_value_skips_gap() {
        let mut series = Series::with_capacity(4);
        series.push_value(1, 5.0);
        series.push_gap(2);
        assert_eq!(series.last_value(), Some(5.0));
    }

    #[test]
    fn latest_value_does_not_skip_gap() {
        let mut series = Series::with_capacity(4);
        series.push_value(1, 5.0);
        series.push_gap(2);
        // «Известно раньше» не подменяет «неизвестно сейчас».
        assert_eq!(series.latest_value(), None);
        series.push_value(3, 7.0);
        assert_eq!(series.latest_value(), Some(7.0));
    }

    #[test]
    fn timeline_window_covers_span() {
        let timeline = Timeline::new(TimeSpan::TwoMinutes);
        let now = 1_000_000u64;
        assert_eq!(timeline.end(now), now);
        assert_eq!(timeline.start(now), now - 120_000);
        assert_eq!(timeline.start(now) + timeline.span.millis(), now);
    }

    #[test]
    fn cursor_stays_inside_window() {
        let mut timeline = Timeline::new(TimeSpan::OneMinute);
        let now = 1_000_000u64;
        for _ in 0..200 {
            timeline.move_cursor(now, false);
        }
        assert!(timeline.cursor_unix_ms.unwrap() >= timeline.start(now));

        for _ in 0..200 {
            timeline.move_cursor(now, true);
        }
        assert!(timeline.cursor_unix_ms.unwrap() <= timeline.end(now));
    }

    #[test]
    fn cursor_step_is_at_least_a_second() {
        let timeline = Timeline::new(TimeSpan::OneMinute);
        assert_eq!(timeline.step(), 1_000);
    }
}
