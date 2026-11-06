# Murmure Hospital

Adaptation de [Murmure](https://github.com/Kieirra/murmure) pour Vauclaire, dans le cadre de la reconnaissance vocale portée par l'intelligence artificielle.

L'objectif est d'effectuer les calculs de reconnaissance sur les postes utilisateurs, pour répartir la charge et traiter les dictées localement, sans API de transcription cloud.

## Capitalisation sur Murmure

Après étude de Handy et Murmure, nous avons retenu **Murmure pour son dictionnaire personnalisable et ses commandes vocales de mise en forme**.

Cette distribution repose sur la **version officielle 2.0.3**. Elle conserve le moteur Parakeet, l'insertion du texte dans les applications et l'architecture React/TypeScript + Rust/Tauri. Les adaptations restent limitées pour faciliter la maintenance et l'intégration des futures évolutions officielles.

La branche `main` conserve la référence officielle ; `hospital/main` regroupe les adaptations hospitalières. Les mises à jour du projet d'origine doivent être examinées et testées avant intégration.

## Changements effectués

- **Interface simplifiée** : accès à l'accueil, au dictionnaire, aux règles personnalisées et à une sélection de paramètres système. Les extensions, règles générales, réglages des raccourcis et import/export global de configuration sont masqués.
- **Réglages conservés** : microphone, sons et volume, mode de dictée « appuyer pour parler » ou bascule, insertion Standard ou Direct, conversion des nombres avec langue et seuil.
- **Personnalisation visuelle** : ajout du logo Vauclaire à côté de celui de Murmure, avec une taille harmonisée.
- **Liens et mises à jour** : retrait des accès « Signaler un bug », « Notes de version » et « Vérifier les mises à jour » dans la sidebar. Le lien « Soutenir Murmure » est conservé et le composant de vérification des mises à jour n'est plus monté.
- **Configuration initiale commune** : paramètres, dictionnaire et commandes vocales embarqués lors du build, puis utilisés pour initialiser les fichiers utilisateur absents.
- **Confidentialité** : historique persistant, conservation des enregistrements audio, logs, API locale et Smart Speech Mic désactivés par défaut.
- **Build Windows manuel sur GitHub Actions**, avec choix d'un installateur EXE ou MSI et tests ciblés sur la configuration.

Les restrictions de ce premier jalon portent sur l'interface : les fonctionnalités masquées et le plugin natif de mise à jour restent présents dans le backend. Il ne s'agit pas encore d'une politique de verrouillage administrée.

### Fichiers de référence à préparer avant le build

| Fichier                                                                                                | Rôle                                                                                                     |
| ------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| [`resources/hospital/default-settings.json`](resources/hospital/default-settings.json)                 | Paramètres initiaux de l'application.                                                                    |
| [`resources/hospital/default-dictionnary.json`](resources/hospital/default-dictionnary.json)           | Dictionnaire initial, incluant les expressions des commandes vocales pour favoriser leur reconnaissance. |
| [`resources/hospital/default-formatting-rules.json`](resources/hospital/default-formatting-rules.json) | Règles initiales de ponctuation, retours à la ligne, paragraphes et guillemets.                          |

Ces fichiers utilisent les formats JSON natifs de Murmure. Chaque fichier utilisateur absent est initialisé indépendamment ; les fichiers existants ne sont pas écrasés par ce mécanisme.

Sous Windows, les fichiers actifs sont `settings.json`, `dictionary.json` et `formatting_rules.json`, dans `%APPDATA%\com.al1x-ai.murmure\`. Réinstaller l'application ne réinitialise pas un profil existant. Pour tester les valeurs initiales, quitter complètement Murmure puis renommer ce dossier pour en conserver une sauvegarde avant de relancer l'application.

## Lancer en local

### Prérequis

- Git, Node.js 22 et pnpm 10.19.0.
- Rust et Cargo ; le workflow Windows utilise Rust 1.98.1.
- Les [dépendances système de Tauri 2](https://v2.tauri.app/start/prerequisites/) pour votre système. Sous Ubuntu, prévoir également les bibliothèques de développement audio et clavier, notamment `libasound2-dev` et `libxkbcommon-dev`.
- Sous Windows : outils de compilation C++ MSVC, WebView2 et NASM, comme dans le workflow.

### Récupérer le projet et le modèle

```sh
git clone --branch hospital/main https://github.com/dev-vauclaire/murmure-hospital.git murmure
cd murmure
```

Si le dépôt est déjà cloné, se placer dans sa racine, dans le dossier contenant `package.json`.

Télécharger le [modèle Parakeet 1.3.0 utilisé par cette version](https://github.com/Kieirra/murmure-model/releases/download/1.3.0/parakeet-tdt-0.6b-v3-int8.zip), puis extraire son dossier dans `resources/` pour obtenir :

```text
resources/parakeet-tdt-0.6b-v3-int8/
├── encoder-model.int8.onnx
├── decoder_joint-model.int8.onnx
├── nemo128.onnx
└── vocab.txt
```

### Démarrer l'application

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

L'installation prépare aussi les ressources SmartMic attendues par le projet d'origine. Si elle a été faite avec `--ignore-scripts`, exécuter `pnpm build:smartmic` avant de démarrer Tauri.

`pnpm tauri dev` lance Vite et l'application de bureau, avec rechargement des modifications du frontend. `pnpm dev` seul lance uniquement le frontend et ne permet pas de tester les fonctions natives. La première compilation Rust peut prendre plusieurs minutes.

Sous Ubuntu, si une erreur `GLIBC_PRIVATE` mentionne une bibliothèque provenant de `/snap/`, relancer depuis un terminal système extérieur à une application installée via Snap.

## Logique de build sur GitHub Actions

Le workflow [`Build Windows hospitalier`](.github/workflows/build-hospital-windows.yml) compile l'application sur un runner Windows, même lorsque le développement se fait sous Linux. Il se déclenche manuellement.

### Lancer un build

1. Préparer les trois fichiers JSON de référence, puis faire commiter et pousser les modifications sur la branche à construire. Le workflow utilise les fichiers présents sur GitHub, pas ceux restés uniquement en local.
2. Ouvrir [Actions → Build Windows hospitalier](https://github.com/dev-vauclaire/murmure-hospital/actions/workflows/build-hospital-windows.yml).
3. Cliquer sur **Run workflow**, sélectionner **`hospital/main`** et choisir `installer_format` :
    - **`exe`** : installateur NSIS pour le compte utilisateur courant, choix par défaut.
    - **`msi`** : installateur pour la machine.
4. Lancer le workflow, attendre sa réussite, puis télécharger l'artefact dans la page de l'exécution.

### Ce que fait le workflow

Il vérifie les trois JSON de référence suivis dans Git, prépare les outils et dépendances, télécharge le modèle Parakeet, construit les ressources SmartMic et exécute les tests Rust ciblés des paramètres, du dictionnaire, du formatage et de l'initialisation.

Tauri compile ensuite le frontend et le backend pour **Windows x64**, embarque le modèle et les fichiers de référence, puis génère l'installateur choisi.

L'artefact `murmure-hospital-windows-x64-<format>-<commit>` est conservé **7 jours** et contient :

- L'installateur EXE ou MSI **non signé**. L'EXE est un installateur, pas une application portable.
- `SHA256SUMS.txt`, pour contrôler l'intégrité du fichier.
- `build-info.json`, avec le commit, la version et les informations de construction.

Ce workflow ne crée pas de Release et ne déploie rien sur les postes. Les essais fonctionnels restent à effectuer sur Windows, notamment le chargement des valeurs initiales sur un profil neuf et la conservation des préférences sur un profil existant.

## Contributions, licences et attributions

Les retours, corrections et améliorations sont les bienvenus sur [ce dépôt](https://github.com/dev-vauclaire/murmure-hospital). Voir [CONTRIBUTING.md](CONTRIBUTING.md) et [GUIDELINES.md](GUIDELINES.md) pour les conventions. Utiliser uniquement des données fictives ou anonymisées pour les tests et contributions.

- Murmure et cette adaptation sont distribués sous **GNU AGPL v3** ; voir [LICENSE](LICENSE). Merci aux auteurs et contributeurs du [projet officiel](https://github.com/Kieirra/murmure).
- Le modèle repose sur [Parakeet de NVIDIA](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), avec le modèle affiné [parakeet-ultra de Moondream](https://huggingface.co/moondream/parakeet-ultra) et sa [conversion ONNX int8 par @thiswillbeyourgithub](https://huggingface.co/Olicorne/parakeet-tdt-0.6b-v3-ultra-onnx), sous [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/), conformément aux attributions du projet d'origine.
- L'application utilise [Tauri](https://tauri.app/). La [politique de confidentialité du projet d'origine](PRIVACY_POLICY.md) est conservée dans le dépôt.
- Pour soutenir le développement de Murmure : [page de soutien officielle](https://fr.tipeee.com/murmure-al1x-ai/).
