# Murmure Hospital

Adaptation de [Murmure](https://github.com/Kieirra/murmure) pour Vauclaire, dans le cadre de la reconnaissance vocale portée par l'intelligence artificielle.

L'objectif est d'effectuer les calculs de reconnaissance sur les postes utilisateurs, pour répartir la charge et traiter les dictées localement, sans API de transcription cloud.

## Capitalisation sur Murmure

Après étude de Handy et Murmure, nous avons retenu **Murmure pour son dictionnaire personnalisable et ses commandes vocales de mise en forme**.

Cette distribution repose sur la **version officielle 2.0.3**. Elle conserve le moteur Parakeet, l'insertion du texte dans les applications et l'architecture React/TypeScript + Rust/Tauri. Les adaptations restent limitées pour faciliter la maintenance et l'intégration des futures évolutions officielles.

La branche `main` conserve la référence officielle ; `hospital/main` regroupe les adaptations hospitalières. Les mises à jour du projet d'origine doivent être examinées et testées avant intégration.

## Changements effectués

- **Interface simplifiée** : accès à l'accueil, au dictionnaire, aux règles personnalisées et à une sélection de paramètres système. Les extensions, règles générales, réglages des raccourcis d'extensions et import/export global de configuration sont masqués.
- **Réglages conservés** : microphone, sons et volume, mode de dictée « appuyer pour parler » ou bascule, raccourcis de dictée, de collage de la dernière transcription et d'annulation, insertion Standard ou Direct, conversion des nombres avec langue et seuil.
- **Personnalisation visuelle** : ajout du logo Vauclaire à côté de celui de Murmure, avec une taille harmonisée.
- **Liens et mises à jour** : retrait des accès « Signaler un bug », « Notes de version » et « Vérifier les mises à jour » dans la sidebar. Le lien « Soutenir Murmure » est conservé et le composant de vérification des mises à jour n'est plus monté.
- **Configuration initiale commune** : paramètres, dictionnaire et commandes vocales embarqués lors du build, puis utilisés pour initialiser les fichiers utilisateur absents.
- **Confidentialité** : historique persistant, conservation des enregistrements audio, logs, API locale et Smart Speech Mic désactivés par défaut.
- **Build Windows manuel sur GitHub Actions**, avec choix d'un installateur EXE ou MSI et tests ciblés sur la configuration.
- **Boutons SpeechMike SMP3700 sous Windows** : détection automatique, REC pour la dictée, F1 pour annuler et F2 pour réinsérer la dernière transcription. Un diagnostic séparé reste disponible en ligne de commande.

Les restrictions de ce premier jalon portent sur l'interface : les fonctionnalités masquées et le plugin natif de mise à jour restent présents dans le backend. Il ne s'agit pas encore d'une politique de verrouillage administrée.

Dans **Système → Microphone et commandes**, les trois raccourcis clavier sont personnalisables tant que le profil clavier est actif. Cliquer sur une combinaison, saisir les nouvelles touches puis valider avec **Entrée** ou annuler avec **Échap**. Le changement est appliqué immédiatement et conservé dans les préférences utilisateur. L'éditeur natif propose aussi d'effacer un raccourci ou de rétablir sa valeur par défaut. Le modèle initial assigne uniquement **F9** à la dictée ; les douze autres raccourcis, dont collage et annulation, restent non assignés (`""` dans le JSON) jusqu'à une configuration explicite. Sous Linux/Wayland, les raccourcis restent gérés dans les paramètres du système avec les commandes natives affichées dans l'application.

Sous Windows, Murmure surveille les touches sans les intercepter. Vérifier les éventuels conflits avec les raccourcis des logiciels utilisés et personnaliser le raccourci si nécessaire.

### Fichiers de référence à préparer avant le build

| Fichier                                                                                                | Rôle                                                                                                     |
| ------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| [`resources/hospital/default-settings.json`](resources/hospital/default-settings.json)                 | Paramètres initiaux de l'application.                                                                    |
| [`resources/hospital/default-dictionnary.json`](resources/hospital/default-dictionnary.json)           | Dictionnaire initial, incluant les expressions des commandes vocales pour favoriser leur reconnaissance. |
| [`resources/hospital/default-formatting-rules.json`](resources/hospital/default-formatting-rules.json) | Règles initiales de ponctuation, retours à la ligne, paragraphes et guillemets.                          |

Ces fichiers utilisent les formats JSON natifs de Murmure. Chaque fichier utilisateur absent est initialisé indépendamment ; les fichiers existants ne sont pas écrasés par ce mécanisme.

Sous Windows, les fichiers actifs sont `settings.json`, `dictionary.json` et `formatting_rules.json`, dans `%APPDATA%\com.al1x-ai.murmure\`. Réinstaller l'application ne réinitialise pas un profil existant. Pour tester les valeurs initiales, quitter complètement Murmure puis renommer ce dossier pour en conserver une sauvegarde avant de relancer l'application.

### Mettre à jour uniquement les raccourcis d'un profil existant

1. Quitter complètement Murmure depuis son menu de barre système, puis sauvegarder son fichier `settings.json`.
2. Ouvrir ce fichier dans le dossier ci-dessus sous Windows, ou par défaut dans `~/.local/share/com.al1x-ai.murmure/` sous Linux.
3. Modifier uniquement les propriétés ci-dessous, en conservant toutes les autres propriétés et les fichiers de dictionnaire et de règles. Conserver les clés avec une chaîne vide : une clé supprimée reprendrait une valeur native de Murmure.

```json
{
    "record_shortcut": "f9",
    "last_transcript_shortcut": "",
    "command_shortcut": "",
    "llm_mode_1_shortcut": "",
    "llm_mode_2_shortcut": "",
    "llm_mode_3_shortcut": "",
    "llm_mode_4_shortcut": "",
    "llm_transform_1_shortcut": "",
    "llm_transform_2_shortcut": "",
    "llm_transform_3_shortcut": "",
    "llm_transform_4_shortcut": "",
    "voice_mode_toggle_shortcut": "",
    "cancel_shortcut": ""
}
```

4. Enregistrer puis relancer Murmure. Le raccourci de dictée pourra ensuite être personnalisé dans **Système** ; les autres combinaisons sont libérées.

## Boutons SpeechMike SMP3700 sous Windows

Le service détecte un SMP3700 au démarrage de Murmure ou lors de son branchement, y compris lorsque la fenêtre est masquée dans la barre système. Le micro doit être en **Event mode**. Dans **Système → Microphone et commandes**, les raccourcis clavier sont remplacés par le guide des boutons lorsque le SpeechMike est branché et constitue l'entrée audio sélectionnée. Le choix du microphone reste visible. Ce premier support est limité à un seul SpeechMike compatible branché à la fois ; la correspondance matérielle et les transitions restent à valider sur Windows et le matériel.

| Bouton physique          | Action dans Murmure                                                                                                             |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| REC                      | Maintien pour enregistrer, puis relâchement pour transcrire ; ou appui pour démarrer/arrêter selon le mode choisi dans Système. |
| F1                       | Annuler l'enregistrement en cours, sans insertion de sa transcription.                                                          |
| F2                       | Réinsérer la dernière transcription disponible.                                                                                 |
| Autres boutons de dictée | Aucune action attribuée dans Murmure pour ce jalon.                                                                             |

Le **microphone choisi dans Murmure est conservé** : les boutons ne changent pas l'entrée audio. L'identification rapproche l'entrée audio et l'interface HID par leur conteneur matériel Windows, sans utiliser leur nom et sans enregistrer d'identifiant propre à un poste dans le code. En mode Automatique, l'entrée audio par défaut effective est résolue. Une identité absente ou ambiguë ne suffit pas à activer le profil SpeechMike.

Les profils de commandes sont exclusifs : **SpeechMike sélectionné**, les trois raccourcis clavier sont ignorés côté Rust ; **autre microphone sélectionné**, les boutons du SpeechMike sont mis en veille et les raccourcis clavier sont réactivés. Les combinaisons enregistrées dans les préférences ne sont pas effacées. Une bascule demandée pendant une dictée ou son traitement attend la fin de cette session ; les touches doivent être relâchées avant un nouveau déclenchement.

Le guide illustré est embarqué depuis `src/assets/speechmike/smp3700-guide.png`. Ce visuel peut être remplacé avant un build (également `.jpg`, `.jpeg`, `.webp` ou `.svg`). Sans image, le guide affiche les trois actions sous forme de légende. La consigne REC suit le mode d'enregistrement choisi.

Le branchement ou la reconnexion ne démarre aucun enregistrement. Murmure attend un état initial connu avec les boutons relâchés avant d'afficher « prêt ». Si le périphérique ne transmet pas cet état spontanément, suivre l'indication dans Système : **appuyer puis relâcher REC une première fois pour initialiser les boutons**, sans dicter ; le cycle suivant commande la dictée. Un bouton déjà tenu lors de la connexion ne déclenche aucune action. Pendant la capture d'un nouveau raccourci clavier ou la suspension de la dictée, les boutons sont désactivés et un enregistrement SpeechMike en cours est annulé.

Une déconnexion pendant l'enregistrement annule uniquement la session déclenchée par le SpeechMike, sans insérer de texte incomplet. Après une reconnexion, un nouvel appui est nécessaire. Si l'arrêt normal a déjà été demandé, la transcription peut se terminer. Les LED du micro ne sont pas pilotées ; l'overlay de Murmure indique l'état d'enregistrement.

## Diagnostic SpeechMike SMP3700 sous Windows

Ce premier jalon observe les boutons sans déclencher de dictée. La commande s'exécute avant Tauri et le moteur audio, sans lire les préférences utilisateur ni créer de logs sur disque. Elle ne modifie pas le mode du SpeechMike et ne commande pas ses LED.

1. Quitter complètement Murmure et les autres applications susceptibles d'utiliser les boutons du micro, notamment SpeechControl pendant ce test.
2. Brancher le SMP3700 et le placer en **Event mode** selon le [manuel Philips](https://www.dictation.philips.com/fileadmin/Products/smp3700/ifu/en/button-modes.html). Dans une VM, rediriger le périphérique USB complet : la redirection du son seule ne transmet pas les boutons HID.
3. Après installation de cette version, lancer depuis PowerShell, en adaptant le chemin si nécessaire :

```powershell
Start-Process -FilePath "$env:LOCALAPPDATA\murmure\murmure.exe" -ArgumentList "--speechmike-diagnostic" -NoNewWindow -Wait
```

Le diagnostic dure environ **60 secondes**. Appuyer puis relâcher REC et les autres boutons, puis débrancher et rebrancher le micro. Le terminal affiche les interfaces candidates, l'identification du modèle et du mode, l'état initial des boutons puis leurs changements. Les numéros de série, chemins matériels et rapports bruts ne sont pas affichés. **Ctrl+C** permet d'interrompre le test. Le diagnostic observe les boutons sans les raccorder aux actions de dictée du service normal.

Le filtre USB et le décodage proviennent du [SDK Google de référence](https://github.com/GoogleChromeLabs/dictation_support/tree/077292a27ba77e40468fb464889fe3b5c121737d) ; ils restent à valider sur le micro et son firmware. Le diagnostic accepte uniquement un SMP3700 identifié en Event mode et signale les configurations non reconnues. En présence de plusieurs interfaces candidates, ne laisser qu'un seul SpeechMike branché pour ce premier jalon.

En développement sous Windows, la même commande peut être lancée depuis `src-tauri/` avec `cargo run -- --speechmike-diagnostic`. Sous Linux et macOS, elle renvoie un message indiquant que ce diagnostic est réservé à Windows.

La dépendance `hidapi` est limitée à Windows et utilise son backend natif. Les [mentions du protocole adapté](licenses/dictation-support-NOTICE.txt) et la [licence Apache-2.0](licenses/dictation-support-APACHE-2.0.txt) sont embarquées avec les ressources de l'application.

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

Il vérifie les trois JSON de référence suivis dans Git, prépare les outils et dépendances, télécharge le modèle Parakeet, construit les ressources SmartMic et exécute les tests Rust ciblés des paramètres, du dictionnaire, du formatage, de l'initialisation et du protocole SpeechMike. Les tests du protocole utilisent uniquement des messages synthétiques et ne nécessitent pas de micro connecté.

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
- Guide illustré du SMP3700 : adaptation d'une [photo produit publiée par Philips Dictation](https://www.dictation.philips.com/fileadmin/Products/smp3700/img/smp3700-smp3800_philips-speechmike-premium-touch__f2_rgb.tif). Cette mention de provenance ne vaut pas autorisation de redistribution ; les conditions de réutilisation du visuel constructeur restent à valider.
