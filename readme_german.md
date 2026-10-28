# pvz-goty-rs

<img src="./github/pvz-icon.png" width="96" align="right" alt="Plants vs. Zombies Symbol">

Per Vibe Coding entstandene Rekonstruktion von Plants vs. Zombies: Game of the Year Edition.

- Eine gültige Kopie des Spiels wird benötigt
- Derzeit sollten alle Level und Minispiele spielbar sein
- Diese Readme ist auch verfügbar auf:

| [English](readme.md) | [Español](readme_spanish.md) | [Português](readme_portuguese.md) | [Русский](readme_russian.md) | [中文](readme_chinese.md) | [Français](readme_french.md) | [Deutsch](readme_german.md) |
| --- | --- | --- | --- | --- | --- | --- |

![Bild des Hauptmenüs](./github/main%20menu.png)

## Ausführen

- Wenn es eine vorkompilierte Version für dein Betriebssystem gibt, nutze einfach diese
- Andernfalls kannst du das Spiel wie jedes Rust-Programm bauen, indem du Folgendes ausführst:

```bash
cargo build --release -p pvz
```

Die Ausgabe liegt unter:

- macOS/Linux: `target/release/pvz`
- Windows: `target\release\pvz.exe`

Hinweis: Wenn ein Build-Fehler im Zusammenhang mit libopenmpt auftritt, sieh dir den Abschnitt libopenmpt in dieser Readme an.

## Danksagungen

- PopCap Games für die Entwicklung des Spiels Plants vs Zombies.
- ruslan831 für die Pflege einer Dekompilierung von pvz: https://github.com/ruslan831/PlantsVsZombies-decompilation.
- Dem libopenmpt-Entwicklerteam für die Bibliothek libopenmpt: https://lib.openmpt.org/libopenmpt/.
- Den Menschen hinter der Spiel-Engine Bevy: https://github.com/bevyengine/bevy

## Entwicklerbefehle
Zum Testen wurden einige spezielle Befehle und Werkzeuge entwickelt, hier sind einige davon:

```bash
cargo run -p pvz -- --clean          # gespeicherte App-Daten und Profile löschen
cargo run -p pvz -- --lvl 3-1        # ein bestimmtes Abenteuer-Level spielen
cargo run -p pvz -- --intro          # das Intro des ersten Abenteuers erneut abspielen
```

## Eine Mac-Version mit Icon erstellen

Verwende das folgende Skript, um das Spiel für den Mac zu paketieren:

```bash
./scripts/package-macos.sh
```

Die App wird hier abgelegt:

```text
target/release/Plants vs Zombies.app
```

## libopenmpt

Aufgrund der Art, wie einige Teile des Spiel-Audios verarbeitet werden, ist diese Bibliothek zwingend erforderlich.
Vorkompilierte Versionen für Windows, Mac und Ubuntu unter WSL liegen dem Projekt bei.

Falls die mitgelieferten libopenmpt-Bibliotheken für dein Betriebssystem oder deine Architektur nicht funktionieren, kannst du sie selbst bauen mit:

```bash
cargo run -p build-libopenmpt
```
