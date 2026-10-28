# pvz-goty-rs

Reconstruction en vibe coding de Plants vs. Zombies: Game of the Year Edition.

- Une copie valide du jeu est requise
- Pour l'instant, tous les niveaux et mini-jeux devraient être jouables
- Ce readme est aussi disponible en :

| [English](readme.md) | [Español](readme_spanish.md) | [Português](readme_portuguese.md) | [Русский](readme_russian.md) | [中文](readme_chinese.md) | [Français](readme_french.md) | [Deutsch](readme_german.md) |
| --- | --- | --- | --- | --- | --- | --- |

![image du menu principal](./github/main%20menu.png)

## Comment le lancer

- Si une version précompilée existe pour votre système d'exploitation, utilisez-la simplement
- Sinon, vous pouvez compiler le jeu comme n'importe quel programme Rust en lançant :

```bash
cargo build --release -p pvz
```

Le résultat se trouvera dans :

- macOS/Linux : `target/release/pvz`
- Windows : `target\release\pvz.exe`

Remarque : en cas d'erreur de compilation liée à libopenmpt, consultez la section libopenmpt de ce readme.

## Crédits

- PopCap Games pour avoir créé le jeu Plants vs Zombies.
- ruslan831 pour la maintenance d'une décompilation de pvz : https://github.com/ruslan831/PlantsVsZombies-decompilation.
- L'équipe de développement de libopenmpt pour la bibliothèque libopenmpt : https://lib.openmpt.org/libopenmpt/.
- Les personnes derrière le moteur de jeu Bevy : https://github.com/bevyengine/bevy

## Commandes de développement
Pour les tests, quelques commandes et outils spécifiques ont été développés, en voici quelques-uns :

```bash
cargo run -p pvz -- --clean          # efface les données sauvegardées de l'app et les profils
cargo run -p pvz -- --lvl 3-1        # joue un niveau d'aventure choisi
cargo run -p pvz -- --intro          # rejoue l'introduction de la première aventure
```

## Créer une version mac avec une icône

Utilisez le script suivant pour empaqueter le jeu pour mac :

```bash
./scripts/package-macos.sh
```

L'application sera générée dans :

```text
target/release/Plants vs Zombies.app
```

## libopenmpt

En raison de la façon dont certaines parties du son du jeu sont gérées, cette bibliothèque est indispensable.
Des versions précompilées pour Windows, mac et Ubuntu sous WSL sont fournies avec le projet.

Si les bibliothèques libopenmpt fournies ne fonctionnent pas pour votre système d'exploitation ou votre architecture, vous pouvez en compiler une avec :

```bash
cargo run -p build-libopenmpt
```
