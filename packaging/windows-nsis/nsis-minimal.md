# Installateur NSIS Windows

La branche `refactor/windows-packaging` prépare un setup généré par Tauri, avec
des hooks pour Visual C++ x64 et la détection de l'installation. Le packaging MSI/Burn et son workflow
restent disponibles pendant la validation de NSIS.

Le dossier provisoire `packaging/windows-nsis/` regroupe les fichiers de ce jalon.

## Contenu

- Application Windows x64 et ressources habituelles : modèle de transcription,
  configurations de référence, guide et licences.
- Installation pour tous les utilisateurs (`perMachine`), avec droits
  administrateur, et interface en français.
- WebView2 embarqué avec `offlineInstaller`, vérifié et installé par Tauri.
- Visual C++ x64 embarqué, installé seulement si le runtime est absent ou plus
  ancien que la version fixée dans `prerequisites.json`.
- Écrans, raccourcis et désinstallation standards du setup NSIS Tauri.
- Profils séparés dans `%APPDATA%\lan.ch-vauclaire.murmure`.
- Démarrage à l'ouverture de session activé par défaut pour tous les comptes,
  désactivable individuellement dans Murmure, sans droits administrateur.

Visual C++ est un runtime partagé : sa désinstallation n'est jamais déclenchée
par Murmure. La transition depuis Burn reste un jalon distinct. Les dossiers des profils restent ceux
de Tauri 2.11.1 ; `appDirectoriesOverride` est reporté.

## Hooks et détection de l'installation

`hooks.nsh` reste le point d'entrée déclaré dans la configuration Tauri. Il charge :

- `hooks/prerequisites.nsh` : vérification et installation de Visual C++ x64,
  puis gestion d'un éventuel redémarrage.
- `hooks/installation-marker.nsh` : écriture et suppression du marqueur
  `isInstalled` de type `REG_DWORD`, avec la valeur `1` après installation.
- `hooks/startup.nsh` : inscription du démarrage commun à toutes les sessions,
  mise à jour de son chemin à la réinstallation et retrait à la désinstallation.

Le marqueur utilise `${MANUPRODUCTKEY}`, construit par Tauri à partir de
`publisher` et `productName`. Avec l'identité actuelle, il se trouve dans la vue
64 bits de `HKLM\SOFTWARE\ch-vauclaire\Murmure Vauclaire`.
Le hook de fin d'installation l'écrit avant la gestion du code `3010`.
Le hook `POSTUNINSTALL` le supprime même lorsque les préférences sont conservées.
Il signale la présence de l'installation ; la disponibilité de la transcription
se vérifie séparément avec l'API de santé.

## Démarrage automatique et choix personnel

Le setup écrit une valeur `REG_SZ` nommée `lan.ch-vauclaire.murmure` dans
`HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run`, en vue 64 bits. Elle
contient le chemin de l'exécutable entre guillemets, suivi de `--autostart`.
Cette inscription est faite avant la gestion du code `3010` et couvre aussi
les comptes qui n'ont jamais ouvert Murmure.

Dans **Système → Général**, la case **Lancer Murmure à l'ouverture de ma session**
agit uniquement sur `start_on_login` dans le `settings.json` du compte courant.
La valeur par défaut est `true`, y compris pour un profil antérieur ne contenant
pas encore ce champ. Modifier la case ne modifie pas HKLM et ne demande aucune
élévation administrateur.

Au lancement `--autostart`, Murmure lit cette préférence avant de charger le
modèle. Si elle vaut `false`, il quitte immédiatement ; sinon il reste dans la
barre système, sans afficher sa fenêtre. La vérification lance donc brièvement
le processus même lorsque le démarrage personnel est désactivé. Le lancement
manuel, `--hidden` et les commandes CLI explicites restent utilisables.

Le choix prend effet à la prochaine ouverture de session et ne ferme pas une
instance déjà active. Une réinstallation conserve les préférences existantes.
La désinstallation retire uniquement notre valeur de la clé `Run` ; les entrées
des autres applications sont conservées. L'ancien mécanisme Tauri de démarrage
par utilisateur n'est plus réactivé sous Windows.

## Construire

Deux workflows hospitaliers séparent les tests de la construction :

| Fichier                                             | Déclenchement                                                           | Rôle                                 |
| --------------------------------------------------- | ----------------------------------------------------------------------- | ------------------------------------ |
| `.github/workflows/ci-hospital-windows.yml`         | Push sur `hospital/main` et temporairement `refactor/windows-packaging` | Contrôles et tests Windows           |
| `.github/workflows/build-hospital-windows-nsis.yml` | Manuel uniquement                                                       | Construction et publication du setup |

La CI (`CI Windows Vauclaire`) vérifie les versions, l'identité et les fichiers
de référence, puis lance les tests Python, PowerShell, frontend, Rust et NSIS.
Elle prépare son propre compilateur NSIS via `scripts/prepare-nsis.ps1`, à partir
de la version officielle fixée dans `nsis-source.json`, sans construire un setup
Murmure ni télécharger le véritable redistribuable Visual C++.

Pour créer le setup, attendre que cette CI soit verte pour le commit choisi,
puis lancer `Build Windows Vauclaire - NSIS` sur la même branche. Le script
`scripts/check-ci.py` consulte l'API GitHub Actions en lecture seule et vérifie le
SHA exact du commit extrait. Si la CI est absente, en cours ou échouée, le build
s'arrête avant l'installation des outils. Aucun test n'est relancé et aucune CI
n'est déclenchée automatiquement par le build. Le résultat CI accepté est
enregistré dans `build-info.json`.

Après validation et fusion, retirer `refactor/windows-packaging` du filtre de la
CI pour ne conserver que `hospital/main`. La CI amont sur pull request et le
packaging MSI/Burn restent disponibles pendant cette transition.

Les trois contrôles hospitaliers sont dans `scripts/`. La préparation des outils
est regroupée dans `setup-build/action.yml`, utilisée une fois dans chacune des
deux exécutions indépendantes. Chaque machine prépare son modèle ; les caches
Rust accélèrent les compilations. Les tests applicatifs restent dans les dossiers
`tests/` et `src-tauri/` du dépôt.

La source du modèle est fixée dans `model-source.json` : version, URL et SHA-256
publié dans la [release officielle 1.3.0](https://github.com/Kieirra/murmure-model/releases/tag/1.3.0).
`scripts/prepare-model.py` vérifie l'archive avant décompression. Une empreinte
différente interrompt le job. Toute nouvelle version du modèle nécessite la
revue du manifeste. Les tests synthétiques de ces scripts sont dans `tests/`.

Le build manuel conserve les contrôles des fichiers réellement téléchargés et
du paquet produit : empreinte du modèle, signature et version de Visual C++, puis
présence et empreinte du prérequis embarqué. Ce sont les vérifications du livrable
de cette exécution, indépendantes des tests applicatifs déjà réussis en CI.

`scripts/prepare-prerequisites.ps1` télécharge la version Microsoft fixée dans
`prerequisites.json`, puis vérifie son SHA-256, sa signature Authenticode Microsoft
et sa version. Il prépare le redistribuable et un include NSIS dans
`src-tauri/target/nsis-prerequisites/`, sans les installer ni les suivre par Git.
Seul le lancement manuel télécharge ce fichier réel. Les tests du script utilisent
des données synthétiques, sans réseau et sans installation de runtime.

Le setup embarque directement le redistribuable via `hooks/prerequisites.nsh`. Avant de copier
l'application, le hook consulte `Installed` et `Version` dans les vues 32 et
64 bits de la clé Microsoft des runtimes x64. Il conserve une version suffisante
ou plus récente. Sinon, il extrait temporairement `VC_redist.x64.exe` et l'exécute
avec `/install /quiet /norestart`. WebView2 reste pris en charge par Tauri, dans
sa section précédant ce hook.

Le code `0` permet de continuer. Le code `3010` indique un succès nécessitant un
redémarrage : l'installation termine, avec redémarrage différé par défaut et sans
lancement de Murmure avant celui-ci. En mode silencieux/passif, le setup renvoie
`3010`, même si `/R` était demandé. Les autres erreurs arrêtent l'installation de
l'application. Les détails sont visibles dans la progression de NSIS.

La CI teste les fonctions NSIS avec des valeurs synthétiques. Après construction,
le build extrait seulement le redistribuable du setup pour comparer son empreinte
au manifeste. `build-info.json` enregistre sa version, son URL et son SHA-256.

Pour construire localement sous Windows avec PowerShell 7, préparer le prérequis puis,
si cette étape réussit, lancer Tauri :

```powershell
& ./packaging/windows-nsis/scripts/prepare-prerequisites.ps1
pnpm.cmd tauri build --target x86_64-pc-windows-msvc --bundles nsis --config packaging/windows-nsis/tauri.nsis.conf.json --ci --no-sign
```

L'artefact `murmure-vauclaire-windows-x64-nsis-<commit>` contient le setup non
signé, `build-info.json` et `SHA256SUMS.txt`. Il est conservé sept jours.

## Valider sur Windows

La [fiche de validation VM](validation-vm.md) regroupe les scénarios
d'installation, de démarrage automatique, de partage et de désinstallation.

Utiliser une VM Windows x64 sans ancienne installation de Murmure Vauclaire.
Un instantané permet de rejouer le test.

1. Déconnecter Internet et lancer `Murmure-Vauclaire-NSIS-Setup.exe`.
2. Vérifier l'interface française, l'élévation administrateur et l'installation
   dans `C:\Program Files\Murmure Vauclaire`. Sans Visual C++ x64 préalable, le
   prérequis doit être installé depuis le setup, sans téléchargement.
3. Vérifier le lancement, les raccourcis proposés par Tauri, le guide et une
   dictée avec des phrases fictives. Si WebView2 était absent, vérifier qu'il a
   été installé sans Internet.
4. Modifier une préférence puis relancer l'application pour vérifier sa
   conservation. Vérifier également dictionnaire et commandes vocales.
5. Ouvrir une session avec un second compte Windows : l'application doit être
   accessible et ses préférences indépendantes du premier compte.
6. Fermer l'application puis la désinstaller depuis les paramètres Windows.
   Vérifier le retrait du programme et des raccourcis, et la conservation de
   Visual C++. Ne pas cocher l'option native de suppression des données
   utilisateur si les préférences doivent être conservées.

Contrôler le marqueur après l'installation, puis après la désinstallation :

```powershell
reg.exe query "HKLM\SOFTWARE\ch-vauclaire\Murmure Vauclaire" /v isInstalled /reg:64
```

Après installation, le résultat doit afficher `isInstalled REG_DWORD 0x1`.
Après désinstallation, la valeur doit être introuvable, y compris si les
préférences sont conservées. Rejouer avec suppression des préférences et après
réinstallation : le marqueur doit respectivement être absent puis valoir `1`.
Si l'installation demande un redémarrage, vérifier que le marqueur est déjà
présent, y compris en mode silencieux/passif retournant `3010`.

Pour valider le démarrage automatique, utiliser deux comptes successivement
(sans sessions simultanées) :

1. Après une installation neuve, ouvrir une session : Murmure doit être actif
   dans la barre système, avec sa fenêtre masquée et sa case de démarrage cochée.
2. Décocher la case avec le premier compte standard : aucun administrateur ne
   doit être demandé. Fermer sa session puis la rouvrir : Murmure ne doit plus
   rester actif. Le raccourci du menu Démarrer doit toujours permettre de l'ouvrir.
3. Avec le second compte, le démarrage doit rester activé. Recocher dans le premier
   compte doit réactiver son démarrage à la prochaine ouverture de session.
4. Réinstaller après avoir décoché la case : le choix personnel doit rester
   désactivé et l'entrée commune doit pointer vers le dossier d'installation actuel.
5. Désinstaller puis vérifier que notre valeur `Run` a disparu, même si les
   préférences sont conservées. Les autres valeurs de `Run` doivent rester intactes.

```powershell
reg.exe query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" /v lan.ch-vauclaire.murmure /reg:64
```

Rejouer également l'installation avec un runtime x64 de version égale ou plus
récente : l'étape Visual C++ doit être ignorée et sa version conservée. Avec une
version plus ancienne, elle doit être mise à jour. Un runtime x86 seul ne suffit
pas. Ces cas se testent sur des instantanés dédiés, sans supprimer de DLL système.
Les retours `3010` et d'erreur sont simulés par les tests NSIS en CI ; sur VM,
vérifier le comportement réel si un redémarrage est demandé.

## Contrôler le runtime et les DLL sur la VM

Dans les paramètres Windows : **Applications → Applications installées**, puis
chercher **Microsoft Visual C++** et vérifier l'entrée **x64**.

Depuis PowerShell 64 bits, les clés Microsoft se consultent dans les deux vues :

```powershell
reg.exe query "HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" /reg:64
reg.exe query "HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" /reg:32
```

`Installed` doit valoir `0x1`, avec une `Version` suffisante dans au moins une vue.
L'absence de la clé dans une seule vue n'indique pas une absence du runtime.

Sous Windows x64, `C:\Windows\System32` contient les DLL x64 ; `SysWOW64` contient
les DLL x86. Utiliser PowerShell 64 bits pour éviter la redirection vers x86 :

```powershell
[Environment]::Is64BitProcess
Get-Item "$env:windir\System32\msvcp140.dll",
         "$env:windir\System32\vcruntime140.dll",
         "$env:windir\System32\vcruntime140_1.dll" |
    Select-Object Name, FullName,
        @{Name='Version'; Expression={$_.VersionInfo.FileVersion}}
```

La première commande doit afficher `True`. Dans l'Explorateur, **Propriétés →
Détails** de la DLL montre également sa version. Après ouverture de Murmure et
une dictée fictive, on peut contrôler les bibliothèques effectivement chargées :

```powershell
Get-Process -Name 'murmure-vauclaire' -Module |
    Where-Object {
        $_.ModuleName -in @('msvcp140.dll', 'vcruntime140.dll', 'vcruntime140_1.dll')
    } |
    Select-Object ModuleName, FileName
```

Si l'accès est refusé, ouvrir PowerShell en administrateur. La présence d'une DLL
seule ne remplace pas le contrôle de la version du runtime et le test de dictée.

L'exécution du workflow nécessite la publication de la branche sur GitHub ;
aucun commit, push ou build distant n'est déclenché par cette préparation locale.
