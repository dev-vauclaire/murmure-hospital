# Murmure Vauclaire

## Modifications par rapport au dépôt d’origine

Murmure Vauclaire est une adaptation de [Murmure](https://github.com/Kieirra/murmure), initialement basée sur la version officielle **2.0.3**. La reconnaissance vocale reste locale, avec le moteur **Parakeet**.

- Interface simplifiée : accueil, dictionnaire, commandes vocales et réglages essentiels.
- Identité visuelle associant les logos Murmure et Vauclaire.
- Paramètres, dictionnaire et commandes vocales préconfigurés.
- Raccourcis personnalisables et support du **Philips SpeechMike SMP3700 sous Windows**.
- Identité d’application distincte et retrait des mises à jour automatiques vers le projet officiel.
- API locale activable avec **Partage Murmure** et ajout de la route `GET /api/health`.
- Sous Windows, démarrage à l’ouverture de session activé par défaut et personnalisable pour chaque utilisateur.

La simplification de l’interface ne constitue pas un verrouillage des fichiers de préférences.

## Fichiers de configuration

Préparer ces trois fichiers dans **`resources/hospital/` avant le build** :

| Fichier à embarquer                                                               | Contenu                        | Fichier utilisateur créé |
| --------------------------------------------------------------------------------- | ------------------------------ | ------------------------ |
| [default-settings.json](resources/hospital/default-settings.json)                 | Paramètres initiaux            | `settings.json`          |
| [default-dictionnary.json](resources/hospital/default-dictionnary.json)           | Dictionnaire                   | `dictionary.json`        |
| [default-formatting-rules.json](resources/hospital/default-formatting-rules.json) | Commandes vocales et formatage | `formatting_rules.json`  |

Au lancement, chaque fichier utilisateur absent est initialisé depuis son modèle dans **`%APPDATA%\lan.ch-vauclaire.murmure\`** sous Windows. Les fichiers existants sont conservés, y compris après réinstallation : modifier les modèles ne met donc pas à jour les profils existants.

## Construire l’installateur Windows 64 bits

1. Intégrer et pousser sur `hospital/main` les changements et fichiers de configuration à embarquer.
2. Attendre que **CI Windows Vauclaire** soit verte pour ce même commit.
3. Dans **Actions → [Build Windows Vauclaire - NSIS](https://github.com/dev-vauclaire/murmure-hospital/actions/workflows/build-hospital-windows-nsis.yml) → Run workflow**, sélectionner `hospital/main` puis lancer le build. Il vérifie la réussite de la CI sans relancer les tests.
4. Après réussite, ouvrir l’exécution et télécharger l’artefact `murmure-vauclaire-windows-x64-nsis-<commit>`, disponible **7 jours**, puis extraire son archive.
5. Tester `Murmure-Vauclaire-NSIS-Setup.exe` sur une VM Windows avant diffusion.

Le résultat est un **installateur NSIS `.exe` pour Windows x64**, qui installe l’application pour tous les utilisateurs du poste avec des droits administrateur. Le modèle, **Visual C++ x64 et WebView2 sont embarqués** ; les prérequis sont installés si nécessaire sans téléchargement sur le poste. Les préférences restent propres à chaque utilisateur. L’installateur est non signé.

## Documentation et crédits

- [Guide utilisateur HTML](resources/hospital/user-guide/index.html), également accessible depuis l’application.
- [API locale et route health](docs/features/api.fr.md).
- [Fonctionnement du pipeline Windows](packaging/windows-nsis/nsis-minimal.md).

Les retours et améliorations sont les bienvenus sur [ce dépôt](https://github.com/dev-vauclaire/murmure-hospital). Consulter [CONTRIBUTING.md](CONTRIBUTING.md) et [GUIDELINES.md](GUIDELINES.md) pour contribuer.

- **Murmure et cette adaptation** : [GNU AGPL v3](LICENSE). Merci aux auteurs et contributeurs du [projet officiel](https://github.com/Kieirra/murmure). [Soutenir Murmure](https://fr.tipeee.com/murmure-al1x-ai/).
- **Modèle** : [Parakeet de NVIDIA](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), [affinage Moondream](https://huggingface.co/moondream/parakeet-ultra) et [conversion ONNX int8 par @thiswillbeyourgithub](https://huggingface.co/Olicorne/parakeet-tdt-0.6b-v3-ultra-onnx), sous [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
- **Application** : [Tauri](https://tauri.app/), [licences tierces](licenses/) et [politique de confidentialité du projet d’origine](PRIVACY_POLICY.md).
- **Visuel SpeechMike** : [photo Philips Dictation](https://www.dictation.philips.com/fileadmin/Products/smp3700/img/smp3700-smp3800_philips-speechmike-premium-touch__f2_rgb.tif). Les conditions de redistribution du visuel constructeur restent à valider.
