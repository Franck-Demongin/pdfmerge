# QWEN.md — Contexte pour le dépôt `pdfmerge`

Fichier d'instructions destiné aux futures sessions Qwen sur ce dépôt. Résumé du projet, de sa structure actuelle et des conventions de travail.

## Vue d'ensemble du projet

**PDFMerge** est un outil Rust de fusion de PDF avec sélection et réorganisation de pages par source. Pour chaque fichier source, l'utilisateur décrit les pages souhaitées avec une syntaxe souple (plages ascendantes/descendantes, numéros isolés, doublons, `$` pour la dernière page). Les pages sélectionnées sont concaténées dans l'ordre des entrées sources.

Deux modes d'usage, partageant **exactement le même moteur** (`pdfmerge-core`) :
- **GUI** Slint (usage interactif)
- **CLI** (automatisation/scripts)

Le cahier des charges détaillé est dans `doc/cahier-des-charges-pdfmerge.md` (à consulter pour tout détail fonctionnel).

## État actuel du dépôt

Squelette **non implémenté** à ce jour : les deux crates compilent mais ne contiennent que du code template.
- `crates/pdfmerge-core/src/lib.rs` : fonction `add` générique + test unique.
- `crates/pdfmerge-app/src/main.rs` : `println!("Hello, world!")`.

La structure de production prévue (`page_spec.rs`, `sources.rs`, `merge.rs`, `output.rs`, `error.rs`, `cli.rs`, `gui.rs`, `build.rs`, `ui/main.slint`) n'existe pas encore. À développer selon les §6.1 et §10 du cahier des charges, **en commençant par `pdfmerge-core`** (d'abord `page_spec` avec tests exhaustifs).

## Structure du projet

```
pdfmerge/
├── Cargo.toml                      # workspace virtuel (resolver = "3")
├── crates/
│   ├── pdfmerge-core/              # moteur métier, sans dépendance UI
│   │   └── src/lib.rs              # (stub actuel)
│   └── pdfmerge-app/               # binaire unique CLI+GUI
│       └── src/main.rs             # (stub actuel)
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
- Profil release : `lto = true`, `codegen-units = 1`, `strip = true`.

Dépendances centrales disponibles (pas de les redéclarer en dur sauf besoin) : `lopdf 0.36`, `thiserror 2`, `anyhow 1`, `natord 1`, `chrono 0.4 (clock)`, `clap 4 (derive)`, `slint 1`, `slint-build 1`, `rfd 0.15`, `tempfile 3`, `proptest 1`, `assert_cmd 2`.

## Commandes de build / exécution / test

```bash
# Build du workspace
cargo build
cargo build --release

# Tests (core + app)
cargo test
cargo test -p pdfmerge-core
cargo test -- --nocapture   # pour les sorties

# Lints / format requis par le critère d'acceptation n°9
cargo fmt --check
cargo clippy -- -D warnings

# Lancement (stub actuel)
cargo run -p pdfmerge-app              # sans args → GUI prévue
cargo run -p pdfmerge-app -- --help   # CLI (une fois implémentée)
```

Notes machine : `~/.cargo/bin` **n'est pas dans `PATH`** (voir mémoires). Les binaires installés via `cargo install` nécessitent soit l'export `PATH`, soit un lien symbolique vers `~/.local/bin`. Environnement de dev : Ubuntu 26.04, toolchain Rust stable 1.99 (1.89+ OK). Pas de `gh` CLI disponible.

## Conventions de développement

- Avant tout code notable, lire `doc/cahier-des-charges-pdfmerge.md` (spécification validée, à appliquer telle quelle).
- Conventions Rust : édition 2024, `rust-version = "1.85"` ; atomic commits clairs.
- Pas de `unwrap()`/`expect()` sur des données externes ; erreurs typées côté bibliothèque (`thiserror`) + `anyhow` dans les binaires.
- `unsafe_code = "forbid"`.
- Lints clippy traitent `unwrap_used`/`expect_used` comme warnings ; le cas `-- -D warnings` doit passer avant livraison.
- Pas d'introduction de dépendance non listée sans justification (§10.5).
- Le parseur de sélection et la résolution sont des **fonctions pures**, faciles à tester unitairement + `proptest`.
- Messages Git et sujets de commit en **anglais**, style Conventional Commits (`feat:`, `fix:`, …).
- Commencer par `pdfmerge-core` (`page_spec` d'abord) puis `merge`, puis CLI, puis GUI (§10).
- Petites étapes compilables, chaque commit clair.
- Distinction CLI/GUI : un seul binaire. Sous Windows, `windows_subsystem = "windows"` masque la console ; prévoir `AttachConsole` en mode CLI ou deux binaires.

## Résumé des exigences clés (voir cahier des charges pour détails)

- **Syntaxe de sélection** (`page_spec`) : grammaire `item`/`plage`/`page` ; espaces ignorés ; crochets englobants tolérés ; `$` = dernière page. Plages ascendantes et **descendantes** (`b-a` donne b..a) volontaires. Numéros hors limites = erreur. Vide = toutes les pages. Messages d'erreur avec position. Tests + `proptest` (toute page dans `[1,n]`).
- **Sources** : répertoire non récursif, extension `.pdf` insensible, tri alphabétique naturel (`natord`). Même fichier plusieurs fois avec sélections différentes. Police→tri.
- **Sortie** : dossier par défaut = dossier de la 1re source ; nom par défaut `fusion_AAAAMMJJ-HHMMSS.pdf` (heure locale) ; horodatage à lancement de la fusion ; `.pdf` ajouté si absent ; refus si la sortie écraserait une source (comparaison canonique) ; écrasement autre fichier → confirmation GUI / erreur CLI sauf `--force`.
- **Orientation** : conservation de `MediaBox`/`CropBox`/`Resources`/**`Rotate`** (hérités des parents → résoudre et copier sur chaque page). `Rotate` hérité = point critique.
- **Fusion** : renumérotation des objets pour éviter collisions ; matériauser attributs hérités ; pages dupliquées = cloner le dict de page mais partager contenu/ressources ; écriture atomique via fichier temporaire puis rename.
- **CLI** : clap derive, `pdfmerge pdf.pdf[3-6,1,4-$]` (dernier `[…]` final), options `-o/-d/-n/-f/--dry-run/-q/-v/--gui`. Codes de sortie 0–4. Mutuellement exclusifs `--output` et `--output-dir/--name`.
- **GUI** : Slint, `rfd` pour dialogues, thread séparé + `invoke_from_event_loop`, validation à la frappe, français (`@tr()`), modèles `VecModel`/`ModelRc`.
- **Hors périmètre v1** : édition, OCR, compression, annotations, bookmarks/formulaires/a11y, mots de passe/signature, aperçu, récursion des sous-dossiers.

## Points d'attention techniques clés

- **Attributs hérités** (§7.1) : le cœur de la conservation d'orientation. À couvrir par un test dédié (portrait+paysage, `Rotate` 90/180/270).
- **$ canonique selon le document** : `$` = dernière page *du document courant*.
- **Collisions d'objets** : renumérotation obligatoire.
- **PDF chiffrés/corrompus** : détecter et signaler, jamais de panic.
- **Output atomique** + chemins canoniques.
- **Unicode/espaces/accents/très longs chemins et extensions `.PDF`/`.Pdf`**.

## Vérification locale recommandée avant commit

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

Le Go/auxiliaire CI n'existe pas encore ; tests d'intégration prévus via `assert_cmd` + `tempfile` dans `tests/`.