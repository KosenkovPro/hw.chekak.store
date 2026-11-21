# MCPad

Кроссплатформенный диспетчер задач и монитор железа на Rust: **Windows**, **Ubuntu/Linux**, **macOS**.

Проект вырос из сравнения четырёх задач-менеджеров — [System Informer](../systeminformer),
[Task Explorer](../TaskExplorer), [RustWTM](../RustWTM) и [gputaskman](../gputaskman) —
разбор которого лежит в [DEVELOPMENT_PLAN.md](../DEVELOPMENT_PLAN.md).

## Принципы

1. **Неизвестное значение — это `None`, а не ноль.** Каждое число несёт provenance:
   источник (`Source`), уровень уверенности (`Confidence`) и причину недоступности
   (`UnavailableReason`). Ноль — это измеренный ноль; «—» — «данных нет».
2. **Честность важнее полноты.** Там, где ОС не даёт данных, приложение пишет причину
   («нужен root», «нет API у платформы»), а не рисует пустоту.
3. **Действия обратимы и видны.** Любая автоматика по умолчанию в dry-run, журналируется
   и откатывается.
4. **Одна модель для всех платформ.** GUI, CLI, MCP и удалённый клиент читают один
   и тот же снимок.

## Структура workspace

| Крейт | Назначение |
|---|---|
| `mcpad-core` | Модель данных, provenance, JSON-схема снимка. Зависит только от `serde` |
| `mcpad-probe` | Контракт проб: возможности платформы и их деградация |
| `mcpad-backends/windows` | `NtQuerySystemInformation`, путь к образу, командная строка, владелец |
| `mcpad-backends/linux` | `/proc`: процессы, CPU, память, дескрипторы, ввод-вывод |
| `mcpad-backends/macos` | `libproc`: процессы, CPU, память, потоки |
| `mcpad-sampler` | Единственный владелец системных запросов; считает доли CPU из дельт |
| `mcpad-cli` | `mcpad list \| once \| info \| capabilities` |
| `mcpad-theme` | JSON-темы: палитра и метрики, применяются в рантайме |
| `mcpad-canvas` | Бесконечный canvas: граф дерева процессов и граф зависимостей модулей |
| `mcpad-gui` | Окно на egui/eframe |

## Сборка и запуск

```bash
cargo build --workspace
cargo test --workspace

cargo run -p mcpad-cli -- list          # список процессов
cargo run -p mcpad-cli -- list --json   # то же в JSON
cargo run -p mcpad-cli -- info 1234     # подробности процесса
cargo run -p mcpad-cli -- capabilities  # возможности платформы
cargo run -p mcpad-gui                  # окно приложения
```

Требуется Rust 1.85+ (edition 2024). Проверено на Windows; сборка под Linux и macOS
идёт в CI (`.github/workflows/ci.yml`).

## Формат снимка

Схема: `crates/mcpad-core/schema/snapshot.schema.json`. Пример:

```json
{
  "schema_version": 1,
  "taken_at_unix_ms": 1795030000000,
  "system": { "os_name": "Windows", "logical_cpus": 16, "cpu_percent": { "value": 12.4, "source": "derived", "confidence": "measured" } },
  "processes": [
    {
      "key": { "pid": 4242, "start_time_unix": 1795029000, "boot_id": 123456 },
      "name": "explorer.exe",
      "cpu_percent": { "value": 0.4, "source": "derived", "confidence": "measured" },
      "memory_working_set": { "value": 104857600, "source": "windows_system_information", "confidence": "measured" }
    }
  ]
}
```

## Текущее состояние

Волна 1 (M0–M1): workspace, ядро модели со схемой, контракт проб, три бэкенда,
сэмплер с расчётом долей CPU, CLI, JSON-темы, прототип canvas и каркас окна.
Дальше — по [дорожной карте плана](../DEVELOPMENT_PLAN.md#6-дорожная-карта).

## Лицензия

MIT или Apache-2.0.
