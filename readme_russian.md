# pvz-goty-rs

Реконструкция Plants vs. Zombies: Game of the Year Edition, написанная в стиле vibe coding.

- Требуется легальная копия игры
- На данный момент все уровни и мини-игры должны быть проходимы
- Этот readme также доступен на:

| [English](readme.md) | [Español](readme_spanish.md) | [Português](readme_portuguese.md) | [Русский](readme_russian.md) | [中文](readme_chinese.md) | [Français](readme_french.md) | [Deutsch](readme_german.md) |
| --- | --- | --- | --- | --- | --- | --- |

![главное меню](./github/main%20menu.png)

## Как запустить

- Если для вашей ОС есть готовая скомпилированная версия, просто используйте её
- Если нет, игру можно собрать как обычную программу на Rust командой:

```bash
cargo build --release -p pvz
```

Результат сборки будет здесь:

- macOS/Linux: `target/release/pvz`
- Windows: `target\release\pvz.exe`

Примечание: если при сборке возникает ошибка, связанная с libopenmpt, обратитесь к разделу libopenmpt этого readme.

## Благодарности

- PopCap Games за создание игры Plants vs Zombies.
- ruslan831 за поддержку декомпиляции pvz: https://github.com/ruslan831/PlantsVsZombies-decompilation.
- Команде разработчиков libopenmpt за библиотеку libopenmpt: https://lib.openmpt.org/libopenmpt/.
- Создателям игрового движка Bevy: https://github.com/bevyengine/bevy

## Команды для разработки
Для тестирования были созданы специальные команды и инструменты, вот некоторые из них:

```bash
cargo run -p pvz -- --clean          # удалить сохранённые данные приложения и профили
cargo run -p pvz -- --lvl 3-1        # сыграть выбранный уровень приключения
cargo run -p pvz -- --intro          # заново показать вступление первого приключения
```

## Сборка версии для mac с иконкой

Используйте следующий скрипт, чтобы упаковать игру для mac:

```bash
./scripts/package-macos.sh
```

Приложение будет сохранено в:

```text
target/release/Plants vs Zombies.app
```

## libopenmpt

Из-за того, как обрабатываются некоторые части звука в игре, эта библиотека обязательна.
В проект включены готовые сборки для Windows, mac и Ubuntu в WSL.

Если включённые библиотеки libopenmpt не работают на вашей ОС или архитектуре, вы можете собрать их сами командой:

```bash
cargo run -p build-libopenmpt
```
