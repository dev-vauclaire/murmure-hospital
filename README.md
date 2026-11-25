# Murmure Vauclaire

**Sommaire**

1. [Présentation](#présentation)
2. [Adaptations Vauclaire](#adaptations-vauclaire)
3. [Lancer en développement](#lancer-en-développement)
4. [Configuration initiale](#configuration-initiale)
5. [Construire l’installateur Windows](#construire-linstallateur-windows)
6. [Documentation, contributions et crédits](#documentation-contributions-et-crédits)

## Présentation

Murmure Vauclaire est une adaptation de [Murmure](https://github.com/Kieirra/murmure), initialement basée sur la version officielle **2.0.3**.

La dictée utilise le moteur local **Parakeet** pour répartir les calculs sur les postes utilisateurs, sans API de transcription cloud. Murmure a été retenu pour son dictionnaire personnalisable et ses commandes vocales de mise en forme.

## Adaptations Vauclaire

- Interface simplifiée : accueil, dictionnaire, commandes vocales et réglages essentiels.
- Identité visuelle associant les logos Murmure et Vauclaire.
- Paramètres, dictionnaire et commandes vocales préconfigurés.
- Raccourcis personnalisables et support du **Philips SpeechMike SMP3700 sous Windows**.
- Identité d’application distincte et retrait du système de mise à jour intégré : les nouvelles versions passent par un installateur validé.
- API locale activable avec **Partage Murmure**, avec une route de disponibilité `GET /api/health`.
- Sous Windows, démarrage à l'ouverture de session activé par défaut et personnalisable pour chaque utilisateur.

La simplification de l’interface ne constitue pas un verrouillage des fichiers de préférences.

## Lancer en développement

Prérequis : **Git, Node.js 22, pnpm 10.19.0, Rust/Cargo** et les [dépendances système de Tauri 2](https://v2.tauri.app/start/prerequisites/).

Cloner le dépôt, puis se placer à sa racine sur la branche à tester. Télécharger le [modèle Parakeet 1.3.0](https://github.com/Kieirra/murmure-model/releases/download/1.3.0/parakeet-tdt-0.6b-v3-int8.zip) et extraire l’archive dans `resources/`, pour obtenir le dossier `resources/parakeet-tdt-0.6b-v3-int8/`.

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

`pnpm tauri dev` lance l’application complète et son frontend. `pnpm dev` lance uniquement le frontend. `pnpm test` exécute les tests des commandes vocales et de Partage Murmure.

Sous Ubuntu, si une erreur `GLIBC_PRIVATE` mentionne `/snap/`, utiliser un terminal système extérieur à une application Snap.

## Configuration initiale

Ces trois fichiers sont à préparer **avant le build** :

| Fichier                                                                           | Rôle                                      |
| --------------------------------------------------------------------------------- | ----------------------------------------- |
| [default-settings.json](resources/hospital/default-settings.json)                 | Paramètres initiaux.                      |
| [default-dictionnary.json](resources/hospital/default-dictionnary.json)           | Mots et expressions du dictionnaire.      |
| [default-formatting-rules.json](resources/hospital/default-formatting-rules.json) | Commandes vocales et règles de formatage. |

Au lancement, chaque fichier utilisateur absent est initialisé depuis son modèle. **Les fichiers existants ne sont pas écrasés**, y compris après réinstallation.

Sous Windows, les fichiers actifs se trouvent dans `%APPDATA%\lan.ch-vauclaire.murmure\`. L’API locale est désactivée par défaut et se contrôle dans **Système → Général → Partage Murmure**. Les routes sont décrites dans la [documentation API](docs/features/api.fr.md).

## Construire l’installateur Windows

1. Intégrer et pousser sur `hospital/main` les changements et fichiers de configuration à embarquer.
2. Attendre que **CI Windows Vauclaire** soit verte pour le commit exact à construire. Le build vérifie cette validation sans relancer les tests.
3. Lancer manuellement [Build Windows Vauclaire - NSIS](https://github.com/dev-vauclaire/murmure-hospital/actions/workflows/build-hospital-windows-nsis.yml) sur cette même branche.
4. Après réussite, télécharger et extraire l’artefact `murmure-vauclaire-windows-x64-nsis-<commit>`, disponible **7 jours**.
5. Tester `Murmure-Vauclaire-NSIS-Setup.exe` sur une VM Windows avant diffusion.

La CI hospitalière s'exécute sur `hospital/main`. Après intégration des changements et réussite de cette CI, lancer le build depuis GitHub CLI :

```sh
gh workflow run build-hospital-windows-nsis.yml \
  --repo dev-vauclaire/murmure-hospital \
  --ref hospital/main
```

Le workflow produit un **setup unique pour Windows x64 et tous les utilisateurs du poste**. Il embarque l’application, le modèle, **Visual C++ x64 et WebView2**, installés si nécessaire sans téléchargement sur le poste. L’installation nécessite des droits administrateur ; les préférences restent propres à chaque utilisateur.

L’installateur est non signé. Le workflow ne crée pas de Release et ne déploie rien sur les postes.

## Documentation, contributions et crédits

- [Guide utilisateur HTML](resources/hospital/user-guide/index.html), également accessible depuis l’application.
- [API locale et route health](docs/features/api.fr.md).
- [Fonctionnement du pipeline Windows](packaging/windows-nsis/nsis-minimal.md).

Les retours et améliorations sont les bienvenus sur [ce dépôt](https://github.com/dev-vauclaire/murmure-hospital). Consulter [CONTRIBUTING.md](CONTRIBUTING.md) et [GUIDELINES.md](GUIDELINES.md) pour contribuer.

- **Murmure et cette adaptation** : [GNU AGPL v3](LICENSE). Merci aux auteurs et contributeurs du [projet officiel](https://github.com/Kieirra/murmure). [Soutenir Murmure](https://fr.tipeee.com/murmure-al1x-ai/).
- **Modèle** : [Parakeet de NVIDIA](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), [affinage Moondream](https://huggingface.co/moondream/parakeet-ultra) et [conversion ONNX int8 par @thiswillbeyourgithub](https://huggingface.co/Olicorne/parakeet-tdt-0.6b-v3-ultra-onnx), sous [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
- **Application** : [Tauri](https://tauri.app/), [licences tierces](licenses/) et [politique de confidentialité du projet d’origine](PRIVACY_POLICY.md).
- **Visuel SpeechMike** : [photo Philips Dictation](https://www.dictation.philips.com/fileadmin/Products/smp3700/img/smp3700-smp3800_philips-speechmike-premium-touch__f2_rgb.tif). Les conditions de redistribution du visuel constructeur restent à valider.
