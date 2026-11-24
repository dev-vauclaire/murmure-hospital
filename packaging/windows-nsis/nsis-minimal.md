# Premier jalon NSIS

La branche `refactor/windows-packaging` prépare un setup avec les mécanismes
standards de Tauri, sans template ni hook personnalisé. Le packaging MSI/Burn et
son workflow restent disponibles jusqu'à validation de ce jalon.

Le dossier provisoire `packaging/windows-nsis/` regroupe les fichiers de ce jalon.

## Contenu

- Application Windows x64 et ressources habituelles : modèle de transcription,
  configurations de référence, guide et licences.
- Installation pour tous les utilisateurs (`perMachine`), avec droits
  administrateur, et interface en français.
- WebView2 embarqué avec `offlineInstaller`, vérifié et installé par Tauri.
- Écrans, raccourcis et désinstallation standards du setup NSIS Tauri.
- Profils séparés dans `%APPDATA%\lan.ch-vauclaire.murmure`.

**Visual C++ x64 doit déjà être installé sur la VM.** Tauri ne gère pas ce
prérequis nativement. Son intégration, les options spécifiques de démarrage
automatique et la transition depuis Burn seront examinées après validation.

## Construire

Le workflow `Windows Vauclaire - NSIS minimal` est défini dans
`.github/workflows/build-hospital-windows-nsis.yml` et possède un seul job
`windows-nsis`. Ses étapes partagent la même machine Windows :

| Déclenchement                         | Contrôles et tests | Construction et publication du setup  |
| ------------------------------------- | ------------------ | ------------------------------------- |
| Push sur `refactor/windows-packaging` | Exécutés           | Ignorées                              |
| Pull request                          | Exécutés           | Ignorées                              |
| Lancement manuel                      | Exécutés           | Après réussite des contrôles et tests |

Après publication, le workflow peut être lancé manuellement dans GitHub Actions
en sélectionnant la branche `refactor/windows-packaging`. Le premier lancement
sur cette branche a été vérifié sans fusion dans `hospital/main`. Chaque
lancement manuel rejoue les tests sur le même commit ; aucun ancien résultat de
CI n'est réutilisé.

Les trois contrôles hospitaliers sont dans `scripts/`. La préparation des outils
est regroupée dans `setup-build/action.yml` et exécutée une seule fois par
lancement. Le modèle est également téléchargé et vérifié une seule fois, puis
réutilisé par les tests et, lors d'un lancement manuel, par la construction.
Les tests applicatifs restent dans les dossiers `tests/` et `src-tauri/` du dépôt.

La source du modèle est fixée dans `model-source.json` : version, URL et SHA-256
publié dans la [release officielle 1.3.0](https://github.com/Kieirra/murmure-model/releases/tag/1.3.0).
`scripts/prepare-model.py` vérifie l'archive avant décompression. Une empreinte
différente interrompt le job. Toute nouvelle version du modèle nécessite la
revue du manifeste. Les tests synthétiques de ces scripts sont dans `tests/`.

Après les tests, les étapes de construction, de préparation des métadonnées et
de publication de l'artefact s'exécutent uniquement lors d'un lancement manuel.
Une erreur dans les contrôles ou les tests empêche ces étapes. La commande de
construction produit uniquement NSIS :

```powershell
pnpm.cmd tauri build --target x86_64-pc-windows-msvc --bundles nsis --config packaging/windows-nsis/tauri.nsis.conf.json --ci --no-sign
```

L'artefact `murmure-vauclaire-windows-x64-nsis-<commit>` contient le setup non
signé, `build-info.json` et `SHA256SUMS.txt`. Il est conservé sept jours.

## Valider sur Windows

Utiliser une VM sans ancienne installation de Murmure Vauclaire, avec Visual C++
x64 déjà installé. Un instantané permet de rejouer le test.

1. Déconnecter Internet et lancer `Murmure-Vauclaire-NSIS-Setup.exe`.
2. Vérifier l'interface française, l'élévation administrateur et l'installation
   dans `C:\Program Files\Murmure Vauclaire`.
3. Vérifier le lancement, les raccourcis proposés par Tauri, le guide et une
   dictée avec des phrases fictives. Si WebView2 était absent, vérifier qu'il a
   été installé sans Internet.
4. Modifier une préférence puis relancer l'application pour vérifier sa
   conservation. Vérifier également dictionnaire et commandes vocales.
5. Ouvrir une session avec un second compte Windows : l'application doit être
   accessible et ses préférences indépendantes du premier compte.
6. Fermer l'application puis la désinstaller depuis les paramètres Windows.
   Vérifier le retrait du programme et des raccourcis. Ne pas cocher l'option
   native de suppression des données utilisateur si les préférences doivent
   être conservées.

L'exécution du workflow nécessite la publication de la branche sur GitHub ;
aucun commit, push ou build distant n'est déclenché par cette préparation locale.
