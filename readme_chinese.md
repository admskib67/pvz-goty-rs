# pvz-goty-rs

<img src="./github/pvz-icon.png" width="96" align="right" alt="植物大战僵尸图标">

以 vibe coding 方式重制的《植物大战僵尸：年度版》(Plants vs. Zombies: Game of the Year Edition)。

- 需要一份正版游戏
- 目前所有关卡和小游戏都应可以游玩
- 本 readme 还提供以下语言版本：

| [English](readme.md) | [Español](readme_spanish.md) | [Português](readme_portuguese.md) | [Русский](readme_russian.md) | [中文](readme_chinese.md) | [Français](readme_french.md) | [Deutsch](readme_german.md) |
| --- | --- | --- | --- | --- | --- | --- |

![主菜单截图](./github/main%20menu.png)

## 如何运行

- 如果有适用于你操作系统的预编译版本，直接使用即可
- 否则，可以像普通 Rust 程序一样编译游戏，运行：

```bash
cargo build --release -p pvz
```

编译输出位于：

- macOS/Linux：`target/release/pvz`
- Windows：`target\release\pvz.exe`

注意：如果出现与 libopenmpt 相关的编译错误，请参阅本 readme 的 libopenmpt 部分。

## 致谢

- PopCap Games，创作了《植物大战僵尸》。
- ruslan831，维护 pvz 的反编译项目：https://github.com/ruslan831/PlantsVsZombies-decompilation。
- libopenmpt 开发团队，开发了 libopenmpt 库：https://lib.openmpt.org/libopenmpt/。
- Bevy 游戏引擎背后的开发者们：https://github.com/bevyengine/bevy

## 开发命令
为了方便测试，开发了一些专用命令和工具，以下是其中一部分：

```bash
cargo run -p pvz -- --clean          # 清除已保存的应用数据和用户档案
cargo run -p pvz -- --lvl 3-1        # 游玩指定的冒险模式关卡
cargo run -p pvz -- --intro          # 重新播放第一次冒险的开场
```

## 生成带图标的 mac 版本

使用以下脚本为 mac 打包游戏：

```bash
./scripts/package-macos.sh
```

应用将生成在：

```text
target/release/Plants vs Zombies.app
```

## libopenmpt

由于游戏部分音频的处理方式，此库是必需的。
项目已附带适用于 Windows、mac 和 WSL Ubuntu 的预编译版本。

如果附带的 libopenmpt 库无法在你的操作系统或架构上使用，可以通过以下命令自行编译：

```bash
cargo run -p build-libopenmpt
```
