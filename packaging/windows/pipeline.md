# Comprendre le build Windows hospitalier

Le workflow `Build Windows hospitalier` fabrique un seul installateur :
`Murmure-Hospital-Setup.exe`, pour Windows x64 et tous les utilisateurs du poste.
Le MSI de Murmure, Visual C++ x64 et WebView2 sont contenus dans ce setup.

## Pendant le build, sur GitHub

1. Le lancement manuel choisit la branche à compiler, sans choix de format.
2. La CI vérifie les versions, les fichiers hospitaliers et les scripts.
3. Elle prépare Node.js, pnpm, Rust et NASM, puis les dépendances du projet.
4. Elle prépare le template MSI vérifié, télécharge le modèle vocal, construit SmartMic et exécute les tests ciblés.
5. Tauri compile Murmure et fabrique un MSI avec WebView2 embarqué en mode `offlineInstaller`.
6. WiX Burn assemble le MSI et Visual C++ dans le setup.
7. Les contrôles inspectent le MSI et extraient le setup pour vérifier son contenu.
8. GitHub conserve le setup, ses empreintes et ses métadonnées pendant 7 jours.

Cette phase a besoin d’Internet. Les installateurs des prérequis sont téléchargés
pendant le build et vérifiés avant d’être incorporés. La CI ne les installe pas.

## Pendant l’installation, sur le poste Windows

Burn vérifie et installe Visual C++ si nécessaire, puis lance le MSI.
Le MSI utilise la gestion native de Tauri pour installer WebView2 si absent,
puis installer Murmure. Ces fichiers sont déjà embarqués : aucun téléchargement
n’est nécessaire pour installer les prérequis. Une élévation administrateur est
nécessaire. Les préférences restent dans le profil de chaque utilisateur.
Un prérequis en échec bloque l’installation de l’application. Désinstaller Murmure
conserve les runtimes partagés. Le MSI interne ne doit pas être lancé séparément.

## Les deux choix proposés dans le setup

| Option                                          | Premier lancement du setup | Effet                                                                              |
| ----------------------------------------------- | -------------------------- | ---------------------------------------------------------------------------------- |
| Créer un raccourci sur le Bureau                | Cochée                     | Ajoute le raccourci au bureau public, accessible à tous les comptes.               |
| Lancer Murmure à l'ouverture de session Windows | Décochée                   | Ajoute un démarrage à la connexion pour tous les comptes, avec la fenêtre masquée. |

Les cases apparaissent avant le bouton Installer. Les choix sont enregistrés dans
`HKLM\Software\Vauclaire\Murmure\Installer` (vue 64 bits), puis relus par le setup
suivant. Une mise à jour conserve donc les choix, qui restent modifiables avant
l'installation. Une réparation depuis le setup reprend les valeurs enregistrées.

Le MSI crée ou retire les composants correspondants. Le démarrage automatique
utilise la valeur `MurmureHospital` dans
`HKLM\Software\Microsoft\Windows\CurrentVersion\Run`, avec les arguments
`--autostart --hidden`. La désinstallation supprime cette valeur et les choix
d'installation. Elle ne modifie pas un éventuel démarrage personnel préexistant
dans HKCU. Murmure n'est pas lancé à la fin de l'installation sous le compte
administrateur.

## Rôle des fichiers

| Fichier                                                                        | Responsabilité                                                                      |
| ------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------- |
| `.github/workflows/build-hospital-windows.yml`                                 | Ordonne les étapes sur GitHub.                                                      |
| `packaging/windows/tauri.setup.conf.json`                                      | Confie WebView2 au MSI Tauri en mode hors ligne.                                    |
| `packaging/windows/msi-template-source.json` et `prepare-installer-options.py` | Vérifient le template officiel Tauri puis ajoutent les composants des deux options. |
| `packaging/windows/bootstrapper-dependencies.json`                             | Fige les URL, versions et empreintes de WiX et Visual C++.                          |
| `packaging/windows/build-bootstrapper.ps1`                                     | Télécharge, vérifie et assemble le setup.                                           |
| `packaging/windows/bootstrapper.wxs`                                           | Décrit la détection et l’ordre d’installation des composants.                       |
| `packaging/windows/bootstrapper-theme.xml` et `.fr-fr.wxl`                     | Présentent les deux cases et les textes du setup en français.                       |
| `packaging/windows/test-bootstrapper.py` et `.ps1`                             | Vérifient les contrats du packaging et les fichiers produits.                       |
| `packaging/windows/test-installer-options.py`                                  | Vérifie les options MSI et le refus d'un template modifié.                          |

Le template généré reste dans `src-tauri/target/hospital-installer/main.wxs`, sans
copier le template complet dans Git. Le script exige la version Tauri CLI et
l'empreinte déclarées dans le manifeste : une mise à jour de Tauri nécessite leur
révision explicite. Pour un build manuel, exécuter après `pnpm install` :

```sh
python packaging/windows/prepare-installer-options.py
```

Les notices du thème WiX adapté et du template Tauri sont conservées dans
`licenses/wix3-MS-RL.txt` et `licenses/tauri-installer-MIT.txt`.

`build-info.json` identifie notamment le commit, la version et les composants.
`SHA256SUMS.txt` permet de vérifier le setup téléchargé. L’installateur final reste
non signé. Le workflow officiel `build-windows.yml` reste indépendant.

## Validation sur une VM

La réussite de la CI ne prouve pas qu’une installation fonctionne. Sur une VM
Windows 11, couper Internet et tester les prérequis absents, présents et plus
récents, le refus d’élévation, une mise à jour et la désinstallation. Vérifier la
dictée depuis deux comptes standards et l’indépendance de leurs préférences.

Pour les options, repartir du même instantané vierge pour chacune des quatre
combinaisons des deux cases. Contrôler le bureau des deux comptes, puis leur
déconnexion/reconnexion : Murmure doit démarrer masqué uniquement si l'option est
cochée. Vérifier aussi le lancement manuel par le menu Démarrer, toujours présent.

Installer ensuite un setup plus récent : les cases doivent reprendre les choix
précédents. Tester une mise à jour sans les changer, puis une autre en les
inversant, notamment le retrait du raccourci existant. Tester Réparer et
Désinstaller depuis le setup, puis vérifier le retrait du raccourci public et de
la valeur Run. Contrôler le rendu des cases à 100 %, 125 % et 150 % de mise à
l'échelle. Les vérifications automatisées inspectent le MSI sans l'installer ;
elles ne remplacent pas ces essais Windows.

WebView2 est téléchargé et mis en cache par Tauri pendant le build. Sa version
n’est plus figée dans notre manifeste ; les métadonnées indiquent cette gestion
native. Tester aussi un runtime WebView2 présent uniquement dans un profil
utilisateur : la détection native de Tauri consulte également HKCU.
