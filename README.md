# Vesper

Язык для Telegram-ботов. Исходники — `.vx`, запуск `vesper run bot.vx`.

Реализация на Rust: лексер → парсер → AST → интерпретатор → генерируемый слой Bot API.

- архитектура: [docs/design.md](docs/design.md)
- синтаксис: [docs/syntax.md](docs/syntax.md)
- примеры: [examples/](examples)

Статус: этап 1 (лексер). Готово: токены с позициями, блоки INDENT/DEDENT, строки с
экранированием и подстановкой `{…}`, ошибки с файлом/строкой/колонкой.

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
