# Cahier des charges : PDFMerge

> Application de fusion de PDF avec sélection et réorganisation de pages.
> Document destiné à servir de point de départ à un LLM (vibe coding). Les choix de conception (syntaxe CLI, plages descendantes, dossiers, nom de sortie, bibliothèque PDF, structure en workspace) ont été **validés par le développeur** et sont à appliquer tels quels.

---

## 1. Présentation

**PDFMerge** permet de créer un nouveau PDF à partir de pages issues de plusieurs PDF sources. Pour chaque source, l'utilisateur décrit les pages voulues avec une expression souple (plages, numéros isolés, ordre libre, doublons, « jusqu'à la fin »). Les pages sélectionnées de chaque document sont ajoutées les unes après les autres : d'abord celles du document 1, puis celles du document 2, etc.

L'application s'utilise de deux façons :

- **GUI** (Slint) pour un usage interactif ;
- **CLI** pour l'automatisation et les scripts.

Les deux modes partagent exactement le même moteur (crate cœur), pour garantir un comportement identique.

## 2. Périmètre fonctionnel

### 2.1 Fonctionnalités attendues

| # | Fonctionnalité | Priorité |
|---|----------------|----------|
| F1 | Ajouter un ou plusieurs fichiers PDF (nombre illimité) | Must |
| F2 | Ajouter un répertoire : tous les PDF qu'il contient sont ajoutés | Must |
| F3 | Définir pour chaque fichier une sélection de pages (syntaxe §3) | Must |
| F4 | Réorganisation et doublons de pages à l'intérieur d'un même document | Must |
| F5 | Conservation de l'orientation de chaque page (portrait/paysage, rotations) même en mélangeant des formats | Must |
| F6 | Choix du dossier et du nom du fichier de sortie, avec valeurs par défaut | Must |
| F7 | Réordonner, retirer des fichiers dans la liste (GUI) | Must |
| F8 | Mode CLI complet avec paramètres | Must |
| F9 | Validation en direct de la sélection de pages (GUI) avec message d'erreur clair et nombre de pages résultant | Should |
| F10 | Barre de progression et exécution non bloquante (GUI) | Should |
| F11 | Gestion de l'écrasement d'un fichier existant (confirmation / `--force`) | Should |
| F12 | Glisser-déposer de fichiers/dossiers dans la fenêtre | Could |

### 2.2 Règles de gestion

**Fichiers sources**
- Un même fichier peut être ajouté plusieurs fois, avec des sélections différentes (chaque entrée est indépendante).
- L'ordre des entrées dans la liste détermine l'ordre dans le PDF final.
- Un répertoire ajouté est développé en une entrée par PDF (extension `.pdf`, insensible à la casse), **non récursif** (les sous-dossiers sont ignorés), tri **alphabétique naturel** (`doc2.pdf` avant `doc10.pdf`).
- Les fichiers non-PDF ou illisibles sont signalés, sans arrêter silencieusement le traitement.

**Sélection de pages**
- Sans sélection, toutes les pages du document sont prises.
- La sélection est évaluée **par document** : `$` désigne la dernière page de *ce* document.

**Sortie**
- **Dossier par défaut** : le dossier du premier fichier source (ou le répertoire ajouté).
- **Nom par défaut** : `fusion_AAAAMMJJ-HHMMSS.pdf`, horodatage en **heure locale** au moment de la fusion (ex. `fusion_20261022-143507.pdf`). L'horodatage est calculé au lancement de la fusion ; en GUI, il est pré-rempli à l'ajout de la première source et recalculé tant que l'utilisateur n'a pas modifié le nom manuellement.
- Dossier et nom sont librement modifiables.
- Si le fichier de sortie existe déjà : confirmation en GUI, erreur en CLI sauf `--force`.
- Le fichier de sortie ne doit **jamais** écraser un fichier source utilisé dans la fusion (refus explicite).
- L'extension `.pdf` est ajoutée si absente.

**Orientation**
- Chaque page conserve son orientation d'origine, y compris l'attribut `/Rotate` et ses dimensions (`MediaBox`/`CropBox`). Un document composé de pages portrait et paysage mélangées reste fidèle aux sources, sans normalisation ni rotation forcée.

## 3. Syntaxe de sélection des pages

### 3.1 Grammaire

```
selection  := item ( "," item )*
item       := page | plage
plage      := page "-" page
page       := entier | "$"
entier     := [1-9][0-9]*        (numérotation à partir de 1)
```

- Les espaces autour des éléments sont ignorés.
- Les crochets englobants sont tolérés : `[3-6,1,4-$]` ≡ `3-6,1,4-$`.
- `$` = dernière page du document.

### 3.2 Sémantique

- Les éléments sont traités **dans l'ordre d'écriture**, les doublons sont conservés.
- Plage ascendante `a-b` : pages a, a+1, …, b.
- **Plage descendante** `b-a` (b > a) : pages en ordre inversé. C'est un comportement **normal et sans erreur** (ex. `$-1` inverse tout le document, `6-3` donne 6, 5, 4, 3).
- Un numéro hors limites (> nombre de pages) est une **erreur** explicite indiquant le document et le nombre de pages réel.
- Sélection vide ou chaîne vide : toutes les pages.

### 3.3 Exemples

Pour un document de 10 pages :

| Expression | Pages obtenues |
|------------|----------------|
| `3-6,1,4-$` | 3, 4, 5, 6, 1, 4, 5, 6, 7, 8, 9, 10 |
| `$` | 10 |
| `$-1` | 10, 9, 8, …, 1 (inversion complète) |
| `2,2,2` | 2, 2, 2 |
| `1-3,8-$` | 1, 2, 3, 8, 9, 10 |
| `12` | Erreur : page 12 inexistante (document de 10 pages) |

## 4. Interface en ligne de commande

Le binaire sans argument lance la GUI ; avec des arguments de fusion, il s'exécute en mode CLI.

### 4.1 Syntaxe proposée

```
pdfmerge [OPTIONS] <ENTRÉE>...
```

Chaque `<ENTRÉE>` est :
- `chemin/fichier.pdf` : toutes les pages ;
- `chemin/fichier.pdf[3-6,1,4-$]` : sélection de pages (même notation que dans l'énoncé) ;
- `chemin/dossier/` : tous les PDF du dossier.

| Option | Description |
|--------|-------------|
| `-o, --output <CHEMIN>` | Fichier de sortie complet (dossier + nom) |
| `-d, --output-dir <DOSSIER>` | Dossier de sortie (défaut : dossier de la 1re source) |
| `-n, --name <NOM>` | Nom du fichier de sortie (défaut : `fusion_AAAAMMJJ-HHMMSS.pdf`) |
| `-f, --force` | Écrase le fichier de sortie existant sans demander |
| `--dry-run` | Affiche le plan (fichier → pages retenues) sans écrire |
| `-q, --quiet` / `-v, --verbose` | Niveau de verbosité |
| `--gui` | Force le lancement de l'interface graphique |
| `-h, --help` / `-V, --version` | Aide / version |

`--output` et (`--output-dir` / `--name`) sont mutuellement exclusifs.

### 4.2 Exemples

```bash
# Fusion simple de deux fichiers entiers
pdfmerge a.pdf b.pdf

# Sélection de pages (guillemets obligatoires dans le shell à cause de [ ] et $)
pdfmerge 'a.pdf[3-6,1,4-$]' 'b.pdf[2-4]' -o resultat.pdf

# Tous les PDF d'un dossier, nom de sortie personnalisé
pdfmerge ./scans/ -n bilan_2026.pdf

# Simulation
pdfmerge 'a.pdf[1-3]' b.pdf --dry-run
```

### 4.3 Codes de sortie

| Code | Signification |
|------|---------------|
| 0 | Succès |
| 1 | Erreur d'usage / arguments invalides |
| 2 | Erreur de sélection de pages (syntaxe ou hors limites) |
| 3 | Erreur de lecture d'un PDF (introuvable, corrompu, chiffré) |
| 4 | Erreur d'écriture (permissions, fichier existant sans `--force`) |

Les messages d'erreur vont sur `stderr`, les résultats utiles sur `stdout`.

## 5. Interface graphique (Slint)

### 5.1 Écran principal (fenêtre unique)

1. **Barre d'ajout** : boutons « Ajouter des fichiers… » et « Ajouter un dossier… » (dialogues natifs).
2. **Liste des sources** (zone centrale), une ligne par fichier :
   - poignée / boutons ↑ ↓ pour réordonner, bouton ✕ pour retirer ;
   - nom du fichier (chemin complet en infobulle) ;
   - nombre de pages du document ;
   - **champ « Pages »** avec placeholder `toutes` et exemple `3-6,1,4-$` ;
   - indicateur de validité : nombre de pages retenues, ou message d'erreur en rouge.
3. **Zone de sortie** : champ dossier (+ bouton « Parcourir… »), champ nom du fichier ; valeurs par défaut pré-remplies dès l'ajout de la première source.
4. **Bouton « Fusionner »** : désactivé tant qu'une entrée est invalide ou que la liste est vide.
5. **Barre de statut / progression** : état, erreur éventuelle, lien « Ouvrir le dossier » après succès.

### 5.2 Comportements

- Le traitement s'exécute dans un **thread séparé** ; l'interface reste réactive et la progression remonte via `slint::invoke_from_event_loop`.
- Validation de la sélection à la frappe (avec petit délai) en réutilisant le parseur du crate cœur.
- Interface en **français** (prévoir `@tr()` de Slint pour une éventuelle traduction).
- Thème clair/sombre suivant le système si possible.

## 6. Choix techniques

| Domaine | Choix | Remarque |
|---------|-------|----------|
| Langage | **Rust, édition 2024** | Toolchain ≥ 1.85, `rust-version` fixé dans `Cargo.toml` |
| GUI | **Slint** (dernière version stable) | Fichiers `.slint` + `slint-build` dans `build.rs` |
| CLI | **clap** (derive) | Aide et validation automatiques |
| Manipulation PDF | **lopdf** | Rust pur, permet de cloner/réordonner des pages et d'écrire un nouveau document |
| Dialogues de fichiers | **rfd** | Dialogue natif, utilisable avec Slint |
| Erreurs | **thiserror** (cœur) + **anyhow** (binaires) | Erreurs typées côté bibliothèque |
| Tri naturel | **natord** (ou équivalent) | Ordre des fichiers d'un dossier |
| Parcours de dossiers | `std::fs` | Non récursif : `read_dir` suffit |
| Date et heure | **chrono** (feature `clock`) | Horodatage local du nom par défaut |
| Journalisation | `tracing` ou `log` + `env_logger` | Optionnel |
| Tests | `cargo test`, `assert_cmd`, `tempfile`, `proptest` (parseur) | PDF de test générés par code |

> **Décision** : `lopdf` uniquement, **aucune prévisualisation** des pages dans cette version. Une évolution ultérieure pourrait utiliser `pdfium-render` pour des miniatures, mais cela imposerait de distribuer la bibliothèque native PDFium ; l'architecture (moteur séparé de l'interface) ne ferme pas cette porte.

### 6.1 Architecture proposée

Workspace Cargo avec séparation nette moteur / interfaces :

```
pdfmerge/
├── Cargo.toml                  # workspace, edition = "2024"
├── crates/
│   ├── pdfmerge-core/          # logique métier, sans dépendance UI
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── page_spec.rs    # parseur + évaluation de la sélection
│   │       ├── sources.rs      # résolution fichiers/dossiers, tri
│   │       ├── merge.rs        # fusion lopdf
│   │       ├── output.rs       # chemin/nom de sortie par défaut, anti-écrasement
│   │       └── error.rs
│   └── pdfmerge-app/           # binaire unique
│       ├── build.rs            # compile les .slint
│       ├── ui/main.slint
│       └── src/
│           ├── main.rs         # aiguillage CLI / GUI
│           ├── cli.rs
│           └── gui.rs
└── tests/                      # tests d'intégration, jeux de PDF
```

**API publique du cœur (indicative)**

```rust
pub struct PageSpec(/* éléments parsés */);
impl PageSpec {
    pub fn parse(input: &str) -> Result<Self, SpecError>;
    pub fn resolve(&self, page_count: u32) -> Result<Vec<u32>, SpecError>; // pages 1-indexées, ordre final
}

pub struct SourceEntry { pub path: PathBuf, pub spec: PageSpec }

pub struct MergeRequest { pub sources: Vec<SourceEntry>, pub output: PathBuf, pub overwrite: bool }

pub fn merge(req: &MergeRequest, progress: impl FnMut(Progress)) -> Result<MergeReport, MergeError>;
```

Le parseur et la résolution sont des **fonctions pures**, faciles à tester unitairement.

### 6.2 Algorithme de fusion (principe)

1. Pour chaque source : charger le document, résoudre la sélection en liste ordonnée de pages.
2. Renuméroter les objets du document source pour éviter les collisions d'identifiants avec le document cible, puis les importer.
3. Pour chaque page retenue (dans l'ordre, doublons inclus) : **matérialiser les attributs hérités** (`MediaBox`, `CropBox`, `Resources`, `Rotate`) directement sur le dictionnaire de la page, puis l'ajouter à l'arbre `/Pages` du document cible.
4. Reconstruire le `/Pages` racine (`Kids`, `Count`) et le catalogue, supprimer les objets inutilisés, écrire le fichier.

## 7. Difficultés et points d'attention

### 7.1 Techniques (PDF)

- **Attributs hérités de l'arbre de pages** : `MediaBox`, `CropBox`, `Resources` et surtout **`Rotate`** peuvent être définis sur un nœud parent et non sur la page. En copiant la page vers un autre arbre sans résoudre l'héritage, **l'orientation et les dimensions sont perdues**. C'est le point critique de l'exigence « orientation conservée » : il faut explicitement résoudre et copier ces attributs sur chaque page. À couvrir par un test dédié (mélange portrait/paysage, `Rotate` 90/180/270).
- **Pages dupliquées** : une même page présente plusieurs fois (`2,2,2`) doit produire des pages valides et indépendantes. Décider : cloner le dictionnaire de page mais partager le contenu et les ressources (fichier plus léger), ce qui est valide en PDF. Éviter de dupliquer les flux de contenu lourds.
- **Collisions d'identifiants d'objets** entre documents : renumérotation obligatoire avant import.
- **Fichiers volumineux ou nombreux** : maîtriser la mémoire (chargement séquentiel, libération du document après import). Tester sur plusieurs centaines de pages.
- **PDF chiffrés / protégés par mot de passe** : détecter et signaler clairement (v1 : refus avec message explicite ; v2 : demande du mot de passe).
- **PDF corrompus ou atypiques** (xref cassée, flux d'objets, PDF hybrides) : certains peuvent échouer avec lopdf. Prévoir des erreurs lisibles, jamais de `panic`, et un jeu de tests incluant des PDF « difficiles ».
- **Perte de métadonnées de structure** : signets/outlines, liens internes, champs de formulaire (AcroForm), pièces jointes et tags d'accessibilité ne sont pas conservés ou peuvent devenir invalides après sélection partielle. **Limite connue de la v1** à documenter, avec nettoyage des annotations de lien pointant vers des pages absentes.
- **Polices et ressources partagées** : s'assurer que les `Resources` référencées sont bien importées même si elles proviennent d'un nœud parent.

### 7.2 Parseur de sélection

- Cas limites : `3-`, `-3`, `1,,2`, `0`, `a-b`, `$-$`, espaces, `$` seul, plage descendante, très grands entiers (débordement), crochets déséquilibrés.
- Les messages d'erreur doivent indiquer **la position** dans la chaîne et la cause.
- Le tester par tests unitaires **et** par propriété (`proptest`) : toute page produite est dans `[1, n]`.

### 7.3 CLI

- Les caractères `[`, `]` et `$` sont interprétés par le shell : les arguments doivent être **entre guillemets simples** (bash/zsh) ; documenter l'usage sous PowerShell et `cmd`.
- Sous Windows, le `:` des chemins (`C:\…`) interdit d'utiliser `:` comme séparateur fichier/pages : d'où le choix de la notation `fichier.pdf[pages]`. Le parseur d'entrée doit prendre le **dernier** `[` suivi d'un `]` final.

### 7.4 GUI / Slint

- **Un seul binaire CLI + GUI** : sous Windows, `windows_subsystem = "windows"` masque la console et casse la sortie CLI. Solution : attacher la console parente (`AttachConsole`) au démarrage en mode CLI, ou fournir deux binaires.
- **Pas de dialogue de fichiers intégré à Slint** : utiliser `rfd` (attention au thread appelant, préférer l'API asynchrone ou un thread dédié).
- **Glisser-déposer** : support limité/dépendant du backend Slint ; traiter comme optionnel (F12).
- **Modèles de données** : utiliser `VecModel`/`ModelRc` pour la liste des sources ; les mises à jour depuis un thread doivent passer par `invoke_from_event_loop`.
- **Réordonnancement** : boutons ↑↓ plus simples et fiables qu'un drag & drop de lignes en v1.
- **Backends et rendu** : tester sur Windows, macOS, Linux (X11 et Wayland) ; choisir le renderer adapté (femtovg / skia / software).

### 7.5 Robustesse et sécurité des données

- Écrire d'abord dans un **fichier temporaire** du dossier cible, puis renommer atomiquement : aucune sortie corrompue en cas d'échec.
- Comparer les chemins **canoniques** pour détecter qu'une sortie écraserait une source.
- Chemins avec espaces, accents, Unicode et très longs (Windows) ; extensions `.PDF`/`.Pdf`.
- Aucun `unwrap()`/`expect()` sur des données externes.

## 8. Critères d'acceptation

1. `pdfmerge 'a.pdf[3-6,1,4-$]'` sur un PDF de 10 pages produit un fichier de 12 pages dans l'ordre 3,4,5,6,1,4,5,6,7,8,9,10.
2. Fusionner un PDF portrait et un PDF paysage conserve l'orientation et la taille de chaque page ; vérifié aussi avec des pages dont `Rotate` est hérité du nœud parent.
3. Passer un dossier utilise tous ses PDF en ordre naturel ; une entrée peut être retirée ou réordonnée dans la GUI.
4. Sans option de sortie, le fichier est créé dans le dossier source avec le nom horodaté par défaut (`fusion_AAAAMMJJ-HHMMSS.pdf`) ; la GUI affiche ces valeurs et les rend modifiables.
5. Une sélection invalide ou hors limites est signalée clairement (GUI : message sur la ligne ; CLI : message + code 2) sans créer de fichier.
6. Le fichier de sortie n'écrase jamais une source ; l'écrasement d'un autre fichier demande confirmation ou `--force`.
7. La GUI reste réactive pendant une fusion de 500+ pages.
8. Le PDF généré s'ouvre sans erreur dans au moins trois lecteurs (navigateur, Acrobat/Preview, lecteur Linux) et passe une vérification (`qpdf --check` ou équivalent).
9. `cargo clippy -- -D warnings`, `cargo fmt --check` et `cargo test` passent.

## 9. Hors périmètre (v1)

- Édition de contenu, OCR, compression, annotation.
- Conservation des signets, formulaires et tags d'accessibilité.
- Mots de passe et signature numérique.
- Aperçu / miniatures des pages (pas de prévisualisation pour l'instant ; piste future : `pdfium-render`).
- Parcours récursif des sous-dossiers.

## 10. Consignes pour le LLM de développement

1. Commencer par le crate **`pdfmerge-core`** : `page_spec` (avec tests unitaires exhaustifs) avant toute autre brique.
2. Implémenter ensuite `merge` avec lopdf et un **jeu de tests générant des PDF en code** (portrait, paysage, `Rotate` hérité, plusieurs pages) avant de toucher aux interfaces.
3. Ajouter la **CLI** (clap) et ses tests d'intégration (`assert_cmd`).
4. Construire enfin la **GUI Slint** par-dessus le même cœur, sans dupliquer de logique métier.
5. Procéder par petites étapes compilables, avec des commits clairs ; ne jamais introduire de dépendance non listée sans justification.
6. Respecter la structure **workspace** : `Cargo.toml` racine de type workspace virtuel (`resolver = "3"`, `[workspace.package]` avec `edition = "2024"`, `[workspace.dependencies]` pour centraliser les versions), chaque crate héritant de ces valeurs (`edition.workspace = true`, `dep.workspace = true`).
7. En cas d'ambiguïté non couverte par ce document, choisir l'option la plus simple et la consigner dans le README.
