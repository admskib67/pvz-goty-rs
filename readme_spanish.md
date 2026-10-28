# pvz-goty-rs

Reconstrucción hecha con vibe coding de Plants vs. Zombies: Game of the Year Edition.

- Se requiere una copia válida del juego
- Ahora mismo todos los niveles y minijuegos deberían ser jugables
- Este readme también está disponible en:

| [English](readme.md) | [Español](readme_spanish.md) | [Português](readme_portuguese.md) | [Русский](readme_russian.md) | [中文](readme_chinese.md) | [Français](readme_french.md) | [Deutsch](readme_german.md) |
| --- | --- | --- | --- | --- | --- | --- |

![imagen del menú principal](./github/main%20menu.png)

## Cómo ejecutarlo

- Si existe una versión precompilada para tu sistema operativo, simplemente usa esa
- Si no, puedes compilar el juego como cualquier programa de Rust ejecutando:

```bash
cargo build --release -p pvz
```

El resultado estará en:

- macOS/Linux: `target/release/pvz`
- Windows: `target\release\pvz.exe`

Nota: si aparece un error de compilación relacionado con libopenmpt, consulta la sección libopenmpt de este readme.

## Créditos

- PopCap Games por crear el juego Plants vs Zombies.
- ruslan831 por mantener una decompilación de pvz en: https://github.com/ruslan831/PlantsVsZombies-decompilation.
- El equipo de desarrollo de libopenmpt por crear la biblioteca libopenmpt: https://lib.openmpt.org/libopenmpt/.
- Las personas detrás del motor de juegos Bevy: https://github.com/bevyengine/bevy

## Comandos de desarrollo
Para las pruebas se han desarrollado algunos comandos y herramientas específicos; aquí tienes algunos de ellos:

```bash
cargo run -p pvz -- --clean          # borra los datos guardados de la app y los perfiles
cargo run -p pvz -- --lvl 3-1        # juega un nivel de aventura elegido
cargo run -p pvz -- --intro          # vuelve a reproducir la intro de la primera aventura
```

## Generar una versión para mac con icono

Usa el siguiente script para empaquetar el juego para mac:

```bash
./scripts/package-macos.sh
```

La app se generará en:

```text
target/release/Plants vs Zombies.app
```

## libopenmpt

Debido a cómo se gestionan algunas partes del audio del juego, usar esta biblioteca es un requisito obligatorio.
El proyecto incluye versiones precompiladas para Windows, mac y Ubuntu en WSL.

Si las bibliotecas de libopenmpt incluidas no funcionan para tu sistema operativo o arquitectura, puedes compilar una con:

```bash
cargo run -p build-libopenmpt
```
