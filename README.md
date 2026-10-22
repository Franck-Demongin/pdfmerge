# PDFMerge

PDFMerge est un outil Rust de fusion de PDF permettant de sélectionner et de réorganiser les pages de chaque fichier source. Il assemble les pages choisies en respectant l'ordre défini par l'utilisateur.

L'application propose deux modes d'usage partageant le même moteur :

- une interface graphique Slint ;
- une interface en ligne de commande pour l'automatisation et les scripts.

## Fonctionnalités

- fusion de plusieurs PDF dans l'ordre des entrées sources ;
- sélection de pages par source : pages isolées, plages ascendantes ou descendantes, `$` pour la dernière page, et doublons ;
- réorganisation manuelle, suppression et duplication de sources dans l'interface graphique ;
- validation en direct de la syntaxe de sélection, avec message clair et nombre de pages résultant ;
- dossier de sortie par défaut dans le dossier de la première source et nom par défaut `fusion_AAAAMMJJ-HHMMSS.pdf` ;
- contrôle du fichier de sortie avec refus d'écrasement d'une source et confirmation d'écrasement pour un fichier existant ;
- barre de progression et exécution non bloquante ;
- tri naturel des PDF lorsqu'un dossier est passé en source ;
- conservation des dimensions, de l'orientation des pages et des attributs hérités par les pages.

## Syntaxe de sélection

La même grammaire est utilisée en CLI et dans l'interface graphique.

| Sélection | Signification |
|-----------|---------------|
| vide | toutes les pages |
| `3` | la page 3 |
| `1,3,5` | pages 1, 3 et 5 dans cet ordre |
| `3-6` | pages 3 à 6 |
| `6-3` | pages 6 à 3 dans cet ordre |
| `$` | la dernière page |
| `1-$` | toutes les pages |
| `3-$` | pages 3 jusqu'à la dernière |

Exemples :

```sh
pdfmerge "a.pdf[3-6,1]" "b.pdf[1-3,51-$]"
```

Les numéros hors limites sont refusés avec un message clair.

## Usage en ligne de commande

```sh
pdfmerge                    # lance la GUI sans argument
pdfmerge document.pdf
pdfmerge "a.pdf[3-6,1]" "b.pdf[1-3,51-$]"
pdfmerge -o sortie.pdf a.pdf b.pdf
pdfmerge -d dossier -n fusion a.pdf b.pdf
pdfmerge --dry-run a.pdf b.pdf
pdfmerge -f -q sortie-existante.pdf a.pdf b.pdf
```

Options disponibles :

- `-o, --output` : fichier de sortie complet ;
- `-d, --output-dir` : dossier de sortie ;
- `-n, --name` : nom du fichier de sortie ;
- `-f, --force` : écrase la sortie existante ;
- `--dry-run` : affiche le plan sans écrire le fichier ;
- `-q, --quiet` : sortie silencieuse ;
- `-v, --verbose` : sortie détaillée ;
- `--gui` : force le lancement de l'interface graphique.

Codes de sortie :

| Code | Signification |
|------|---------------|
| `0` | succès |
| `1` | usage incorrect |
| `2` | sélection invalide |
| `3` | fichier PDF illisible, chiffré ou source introuvable |
| `4` | sortie refusée ou impossible à écrire |

## Usage de l'interface graphique

L'interface graphique est lancée automatiquement sans argument. Elle permet de :

- ajouter des fichiers et dossiers PDF ;
- réordonner, supprimer et dupliquer des sources ;
- saisir une sélection de pages par source avec validation ;
- choisir le dossier et le nom de fichier de sortie ;
- suivre la progression et rester réactive pendant la fusion ;
- ouvrir le fichier ou le dossier obtenu après un succès ;
- confirmer ou annuler l'écrasement d'un fichier existant.

## Comportement de la sortie

Sans option explicite, le fichier de sortie est créé dans le dossier de la première source. Le nom par défaut suit le format `fusion_AAAAMMJJ-HHMMSS.pdf`, avec un horodatage en heure locale au moment de la fusion. Dans la GUI, la valeur est pré-remplie et reste modifiable tant que l'utilisateur n'en modifie pas le champ.

Si le fichier de sortie existe déjà :

- en CLI, l'opération échoue avec le code `4`, sauf avec `--force` ;
- en GUI, une boîte de dialogue demande confirmation.

La sortie ne peut pas écraser une source, même avec `--force`.

Les sources sont recherchées sans récursion dans les dossiers. Les extensions `.pdf`, `.PDF` et assimilées sont reconnues. Les noms sont triés par ordre alphabétique naturel.

## Compilation depuis les sources

Prérequis :

- Rust stable ≥ 1.85 avec l'édition 2024 ;
- les dépendances de développement Slint sur Linux.

Sur Ubuntu :

```sh
sudo apt-get update
sudo apt-get install -y \
  libfontconfig1-dev \
  libfreetype6-dev \
  libx11-dev \
  libxkbcommon-dev \
  libwayland-dev
```

Compilation :

```sh
cargo build --release -p pdfmerge-app --bin pdfmerge
```

Le binaire se trouve ensuite à `target/release/pdfmerge` sur Linux, soit `target/release/pdfmerge.exe` sur Windows.

Exécution en développement :

```sh
cargo run -p pdfmerge-app
cargo run -p pdfmerge-app -- --help
```

## Qualité et tests

Le dépôt contient des tests unitaires et d'intégration pour le cœur et le comportement CLI.

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Tests ciblés :

```sh
cargo test -p pdfmerge-core
cargo test -p pdfmerge-app --test cli
```

## Publication GitHub

Le dépôt publie des binaires Linux et Windows via GitHub Actions. Lorsqu'un tag `v*` est poussé :

1. une version Windows est compilée et attachée à la release ;
2. une version Linux est compilée et attachée à la même release ;
3. des notes de version sont générées à partir de l'historique Git.

Tag attendu pour une nouvelle version, par exemple :

```sh
git tag v0.1.0
git push origin main v0.1.0
```

## Documentation et contexte

- `doc/cahier-des-charges-pdfmerge.md` : spécification fonctionnelle complète ;
- `doc/prompt-refonte-gui-pdfmerge.md` : guide de conception et refonte de l'interface ;
- `QWEN.md` : contexte de développement, structure et conventions du projet.

## Licence

Ce projet est distribué sous licence MIT.
