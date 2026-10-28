# pvz-goty-rs

<img src="./github/pvz-icon.png" width="96" align="right" alt="Ícone de Plants vs. Zombies">

Reconstrução feita com vibe coding de Plants vs. Zombies: Game of the Year Edition.

- É necessária uma cópia válida do jogo
- No momento, todas as fases e minijogos devem ser jogáveis
- Este readme também está disponível em:

| [English](readme.md) | [Español](readme_spanish.md) | [Português](readme_portuguese.md) | [Русский](readme_russian.md) | [中文](readme_chinese.md) | [Français](readme_french.md) | [Deutsch](readme_german.md) |
| --- | --- | --- | --- | --- | --- | --- |

![imagem do menu principal](./github/main%20menu.png)

## Como executar

- Se existir uma versão pré-compilada para o seu sistema operacional, basta usá-la
- Caso contrário, você pode compilar o jogo como qualquer programa Rust executando:

```bash
cargo build --release -p pvz
```

O resultado ficará em:

- macOS/Linux: `target/release/pvz`
- Windows: `target\release\pvz.exe`

Observação: se ocorrer um erro de compilação relacionado ao libopenmpt, consulte a seção libopenmpt deste readme.

## Créditos

- PopCap Games por criar o jogo Plants vs Zombies.
- ruslan831 por manter uma decompilação de pvz em: https://github.com/ruslan831/PlantsVsZombies-decompilation.
- A equipe de desenvolvimento do libopenmpt por criar a biblioteca libopenmpt: https://lib.openmpt.org/libopenmpt/.
- As pessoas por trás da engine de jogos Bevy: https://github.com/bevyengine/bevy

## Comandos de desenvolvimento
Para testes, foram desenvolvidos alguns comandos e ferramentas específicos; aqui estão alguns deles:

```bash
cargo run -p pvz -- --clean          # apaga os dados salvos do app e os perfis
cargo run -p pvz -- --lvl 3-1        # joga uma fase escolhida do modo aventura
cargo run -p pvz -- --intro          # reexibe a introdução da primeira aventura
```

## Gerando uma versão para mac com ícone

Use o seguinte script para empacotar o jogo para mac:

```bash
./scripts/package-macos.sh
```

O app será gerado em:

```text
target/release/Plants vs Zombies.app
```

## libopenmpt

Devido à forma como algumas partes do áudio do jogo são tratadas, usar esta biblioteca é um requisito obrigatório.
O projeto inclui versões pré-compiladas para Windows, mac e Ubuntu no WSL.

Caso as bibliotecas libopenmpt incluídas não funcionem no seu sistema operacional ou arquitetura, você pode compilar uma com:

```bash
cargo run -p build-libopenmpt
```
