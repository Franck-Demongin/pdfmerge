# QWEN.md — Contexte pour le dépôt `pdfmerge`

Fichier d'instructions destiné aux futures sessions Qwen sur ce dépôt. Résumé du projet, de sa structure actuelle et des conventions de travail.

## Vue d'ensemble du projet

**PDFMerge** est un outil Rust de fusion de PDF avec sélection et réorganisation de pages par source. Pour chaque fichier source, l'utilisateur décrit les pages souhaitées avec une syntaxe souple (plages ascendantes/descendantes, numéros isolés, doublons, `$` pour la dernière page). Les pages sélectionnées sont concaténées dans l'ordre des entrées sources.

Deux modes d'usage, partageant **exactement le même moteur** (`pdfmerge-core`) :
- **GUI** Slint (usage interactif)
- **CLI** (automatisation/scripts)

Le cahier des charges détaillé est dans `doc/cahier-des-charges-pdfmerge.md` (à consulter pour tout détail fonctionnel).

## État actuel du dépôt

Implémentation fonctionnelle livrée en phase cœur puis app :

- `pdfmerge-core` expose `PageSpec`, `sources`, `output`, `merge` et les types de requête/rapport.
- `pdfmerge-app` fournit un binaire unique avec aiguillage GUI/CLI.
- `crates/pdfmerge-core/tests/merge.rs` couvre la fusion (ordre, dimensions, rotation, attributs hérités, doublons, absence de sources, écrasement refusé).
- `crates/pdfmerge-app/tests/cli.rs` couvre le comportement CLI (aide, codes de sortie, sélection invalide, sources manquantes, ordre, expansion de dossier, simulation, protections d'écrasement).
- GUI Slint implémentée : ajout de fichiers/dossiers, réorganisation, validation des sélections, dossier/fichier de sortie, progression et fusion dans un thread séparé.

Le fichier historique de contexte a été mis à jour pour refléter cette implémentation.

## Structure du projet

```
pdfmerge/
├── Cargo.toml                      # workspace virtuel (resolver = "3")
├── crates/
│   ├── pdfmerge-core/              # moteur métier, sans dépendance UI
│   │   └── src/
│   │       ├── lib.rs              # API publique
│   │       ├── page_spec.rs        # parseur + résolution de sélection
│   │       ├── sources.rs          # fichiers/dossiers, tri naturel
│   │       ├── merge.rs            # fusion lopdf + attributs hérités
│   │       ├── output.rs           # sortie par défaut, extension, validation
│   │       └── error.rs            # SpecError/MergeError/SourceError
│   └── pdfmerge-app/               # binaire unique CLI + GUI
│       ├── build.rs                # compilation Slint
│       ├── ui/main.slint           # fenêtre principale
│       └── src/
│           ├── main.rs             # slint::include_modules! et aiguillage
│           ├── cli.rs              # clap derive, expansion et fusion
│           └── gui.rs              # callbacks Slint et thread de fusion
├── doc/
│   └── cahier-des-charges-pdfmerge.md   # spécification fonctionnelle complète
├── Cargo.lock
└── target/                         # ignoré
```

## Configuration workspace

- `edition = "2024"`, `rust-version = "1.85"` (toolchain ≥ 1.85 requise).
- `resolver = "3"`.
- Les dépendances sont **centralisées** dans `[workspace.dependencies]` de la racine ; chaque crate les référence via `dep.workspace = true` (et `version.workspace`, `edition.workspace`, etc.).
- Lints workspace : `unsafe_code = "forbid"` ; clippy `unwrap_used`/`expect_used = "warn"`.
- L'app autorise ces lints dans `main.rs` parce que `slint::include_modules!()` étend du code généré.
- Profil release : `lto = true`, `codegen-units = 1`, `strip = true`.

Dépendances centrales disponibles (pas de les redéclarer en dur sauf besoin) : `lopdf 0.36`, `thiserror 2`, `anyhow 1`, `natord 1`, `chrono 0.4 (clock)`, `clap 4 (derive)`, `slint 1`, `slint-build 1`, `rfd 0.15`, `tempfile 3`, `proptest 1`, `assert_cmd 2`.

## Commandes de build / exécution / test

```bash
# Build du workspace
cargo build
cargo build --release

# Tests
cargo test
cargo test -p pdfmerge-core
cargo test -p pdfmerge-app --test cli
cargo test -- --nocapture   # pour les sorties

# Lints / format requis par le critère d'acceptation n°9
cargo fmt --check
cargo clippy -- -D warnings

# Lancement
cargo run -p pdfmerge-app              # sans args → GUI
cargo run -p pdfmerge-app -- --help    # CLI
```

Notes machine : `~/.cargo/bin` **n'est pas dans `PATH`**. Environnement de développement testé sur Ubuntu 26.04, toolchain Rust stable. Pas de `gh` CLI disponible.

## Conventions de développement

- Avant tout code notable, lire `doc/cahier-des-charges-pdfmerge.md` (spécification validée, à appliquer telle quelle).
- Conventions Rust : édition 2024, `rust-version = "1.85"` ; commits `git` clairs.
- Pas de `unwrap()`/`expect()` sur des données externes dans le code applicatif ; erreurs typées côté bibliothèque (`thiserror`) + `anyhow` dans les binaires.
- `unsafe_code = "forbid"` ; l'app repasse à `allow` uniquement pour le code généré par `include_modules!`, sans introduire d'`unsafe`.
- Le parseur de sélection et la résolution sont des **fonctions pures**, testées unitairement et avec `proptest`.
- Messages Git et sujets de commit en **anglais**, style Conventional Commits (`feat:`, `fix:`, …).
- Petites étapes compilables, chaque commit clair.
- Distinction CLI/GUI : un seul binaire. Sous Windows, `windows_subsystem = "windows"` masque la console ; prévoir `AttachConsole` en mode CLI ou deux binaires.

## Résumé des exigences clés (voir cahier des charges pour détails)

- **Syntaxe de sélection** (`page_spec`) : grammaire `item`/`plage`/`page` ; espaces ignorés ; crochets englobants tolérés ; `$` = dernière page. Plages ascendantes et **descendantes** (`b-a` donne b..a) volontaires. Numéros hors limites = erreur. Vide = toutes les pages. Messages d'erreur avec position.
- **Sources** : répertoire non récursif, extension `.pdf` insensible, tri alphabétique naturel (`natord`). Même fichier plusieurs fois avec sélections différentes.
- **Sortie** : dossier par défaut = dossier de la 1re source ; nom par défaut `fusion_AAAAMMJJ-HHMMSS.pdf` (heure locale) ; `.pdf` ajouté si absent ; refus si la sortie écraserait une source ; écrasement d'un autre fichier → erreur CLI code 4 sauf `--force`.
- **Orientation** : conservation de `MediaBox`/`CropBox`/`Resources`/`Rotate`, y compris les attributs hérités des parents.
- **Fusion** : renumérotation des objets ; matériauser attributs hérités ; écriture atomique via fichier temporaire puis `rename`.
- **CLI** : clap derive, options `-o/-d/-n/-f/--dry-run/-q/-v/--gui`. Codes de sortie 0–4. `--output` mutuellement exclusif avec `--output-dir/--name`.
- **GUI** : Slint, `rfd` pour les dialogues, thread séparé + `invoke_from_event_loop`, validation de la sélection, français, modèles `VecModel`.
- **Hors périmètre v1** : édition, OCR, compression, annotations, bookmarks/formulaires, mots de passe/signature, aperçu, récursion des sous-dossiers.

## Points d'attention techniques clés

- **Attributs hérités** : cœur de la conservation d'orientation. Couvert par tests dédiés.
- **Collisions d'objets** : renumérotation obligatoire.
- **PDF chiffrés/corrompus** : détecter et signaler, jamais de panic.
- **Output atomique** + chemins canoniques.
- **Unicode/espaces/accents/très longs chemins et extensions `.PDF`/`.Pdf`**.

## Processus de release

- Avant de tagger/pousser une release : exécuter localement `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, puis le build release Linux `cargo build --release -p pdfmerge-app --bin pdfmerge`.
- Le cross-build Windows local dépend de `x86_64-w64-mingw32-dlltool`. Si cet outil manque sur la machine, ne pas improviser d'installation locale ni bloquer la release : conserver les vérifications Linux puis déléguer l'étape Windows à GitHub Actions.
- Une fois les points critiques verts côté Linux, commiter la préparation, créer le tag `vX.Y.Z` et pousser la branche avec le tag. La CI/le workflow Release reste l'autorité pour `pdfmerge.exe` Windows et l'embarquement des icônes.
- Ne pas attendre ni poller l'exécution CI en boucle indéfiniment : déclencher le pipeline, informer l'utilisateur et laisser GitHub_actions produire les artefacts.

## Vérification locale recommandée avant commit

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

Les tests d'intégration CLI sont disponibles dans `crates/pdfmerge-app/tests/cli.rs` et utilisent `assert_cmd` + `tempfile`.
