//! Фильтр процессов: выражения в духе System Informer.
//!
//! Фильтр — обычная строка в поле поиска. Пустая строка означает «показывать
//! всё». Иначе строка разбирается на условия:
//!
//! | Пример | Что означает |
//! |---|---|
//! | `rustc` | имя процесса содержит «rustc» |
//! | `name:rustc` | то же, но поле названо явно |
//! | `имя:rustc` | русский синоним поля |
//! | `pid:1234` | PID равен 1234 |
//! | `pid:>1000` | PID больше 1000 |
//! | `cpu:>5` | доля CPU больше 5 % |
//! | `mem:>100` | рабочий набор больше 100 МБ |
//! | `threads:>4` | больше четырёх потоков |
//! | `handles:>100` | больше сотни дескрипторов |
//! | `not svchost` | отрицание следующего условия |
//! | `a and b` | оба условия |
//! | `a or b` | хотя бы одно |
//! | `что:то` | неизвестное поле: значение ищется как обычный текст |
//!
//! Условия без оператора объединяются по «и». Значение в кавычках может
//! содержать пробелы: `"Microsoft Edge"`.
//!
//! Главное правило то же, что и везде в проекте: **неизвестное значение не
//! проходит условие**. Процесс без измеренного CPU не попадает в `cpu:>0` —
//! иначе «нет данных» выглядело бы как «ноль».

use mcpad_core::ProcessInfo;

/// Поле, по которому идёт сравнение.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// Имя образа.
    Name,
    /// Идентификатор процесса.
    Pid,
    /// Доля CPU, проценты.
    Cpu,
    /// Рабочий набор, мегабайты.
    Memory,
    /// Число потоков.
    Threads,
    /// Число дескрипторов.
    Handles,
}

impl Field {
    /// Разбирает имя поля. Понимает английские и русские синонимы.
    pub fn parse(name: &str) -> Option<Self> {
        // to_lowercase, а не to_ascii_lowercase: кириллица в нижний регистр
        // переходит только так, иначе «ИМЯ:rust» не найдёт ничего.
        match name.trim().to_lowercase().as_str() {
            "name" | "имя" | "process" | "процесс" => Some(Field::Name),
            "pid" | "пид" => Some(Field::Pid),
            "cpu" | "процессор" => Some(Field::Cpu),
            "mem" | "memory" | "память" => Some(Field::Memory),
            "threads" | "потоки" | "thread" => Some(Field::Threads),
            "handles" | "дескрипторы" | "handle" => Some(Field::Handles),
            _ => None,
        }
    }

    /// Числовое значение поля процесса; `None` — не измерено.
    fn number(self, process: &ProcessInfo) -> Option<f64> {
        match self {
            Field::Pid => Some(process.key.pid as f64),
            Field::Cpu => process.cpu_percent.value,
            Field::Memory => process
                .memory_working_set
                .value
                .map(|value| value as f64 / 1_048_576.0),
            Field::Threads => process.thread_count.value.map(f64::from),
            Field::Handles => process.handle_count.value.map(f64::from),
            Field::Name => None,
        }
    }
}

/// Сравнение.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
    /// Равно.
    Equal,
    /// Больше.
    Greater,
    /// Больше или равно.
    GreaterOrEqual,
    /// Меньше.
    Less,
    /// Меньше или равно.
    LessOrEqual,
}

/// Значение условия.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Текст.
    Text(String),
    /// Число.
    Number(f64),
}

/// Условие по полю.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    /// Поле.
    pub field: Field,
    /// Сравнение.
    pub comparison: Comparison,
    /// Значение.
    pub value: Value,
}

/// Разобранное выражение.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Все условия должны выполняться.
    All(Vec<Expr>),
    /// Хотя бы одно условие должно выполняться.
    Any(Vec<Expr>),
    /// Отрицание.
    Not(Box<Expr>),
    /// Условие по полю.
    Condition(Condition),
    /// Свободный текст: вхождение в имя или в PID.
    Text(String),
}

impl Expr {
    /// Выполняется ли условие для процесса.
    pub fn matches(&self, process: &ProcessInfo) -> bool {
        match self {
            Expr::All(items) => items.iter().all(|item| item.matches(process)),
            Expr::Any(items) => items.iter().any(|item| item.matches(process)),
            Expr::Not(inner) => !inner.matches(process),
            Expr::Text(text) => {
                let name = process.name.to_lowercase();
                name.contains(text) || process.key.pid.to_string().contains(text)
            }
            Expr::Condition(condition) => condition.matches(process),
        }
    }
}

impl Condition {
    /// Выполняется ли условие для процесса.
    pub fn matches(&self, process: &ProcessInfo) -> bool {
        match &self.value {
            Value::Text(text) => {
                if self.field != Field::Name || self.comparison != Comparison::Equal {
                    // Текстовое сравнение осмысленно только для имени.
                    return false;
                }
                process.name.to_lowercase().contains(text)
            }
            Value::Number(number) => {
                // Неизвестное значение не проходит условие: «нет данных» — не «ноль».
                let Some(actual) = self.field.number(process) else {
                    return false;
                };
                match self.comparison {
                    Comparison::Equal => actual == *number,
                    Comparison::Greater => actual > *number,
                    Comparison::GreaterOrEqual => actual >= *number,
                    Comparison::Less => actual < *number,
                    Comparison::LessOrEqual => actual <= *number,
                }
            }
        }
    }
}

/// Быстрый фильтр — кнопка в панели инструментов.
///
/// Такие кнопки есть в RustWTM («High CPU», «High memory»): они отвечают на
/// вопрос «что сейчас ест ресурсы», не заставляя вспоминать синтаксис.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuickFilter {
    /// Процессы с заметной долей CPU.
    HighCpu,
    /// Процессы с заметным рабочим набором.
    HighMemory,
}

impl QuickFilter {
    /// Все быстрые фильтры.
    pub fn all() -> [QuickFilter; 2] {
        [QuickFilter::HighCpu, QuickFilter::HighMemory]
    }

    /// Порог высокой загрузки CPU, проценты.
    pub const HIGH_CPU_PERCENT: f64 = 5.0;

    /// Порог высокой памяти, мегабайты.
    pub const HIGH_MEMORY_MB: f64 = 512.0;

    /// Проходит ли процесс под быстрый фильтр.
    pub fn matches(self, process: &ProcessInfo) -> bool {
        match self {
            QuickFilter::HighCpu => process
                .cpu_percent
                .value
                .is_some_and(|value| value >= Self::HIGH_CPU_PERCENT),
            QuickFilter::HighMemory => process
                .memory_working_set
                .value
                .is_some_and(|value| value as f64 / 1_048_576.0 >= Self::HIGH_MEMORY_MB),
        }
    }
}

/// Разобранный фильтр.
///
/// PartialEq не выводится: скомпилированное регулярное выражение сравнивать
/// не с чем, а сравнивать фильтры по смыслу всё равно нужно по результату
/// проверки.
#[derive(Debug, Clone)]
pub struct Filter {
    expr: Option<Expr>,
    regex: Option<regex::Regex>,
    /// Ошибка разбора: неверное регулярное выражение.
    error: Option<String>,
}

/// Режим поиска.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Обычный текст и выражения по полям.
    Text,
    /// Регулярное выражение — как в Task Explorer и System Informer.
    Regex,
}

impl Mode {
    /// Все режимы.
    pub fn all() -> [Mode; 2] {
        [Mode::Text, Mode::Regex]
    }
}

impl Filter {
    /// Разбирает строку фильтра в режиме обычного текста.
    pub fn parse(text: &str) -> Self {
        Self::parse_with_mode(text, Mode::Text)
    }

    /// Разбирает строку фильтра в выбранном режиме.
    ///
    /// Разбор никогда не падает: непонятное условие трактуется как свободный
    /// текст, чтобы опечатка не молча скрыла половину процессов. Неверное
    /// регулярное выражение — исключение: там ошибку показывают, а не
    /// подменяют своим пониманием.
    pub fn parse_with_mode(text: &str, mode: Mode) -> Self {
        match mode {
            Mode::Text => {
                let tokens = tokenize(text);
                if tokens.is_empty() {
                    return Self {
                        expr: None,
                        regex: None,
                        error: None,
                    };
                }
                let mut parser = Parser { tokens, pos: 0 };
                let expr = parser.parse_expr().unwrap_or(Expr::All(Vec::new()));
                Self {
                    expr: Some(expr),
                    regex: None,
                    error: None,
                }
            }
            Mode::Regex => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    return Self {
                        expr: None,
                        regex: None,
                        error: None,
                    };
                }
                // Регистр не важен: обычный текст тоже ищется без учёта
                // регистра, и переключать это отдельной галочкой незачем.
                let pattern = format!("(?i){trimmed}");
                match regex::Regex::new(&pattern) {
                    Ok(regex) => Self {
                        expr: None,
                        regex: Some(regex),
                        error: None,
                    },
                    Err(error) => Self {
                        expr: None,
                        regex: None,
                        error: Some(error.to_string()),
                    },
                }
            }
        }
    }

    /// Пуст ли фильтр: показывать всё.
    pub fn is_empty(&self) -> bool {
        self.expr.is_none() && self.regex.is_none() && self.error.is_none()
    }

    /// Ошибка разбора фильтра, если он не разбирается.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Проходит ли процесс под фильтр.
    pub fn matches(&self, process: &ProcessInfo) -> bool {
        // Некорректное выражение не должно ни что-то показывать, ни делать
        // вид, что всё в порядке: список пуст, а ошибка видна рядом.
        if self.error.is_some() {
            return false;
        }
        if let Some(expr) = &self.expr {
            return expr.matches(process);
        }
        if let Some(regex) = &self.regex {
            return regex.is_match(&process.name) || regex.is_match(&process.key.pid.to_string());
        }
        true
    }
}

/// Разборщик выражения.
struct Parser {
    tokens: Vec<String>,
    pos: usize,
}

impl Parser {
    /// Текущий токен.
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.pos).map(String::as_str)
    }

    /// Забрать текущий токен.
    fn next_token(&mut self) -> Option<String> {
        let token = self.tokens.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    /// Разбор выражения: `a or b or c`.
    fn parse_expr(&mut self) -> Option<Expr> {
        let mut alternatives = vec![self.parse_and()?];
        while self
            .peek()
            .is_some_and(|token| token.eq_ignore_ascii_case("or"))
        {
            self.next_token();
            alternatives.push(self.parse_and()?);
        }
        if alternatives.len() == 1 {
            Some(alternatives.pop().expect("один вариант"))
        } else {
            Some(Expr::Any(alternatives))
        }
    }

    /// Разбор цепочки «и»: условия без оператора тоже соединяются по «и».
    fn parse_and(&mut self) -> Option<Expr> {
        let mut items = vec![self.parse_unary()?];
        loop {
            match self.peek() {
                Some(token) if token.eq_ignore_ascii_case("and") => {
                    self.next_token();
                    items.push(self.parse_unary()?);
                }
                Some(token) if token.eq_ignore_ascii_case("or") => break,
                Some(_) => items.push(self.parse_unary()?),
                None => break,
            }
        }
        Some(Expr::All(items))
    }

    /// Разбор одного условия с возможным отрицанием.
    fn parse_unary(&mut self) -> Option<Expr> {
        if self
            .peek()
            .is_some_and(|token| token.eq_ignore_ascii_case("not"))
        {
            self.next_token();
            let inner = self.parse_unary()?;
            return Some(Expr::Not(Box::new(inner)));
        }
        let token = self.next_token()?;
        Some(parse_term(&token))
    }
}

/// Разбирает одно условие.
fn parse_term(token: &str) -> Expr {
    if let Some((key, value)) = token.split_once(':') {
        if let Some(field) = Field::parse(key) {
            let (comparison, operand) = split_comparison(value);
            let operand = operand.trim();
            if let Ok(number) = operand.parse::<f64>() {
                return Expr::Condition(Condition {
                    field,
                    comparison,
                    value: Value::Number(number),
                });
            }
            return Expr::Condition(Condition {
                field,
                comparison,
                value: Value::Text(operand.to_lowercase()),
            });
        }
        // Поле неизвестно: ищем значение после двоеточия как обычный текст.
        // Так опечатка в имени поля не прячет половину процессов: `pdi:100`
        // найдёт процесс с PID 100, а не покажет пустой список.
        let value = value.trim();
        if !value.is_empty() {
            return Expr::Text(value.to_lowercase());
        }
    }
    Expr::Text(token.to_lowercase())
}

/// Отделяет оператор сравнения от значения.
fn split_comparison(value: &str) -> (Comparison, &str) {
    let value = value.trim_start();
    for (prefix, comparison) in [
        (">=", Comparison::GreaterOrEqual),
        ("<=", Comparison::LessOrEqual),
        (">", Comparison::Greater),
        ("<", Comparison::Less),
        ("=", Comparison::Equal),
    ] {
        if let Some(rest) = value.strip_prefix(prefix) {
            return (comparison, rest);
        }
    }
    (Comparison::Equal, value)
}

/// Разбивает строку на токены, учитывая кавычки.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut has_content = false;

    for character in text.chars() {
        match character {
            '"' => {
                quoted = !quoted;
                has_content = true;
            }
            character if character.is_whitespace() && !quoted => {
                if has_content {
                    tokens.push(std::mem::take(&mut current));
                    has_content = false;
                }
            }
            character => {
                current.push(character);
                has_content = true;
            }
        }
    }
    if has_content {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpad_core::{Counter, ProcessKey, Source};

    fn process(pid: u32, name: &str, cpu: Option<f64>, memory: Option<u64>) -> ProcessInfo {
        let mut process = ProcessInfo::new(ProcessKey::from_pid(pid), name);
        if let Some(value) = cpu {
            process.cpu_percent = Counter::measured(value, Source::Derived);
        }
        if let Some(value) = memory {
            process.memory_working_set = Counter::measured(value, Source::Derived);
        }
        process.thread_count = Counter::measured(7, Source::Derived);
        process.handle_count = Counter::measured(120, Source::Derived);
        process
    }

    fn sample() -> ProcessInfo {
        process(4242, "rustc.exe", Some(12.5), Some(64 * 1_048_576))
    }

    #[test]
    fn empty_filter_matches_everything() {
        let filter = Filter::parse("");
        assert!(filter.is_empty());
        assert!(filter.matches(&sample()));
    }

    #[test]
    fn plain_text_matches_name_or_pid() {
        let filter = Filter::parse("rustc");
        assert!(filter.matches(&sample()));
        let filter = Filter::parse("4242");
        assert!(filter.matches(&sample()));
        let filter = Filter::parse("python");
        assert!(!filter.matches(&sample()));
    }

    #[test]
    fn named_field_matches_name() {
        let filter = Filter::parse("name:rust");
        assert!(filter.matches(&sample()));
        let filter = Filter::parse("имя:rust");
        assert!(filter.matches(&sample()));
        let filter = Filter::parse("name:python");
        assert!(!filter.matches(&sample()));
    }

    #[test]
    fn numeric_comparison_works() {
        assert!(Filter::parse("pid:>4000").matches(&sample()));
        assert!(!Filter::parse("pid:>5000").matches(&sample()));
        assert!(Filter::parse("pid:4242").matches(&sample()));
        assert!(Filter::parse("cpu:>10").matches(&sample()));
        assert!(!Filter::parse("cpu:>20").matches(&sample()));
        // Память в мегабайтах.
        assert!(Filter::parse("mem:>60").matches(&sample()));
        assert!(!Filter::parse("mem:>100").matches(&sample()));
        assert!(Filter::parse("threads:>4").matches(&sample()));
        assert!(Filter::parse("handles:>=120").matches(&sample()));
    }

    #[test]
    fn unknown_value_does_not_pass_a_condition() {
        let unknown = process(7, "unknown.exe", None, None);
        assert!(!Filter::parse("cpu:>0").matches(&unknown));
        assert!(!Filter::parse("cpu:>=0").matches(&unknown));
        assert!(Filter::parse("pid:7").matches(&unknown));
    }

    #[test]
    fn not_negates_the_next_condition() {
        assert!(!Filter::parse("not rustc").matches(&sample()));
        assert!(Filter::parse("not python").matches(&sample()));
        assert!(!Filter::parse("not name:rustc").matches(&sample()));
    }

    #[test]
    fn and_requires_both_conditions() {
        let filter = Filter::parse("name:rustc cpu:>10");
        assert!(filter.matches(&sample()));
        let filter = Filter::parse("name:rustc cpu:>50");
        assert!(!filter.matches(&sample()));
        let filter = Filter::parse("name:rustc and cpu:>10");
        assert!(filter.matches(&sample()));
    }

    #[test]
    fn or_accepts_one_of_the_conditions() {
        assert!(Filter::parse("name:python or cpu:>10").matches(&sample()));
        assert!(!Filter::parse("name:python or cpu:>50").matches(&sample()));
    }

    #[test]
    fn quoted_value_keeps_spaces() {
        let edge = process(9, "Microsoft Edge.exe", Some(1.0), None);
        assert!(Filter::parse("name:\"Microsoft Edge\"").matches(&edge));
        assert!(!Filter::parse("name:\"Microsoft Edge\"").matches(&sample()));
    }

    #[test]
    fn unknown_field_is_treated_as_plain_text() {
        // Неизвестное поле: значение после двоеточия ищется как обычный текст.
        let filter = Filter::parse("неизвестно:rustc");
        assert!(filter.matches(&sample()));
        let filter = Filter::parse("неизвестно:python");
        assert!(!filter.matches(&sample()));
        // Без значения ищем весь токен целиком.
        let filter = Filter::parse("что:");
        assert!(!filter.matches(&sample()));
        // Опечатка в поле не должна прятать процессы: ищем по значению.
        let filter = Filter::parse("pdi:4242");
        assert!(filter.matches(&sample()));
    }

    #[test]
    fn broken_input_never_panics() {
        for text in [
            "", "   ", ":", "not", "and", "or", "name:", "pid:>", "cpu:abc", "\"",
        ] {
            let filter = Filter::parse(text);
            // Разбор не падает и что-то возвращает.
            let _ = filter.matches(&sample());
        }
    }

    #[test]
    fn regex_mode_matches_by_pattern() {
        let filter = Filter::parse_with_mode("^rust.*\\.exe$", Mode::Regex);
        assert!(filter.error().is_none());
        assert!(filter.matches(&sample()));
        assert!(!filter.matches(&process(9, "python.exe", Some(1.0), None)));
    }

    #[test]
    fn regex_mode_ignores_case() {
        let filter = Filter::parse_with_mode("RUSTC", Mode::Regex);
        assert!(filter.matches(&sample()));
    }

    #[test]
    fn broken_regex_reports_error_and_matches_nothing() {
        let filter = Filter::parse_with_mode("rust(", Mode::Regex);
        assert!(filter.error().is_some());
        assert!(!filter.is_empty());
        assert!(!filter.matches(&sample()));
    }

    #[test]
    fn empty_regex_filter_matches_everything() {
        let filter = Filter::parse_with_mode("   ", Mode::Regex);
        assert!(filter.is_empty());
        assert!(filter.matches(&sample()));
    }
}
