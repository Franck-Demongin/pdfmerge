# Prompt : refonte visuelle de la GUI de PDFMerge

## 1. Contexte

PDFMerge est une application Rust (édition 2024) en workspace Cargo :

- `crates/pdfmerge-core` : moteur (parseur de pages, fusion lopdf, sources, sortie) ;
- `crates/pdfmerge-app` : binaire `pdfmerge`, CLI (clap) + GUI (Slint).

L'application fonctionne. Cette tâche concerne **uniquement la GUI** : son apparence, son ergonomie et quelques fonctionnalités pratiques. Le comportement de la CLI et le moteur ne changent pas, sauf mention explicite.

La GUI actuelle est fonctionnelle mais austère : tout est posé à plat sur un fond sombre, sans regroupement visuel. Il faut lui donner une identité soignée, décrite au §5, et poser un **système de design réutilisable** (thème + composants) pour pouvoir le réutiliser dans d'autres applications Rust + Slint du même auteur.

Disposition : **une seule colonne**.

## 2. Objectif

Obtenir une GUI sobre, aérée et lisible : thème sombre, contenu regroupé en panneaux titrés, champs et boutons homogènes, un unique bouton d'action coloré, plus quelques fonctionnalités pratiques (§8).

## 3. Problèmes à corriger dans la GUI actuelle

À vérifier dans le code actuel (`crates/pdfmerge-app/ui/*.slint`, `gui.rs`) :

| Constat | Correction attendue |
|---------|---------------------|
| Aucun panneau : tout est posé à plat sur le fond | Regrouper en panneaux (cartes) titrés |
| Les deux gros boutons « Ajouter des fichiers… » / « Ajouter un dossier… » flottent au-dessus de la liste | Les placer dans l'en-tête du panneau « Sources » |
| Style des champs incohérent : « Nom du fichier » utilise un champ d'un autre style (soulignement bleu au focus, croix d'effacement) que les autres | Un seul composant de champ texte partout |
| Labels alignés en haut des champs, pas centrés verticalement | Centrer verticalement labels et contrôles sur une même ligne |
| Texte « 1 page(s) » à droite des lignes presque invisible (contraste trop faible) | Contraste lisible pour le texte secondaire, pluriel correct (« 1 page », « 5 pages ») |
| Nombre de pages du document affiché seul, sans libellé | Badge explicite « 52 p. » |
| Compteur « 6/6 » dans le pied de page ambigu | Remplacer par « 2 sources · 6 pages » |
| Bouton « Fusionner » gris, noyé dans le décor | Bouton **primaire** coloré (couleur d'accent) |
| Textes sans accents (« 2 source(s) prete(s). ») | Tous les textes en français correct, avec accents |
| Pas de bouton « Parcourir… » pour le dossier de sortie | Ajouter |
| Nom de fichier par défaut affiché `fusion` (sans horodatage ni `.pdf`) | Vérifier la règle du cahier des charges : défaut `fusion_AAAAMMJJ-HHMMSS`, extension `.pdf` affichée en suffixe non éditable |
| Grand vide au milieu quand la liste est courte ou vide, aucun état vide | Ajouter un état vide explicite (voir §7) |

## 4. Description visuelle cible

Aspect général : **sombre, plat, sobre**, sans ombres portées marquées ni dégradés. L'espace est aéré, les éléments sont alignés, la hiérarchie est claire.

- **Fenêtre** : fond gris quasi noir.
- **Panneaux** : rectangles à coins arrondis, d'un gris à peine plus clair que le fond, avec une fine bordure de 1 px légèrement plus claire. Chaque panneau porte un **titre en gras** en haut à gauche. Les panneaux sont séparés par un petit espace vertical régulier.
- **Champs de saisie** : rectangles à coins arrondis, gris foncé un peu plus clair que le panneau, avec une bordure fine. Le texte est clair ; le texte d'indication (placeholder) est gris moyen. Au focus, la bordure prend la couleur d'accent.
- **Boutons secondaires** : même gabarit que les champs (gris foncé, coins arrondis, même hauteur), texte clair, légère variation de teinte au survol.
- **Bouton primaire** : rectangle arrondi **bleu ciel clair** avec texte **bleu-noir sombre**, placé en bas à droite de la fenêtre. Il est unique dans l'écran et constitue le seul grand aplat de couleur.
- **Cases à cocher et éléments actifs** : même bleu ciel.
- **Pied de page** : séparé du contenu par un **fin trait horizontal**. À gauche, le message d'état en texte courant ; à droite, le bouton primaire.
- **Texte secondaire** (notes d'aide, chemins, résultats de validation) : petit, gris moyen, mais **toujours lisible**.
- **Typographie** : police système par défaut, sans police embarquée ; titres de panneau en gras, un peu plus grands que le texte courant.

## 5. Jetons de design (`Theme`)

Ces valeurs sont **la référence** : les reprendre telles quelles.

| Jeton | Valeur | Usage |
|-------|--------|-------|
| `bg` | `#1c1c1c` | Fond de fenêtre |
| `panel` | `#232323` | Fond des panneaux |
| `panel-border` | `#2f2f2f` | Bordure 1 px des panneaux |
| `control` | `#2d2d2d` | Fond des champs et boutons secondaires |
| `control-hover` | `#363636` | Survol |
| `control-border` | `#3a3a3a` | Bordure 1 px des champs |
| `text` | `#e8e8e8` | Texte principal |
| `text-muted` | `#9a9a9a` | Texte secondaire, notes, placeholders |
| `accent` | `#4fc3f7` | Bouton primaire, focus, cases cochées |
| `on-accent` | `#0b1d26` | Texte sur fond accent |
| `error` | `#ef5350` | Erreurs |
| `success` | `#66bb6a` | Succès |
| `radius-panel` | `8px` | Coins des panneaux |
| `radius-control` | `6px` | Coins des champs et boutons |
| `control-height` | `32px` | Hauteur des champs et boutons |
| `padding-window` | `12px` | Marge extérieure de la fenêtre |
| `padding-panel` | `12px` | Marge intérieure des panneaux |
| `gap` | `12px` | Espace entre panneaux |
| `gap-small` | `8px` | Espace entre contrôles |
| `font-size` | `14px` | Texte courant |
| `font-size-title` | `16px` | Titres de panneau (gras) |
| `font-size-small` | `12px` | Notes, texte secondaire |

Règles :
- Aucune couleur ni dimension en dur en dehors de `theme.slint`.
- Thème **sombre uniquement**, sans dépendre du thème système.
- Les composants sont **maison** (pas de widgets standards pour les éléments visibles). Si des widgets de `std-widgets` restent nécessaires (ex. barre de progression), forcer un style sombre unique via `slint_build::CompilerConfiguration::new().with_style("fluent-dark".into())` dans `build.rs`, et les habiller avec les jetons.

## 6. Architecture UI

Dans `crates/pdfmerge-app/ui/` :

```
ui/
├── theme.slint        # export global Theme { ... } : jetons du §5
├── components.slint   # composants réutilisables, sans logique métier
├── main.slint         # fenêtre principale (assemblage)
└── (éventuellement) source_row.slint
```

Squelette attendu pour `theme.slint` :

```slint
export global Theme {
    out property <color> bg: #1c1c1c;
    out property <color> panel: #232323;
    out property <color> accent: #4fc3f7;
    out property <color> on-accent: #0b1d26;
    out property <length> radius-panel: 8px;
    out property <length> control-height: 32px;
    // ... tous les jetons du §5
}
```

Composants à créer dans `components.slint` :

- `Panel` : titre, contenu en slot, zone d'actions optionnelle à droite du titre ;
- `AppButton` (secondaire) et `PrimaryButton` (accent), avec états normal / survol / appui / désactivé ;
- `TextField` : **champ unique pour toute l'application** (label optionnel, placeholder, état d'erreur en bordure `error`, suffixe optionnel comme `.pdf`) ;
- `IconButton` : petit bouton carré pour ↑ ↓ ✕ ;
- `Badge` : petite pastille de texte (nombre de pages) ;
- `HintText` : note secondaire ;
- `StatusBar` : pied de page (trait de séparation, message, progression, boutons d'action).

**Réutilisabilité** : `theme.slint` et `components.slint` ne doivent contenir **aucune référence à PDFMerge ni au métier** (pas de mot « fusion », « PDF », etc.), pour pouvoir être copiés tels quels dans un autre projet ou extraits plus tard dans un crate commun. Ne pas faire cette extraction maintenant, mais ne rien faire qui l'empêche.

## 7. Disposition cible (une seule colonne)

```
┌─ Sources · 2 fichiers ──────────────────────────────────┐
│ [Ajouter des fichiers…] [Ajouter un dossier…]   [Vider] │
│ ┌─────────────────────────────────────────────────────┐ │
│ │ ↑ ↓  menu.pdf              [1 p.]  [ toutes ] 1 page │ │
│ │      /home/franck/Bureau                          ✕ │ │
│ │ ↑ ↓  100_Prompts….pdf     [52 p.]  [1-3,51-$] 5 pages│ │
│ └─────────────────────────────────────────────────────┘ │
│ Syntaxe : 3-6,1,4-$ · $ = dernière page · 6-3 = ordre inversé │
└─────────────────────────────────────────────────────────┘
┌─ Sortie ────────────────────────────────────────────────┐
│ Dossier  [ /home/franck/Bureau            ] [Parcourir…]│
│ Fichier  [ fusion_20261022-143507         ] .pdf        │
└─────────────────────────────────────────────────────────┘
─────────────────────────────────────────────────────────
● 2 sources · 6 pages        [progression]   [Fusionner]
```

Détails :

- **Panneau « Sources »** : le titre porte un compteur. Les boutons d'ajout sont dans l'en-tête du panneau. La liste est dans une zone **défilante** qui occupe l'espace disponible.
- **Ligne de source** : contenus centrés verticalement. Nom du fichier en texte principal, dossier parent en `text-muted` dessous, avec **troncature par ellipse**. Badge du nombre de pages du document. Champ « Pages » avec placeholder `toutes`. À droite, résultat de la validation : `→ 5 pages` en `text-muted`, ou message d'erreur en `error` avec bordure rouge sur le champ. Boutons ↑ ↓ ✕ en `IconButton`, désactivés quand non pertinents (↑ sur la 1re ligne, ↓ sur la dernière).
- **État vide** : texte centré dans le panneau (« Aucun fichier. Ajoutez des PDF ou un dossier pour commencer. ») avec les deux boutons d'ajout bien visibles.
- **Panneau « Sortie »** : labels alignés sur une même colonne, champs de même largeur, bouton « Parcourir… » pour le dossier, suffixe `.pdf` non éditable après le nom.
- **Pied de page** : trait de séparation, message d'état à gauche (couleur selon le niveau : neutre, `error`, `success`), barre de progression pendant la fusion, bouton primaire « Fusionner » à droite, désactivé tant que la liste est vide ou qu'une sélection est invalide.
- **Fenêtre** : redimensionnable, taille minimale raisonnable (environ 720×520) ; le panneau « Sources » absorbe l'espace supplémentaire.

## 8. Fonctionnalités pratiques à ajouter

Par ordre de priorité. Les trois premières sont attendues ; les suivantes sont à implémenter si elles restent simples, sinon à lister dans le README.

1. **Fin de fusion utile** : après un succès, message vert avec le nom du fichier créé, et boutons « Ouvrir le fichier » et « Ouvrir le dossier » (crate `opener`).
2. **Progression** : barre de progression, boutons d'ajout et de fusion désactivés pendant le traitement (le traitement est déjà dans un thread).
3. **Gestion de l'écrasement** : si le fichier de sortie existe, dialogue de confirmation (Écraser / Annuler) ; refus explicite si la sortie correspond à une source.
4. **Dupliquer une ligne** : bouton pour ajouter à nouveau le même fichier avec une autre sélection de pages (une même source peut apparaître plusieurs fois).
5. **Vider la liste** : bouton dans l'en-tête du panneau « Sources ».
6. **Dossier de sortie automatique** : tant que l'utilisateur ne l'a pas modifié, il suit le dossier de la première source ; une fois modifié à la main, il n'est plus écrasé.
7. **Mémoire du dernier dossier** utilisé dans les dialogues de sélection (en mémoire pour la session, pas de fichier de configuration).
8. **Glisser-déposer** de fichiers/dossiers sur la fenêtre, **uniquement si** le backend Slint utilisé le permet proprement ; sinon l'ignorer.
9. **Raccourcis clavier** : Entrée dans un champ de pages valide la saisie ; Ctrl+O ouvre « Ajouter des fichiers… ».

Hors périmètre : prévisualisation des pages, panneau « Récapitulatif » séparé (remplacé par la ligne de synthèse du pied de page), thème clair.

## 9. Contraintes

- Ne pas modifier `pdfmerge-core` sauf besoin minimal et justifié (par exemple exposer des informations déjà calculées). Aucune logique métier dans les `.slint`.
- Tous les textes de l'interface en **français correct (accents compris)**, enveloppés dans `@tr()`.
- Pas de `unwrap()` / `expect()` sur des données externes ; `cargo fmt`, `cargo clippy -- -D warnings` et `cargo test` doivent passer.
- Pas de nouvelle dépendance autre que `opener` sans justification ; pour ↑ ↓ ✕, préférer des caractères Unicode ou des SVG embarqués.
- Le comportement de la CLI et les codes de sortie restent identiques.

## 10. Plan de travail

1. Lire les fichiers `.slint` et `gui.rs` actuels pour repérer les propriétés et callbacks existants.
2. Créer `theme.slint` et `components.slint`.
3. Reconstruire `main.slint` avec les panneaux et le pied de page, en conservant les callbacks et propriétés actuels pour ne pas casser `gui.rs` (ou en adaptant `gui.rs` de façon minimale).
4. Ajouter les fonctionnalités par ordre de priorité (§8), une par une, en restant compilable à chaque étape.
5. Mettre à jour le README : liste des composants, jetons de design, fonctionnalités optionnelles non réalisées.

Livrer à chaque étape un résumé court de ce qui a changé et des fichiers touchés.

## 11. Critères d'acceptation

1. L'interface est sombre, plate et sobre : fond quasi noir, panneaux arrondis légèrement plus clairs avec fine bordure et titre en gras, champs et boutons homogènes, un seul bouton bleu ciel (« Fusionner ») en bas à droite.
2. Les contrôles sont alignés verticalement et partagent un seul style de champ texte ; le soulignement bleu de l'ancien champ a disparu.
3. Le bouton « Fusionner » est le seul aplat de couleur d'accent et ne s'active que si la liste est valide.
4. Aucun texte illisible (contraste du texte secondaire correct), aucun texte sans accent, pluriels corrects.
5. Les noms et chemins longs sont tronqués proprement, sans déformer la ligne.
6. L'état vide, l'état d'erreur d'une ligne, la progression et le succès (avec ouverture du fichier/dossier) sont tous visibles et testés manuellement.
7. `theme.slint` et `components.slint` ne contiennent rien de spécifique à PDFMerge.
8. `cargo fmt --check`, `cargo clippy -- -D warnings` et `cargo test` passent ; la CLI se comporte comme avant.
