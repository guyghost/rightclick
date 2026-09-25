# Surveillance de fichier et conservation du format — spécification

**Statut :** proposée pour revue

**Date :** 2026-09-25

**Projet :** RightClick

## Objectif

Quand un fichier ouvert dans RightClick change sur disque, recharger son
contenu si l’éditeur est propre et préserver les changements locaux en cas de
conflit. À l’ouverture et à la sauvegarde, conserver l’encodage et le style de
fin de ligne du fichier.

## Contexte actuel

`rc-document::Document` détecte et conserve l’encodage, mais `save_bytes()` ne
normalise pas les fins de ligne. `rc-editor` n’utilise pas ce résultat comme
métadonnée durable : `load_file` ne renvoie que le chemin et le texte, et
`save_file` écrit le contenu du widget en UTF-8. L’éditeur tient son état de
texte dans `iced::widget::text_editor::Content`, séparément du modèle
`Document`.

## Architecture

- `rc-document` porte un format de fichier composé de l’encodage et du style de
  fin de ligne. Le format est détecté depuis les octets lus et fournit une
  opération qui normalise puis encode un texte. `Document::from_bytes` et
  `Document::save_bytes` utilisent ce même format.
- `rc-editor` conserve le format détecté avec le texte de son widget et le
  transmet au chemin de sauvegarde. Il possède la surveillance du système de
  fichiers et transforme ses événements en messages de l’application iced.
- Un seul fichier est surveillé à la fois. Le watcher observe le dossier
  parent, filtre le chemin du document, et est remplacé à l’ouverture d’un
  autre fichier. Cela continue de détecter un remplacement du fichier par un
  outil qui sauvegarde en créant un fichier temporaire puis en le renommant.

Les notifications sont des signaux d’invalidation, pas le contenu du fichier.
Après regroupement des événements rapprochés, l’éditeur relit le fichier et
compare une empreinte des octets avec le dernier état disque connu. Les
événements répétés et ceux produits par une sauvegarde de RightClick sont
ignorés si les octets n’ont pas changé.

## États et comportement

La propreté est déterminée en comparant le texte actuel à une référence du
dernier contenu chargé ou sauvegardé. Elle ne repose pas seulement sur le
compteur de révision : revenir au contenu enregistré rend le document propre.

L’éditeur conserve deux références distinctes : le texte local de base qui
détermine la propreté, et le dernier état disque observé (empreinte des octets
ou absence du fichier) qui élimine les notifications répétées. Lorsqu’un
changement disque réel est détecté :

1. Si le texte local est propre, l’éditeur recharge le texte, son encodage et
   son style de fin de ligne, puis met à jour ses références propres et disque.
2. Si le texte local est modifié, l’éditeur conserve le texte local et affiche
   un conflit. Deux actions explicites sont proposées : **Recharger depuis le
   disque**, qui abandonne les changements locaux, et **Écraser sur le
   disque**, qui enregistre la version locale. Le bouton de sauvegarde normal
   reste désactivé tant que le conflit n’est pas résolu.
3. À la détection d’un conflit, la référence disque avance vers la dernière
   version observée, tandis que le texte local de base reste inchangé. Un
   conflit reste actif jusqu’à une action explicite, même si une annulation
   ramène ensuite le texte local à son contenu de base.
4. Après un rechargement ou un écrasement réussi, le conflit est effacé et les
   références de contenu et d’octets disque sont mises à jour.

Avant une sauvegarde normale, l’éditeur vérifie que les octets disque
correspondent toujours à la dernière version connue. Si le disque a changé
entre la dernière notification et le clic, la sauvegarde est refusée et le
conflit est présenté. L’action explicite **Écraser sur le disque** permet
alors de confirmer le remplacement.

Une suppression ou une erreur de lecture ne remplace jamais le contenu ouvert.
L’éditeur conserve le texte, indique l’état d’erreur et garde le watcher actif
sur le dossier parent afin de détecter un éventuel retour du fichier. Une
sauvegarde explicite peut recréer un fichier supprimé.

## Fins de ligne et encodage

- Le premier séparateur rencontré (`CRLF`, `CR` ou `LF`) choisit le style du
  document, conformément au comportement actuel de `Newline::detect`.
- À la sauvegarde, chaque séparateur du texte (`CRLF`, `CR` ou `LF`) est
  converti vers le style choisi. La présence ou l’absence d’un séparateur
  final reste identique.
- Si le fichier chargé ne contient aucun séparateur, le style par défaut est
  `LF` pour les fins de ligne insérées ensuite.
- L’encodage détecté, y compris son BOM éventuel, est conservé à la
  sauvegarde. Les règles de remplacement existantes de l’encodage
  Windows-1252 pour les caractères non représentables ne changent pas dans
  cette feature.

## Erreurs et cycle de vie

- Une erreur de création ou de configuration du watcher est affichée sans
  empêcher l’ouverture ou la sauvegarde du document. La détection des
  modifications externes est alors indisponible pour ce fichier.
- Une erreur de lecture d’une version externe laisse le texte ouvert intact et
  expose l’erreur à l’utilisateur.
- Le watcher précédent est libéré lorsque l’utilisateur ouvre un autre
  fichier. Les messages retardés sont ignorés s’ils concernent un chemin qui
  n’est plus ouvert.
- Une sauvegarde asynchrone mémorise le texte exact qu’elle écrit. Si
  l’utilisateur poursuit la saisie pendant cette sauvegarde, le document ne
  devient propre que si son texte courant correspond encore au texte écrit.

## Vérification et critères d’acceptation

- Tests unitaires de normalisation : fins mixtes, `CRLF` traité comme un seul
  séparateur, style cible `CRLF`/`CR`/`LF`, et conservation du saut final.
- Tests de format : aller-retour des fichiers UTF-8, UTF-8 avec BOM, UTF-16 et
  Windows-1252 avec style de fin de ligne conservé.
- Tests de transition d’état : événement sans différence d’octets ignoré,
  document propre rechargé, document modifié préservé avec conflit, mises à
  jour externes répétées sans perdre la dernière référence disque, action de
  rechargement qui remplace l’état local, écrasement explicite qui met à jour
  la référence disque, et échec de lecture qui préserve le texte.
- Vérification d’intégration sur fichiers temporaires : modification externe,
  remplacement par renommage, suppression/recréation et sauvegarde propre de
  RightClick.
- L’ouverture, l’édition et l’enregistrement existants continuent de
  fonctionner lorsque le watcher n’est pas disponible.

## Hors périmètre

- Fusion automatique du texte local et du texte disque.
- Plusieurs onglets ou plusieurs fichiers ouverts simultanément.
- Réglages utilisateur pour choisir un encodage ou un style de fin de ligne.
- Parsing asynchrone, rendu `rc-layout`, et autres éléments de la feuille de
  route.

## Limites connues

Le système de fichiers peut changer immédiatement après la vérification
préalable à la sauvegarde. L’écrasement explicite assume ce cas. Une fusion
optimiste atomique avec verrouillage ou comparaison conditionnelle n’est pas
incluse.
