# Validation du setup sur une VM Windows

Cette fiche est à exécuter sur Windows 11 x64 ; elle ne constitue pas un résultat
de test. Noter le commit indiqué dans `build-info.json` et le résultat de chaque
scénario. Préparer un instantané de VM et deux comptes de test, utilisés
successivement, sans laisser deux sessions ouvertes.

## 1. Installation neuve hors ligne

- [ ] Sur une VM sans Murmure, Visual C++ x64 ni WebView2, couper Internet puis
      lancer le setup : écrans en français, élévation administrateur et
      installation dans `C:\Program Files\Murmure Vauclaire`.
- [ ] L'application démarre sans erreur de DLL. Visual C++ x64 et WebView2 sont
      présents après installation, sans téléchargement. Si un redémarrage est
      demandé, l'effectuer avant de tester la dictée ; noter ce cas séparément.

La licence conserve son texte d'origine. Les dialogues Windows, dont l'UAC,
dépendent de la langue du système.

## 2. Entrées du registre

- [ ] Le marqueur ci-dessous est un `REG_DWORD` égal à `0x1`.
- [ ] La valeur `Run` est un `REG_SZ` contenant le chemin du bon exécutable
      **entre guillemets**, suivi de `--autostart`.

```powershell
reg.exe query "HKLM\SOFTWARE\ch-vauclaire\Murmure Vauclaire" /v isInstalled /reg:64
reg.exe query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" /v lan.ch-vauclaire.murmure /reg:64
```

## 3. Dictée et interface

- [ ] Choisir le microphone puis dicter une phrase fictive dans le Bloc-notes.
      Vérifier insertion, ponctuation, annulation et collage du dernier texte.
- [ ] Ajouter un mot de test au dictionnaire et une règle personnalisée, puis
      relancer Murmure : les deux sont conservés. Vérifier aussi l'ouverture du
      guide. Si un SpeechMike est disponible, essayer REC, F1 et F2.

## 4. Démarrage automatique personnel

- [ ] Après ouverture de session du premier compte, Murmure reste dans la barre
      système sans afficher sa fenêtre. La case **Lancer Murmure à l'ouverture
      de ma session**, dans **Système → Général**, est cochée par défaut.
- [ ] Décocher avec un compte standard : aucune demande administrateur. Fermer
      complètement la session puis la rouvrir : Murmure ne reste pas actif.
      Son raccourci permet toujours de l'ouvrir manuellement.
- [ ] Ouvrir la session du second compte : son démarrage reste activé. Dans le
      premier compte, recocher puis rouvrir la session réactive son démarrage.

Le lancement automatique effectue une vérification rapide même quand la case
est décochée, puis quitte avant de charger le moteur. Fermer la fenêtre masque
Murmure ; utiliser **Quitter** dans son menu de barre système pour l'arrêter.

## 5. Préférences et réinstallation

- [ ] Dans le premier compte, décocher le démarrage puis réinstaller : le choix
      reste désactivé. Si le dossier d'installation change, la valeur `Run`
      pointe vers le nouvel exécutable.
- [ ] Chaque compte conserve ses propres préférences. Un profil antérieur sans
      champ `start_on_login` utilise la valeur par défaut `true`.

Pour lire uniquement les réglages concernés dans le compte courant :

```powershell
Get-Content (Join-Path $env:APPDATA 'lan.ch-vauclaire.murmure\settings.json') -Raw |
    ConvertFrom-Json |
    Select-Object start_on_login, api_enabled, api_port
```

## 6. Partage Murmure

- [ ] Dans un profil neuf, **Partage Murmure** est désactivé et `api_enabled`
      vaut `false`. L'adresse de santé ne répond pas tant que le partage est arrêté.
- [ ] Activer **Partage Murmure** dans **Système → Général** : le serveur écoute
      sur `127.0.0.1:4800`, la case reste activée et `api_enabled` vaut `true`.
      Après fermeture complète et relance de Murmure, le partage reste activé.
- [ ] Désactiver : le serveur s'arrête, la case et `api_enabled` repassent à
      `false`. Après relance, le partage reste arrêté.

Le partage est piloté par cette option ; aucun marqueur d'installation de
DictaNote n'est nécessaire. `api_enabled` est le choix enregistré pour le prochain
lancement, pas un indicateur de disponibilité du moteur. Les commandes ci-dessous
utilisent le port par défaut `4800` ; les adapter si le profil en utilise un autre.

```powershell
curl.exe --noproxy "*" --max-time 3 -i http://127.0.0.1:4800/api/health
Get-NetTCPConnection -LocalPort 4800 -State Listen -ErrorAction SilentlyContinue |
    Select-Object LocalAddress, LocalPort, OwningProcess
```

## 7. Requêtes de transcription

- [ ] Partage activé, la santé renvoie `200` et
      `{"service":"murmure Vauclaire","status":"ready"}` une fois le modèle
      chargé. Un `503` avec `not_ready` est normal pendant le chargement ; si ce
      passage est trop rapide pour être observé, le noter comme non observé.
- [ ] Envoyer un petit fichier WAV de test : réponse `200` avec un champ `text`
      contenant la transcription attendue. La liste des prompts répond aussi
      en JSON ; les routes Prompt Mode existantes restent présentes.

Préparer `C:\Tests\dictee-test.wav` avec une phrase telle que « Ceci est un essai
de dictée. Le résultat attendu est un texte court. », puis exécuter :

```powershell
curl.exe --noproxy "*" --max-time 120 -i -F "audio=@C:\Tests\dictee-test.wav;type=audio/wav" http://127.0.0.1:4800/api/transcribe
curl.exe --noproxy "*" --max-time 3 -i http://127.0.0.1:4800/api/prompt-mode/prompts
```

Pour contrôler la présence des deux autres routes sans appeler de LLM :

```powershell
curl.exe --noproxy "*" --max-time 3 -i -F "test=synthetic" http://127.0.0.1:4800/api/prompt-mode
curl.exe --noproxy "*" --max-time 3 -i -F "test=synthetic" http://127.0.0.1:4800/api/prompt-mode/custom
```

Ces deux requêtes volontairement incomplètes doivent recevoir une erreur JSON
de validation, et non `404`.

## 8. Variantes des prérequis

- [ ] Sur des instantanés dédiés, rejouer les cas du tableau et vérifier le
      lancement puis une dictée après installation.
- [ ] Si Visual C++ renvoie réellement `3010`, vérifier la fin de l'installation
      et la présence du marqueur et de `Run` avant redémarrage. En mode
      silencieux/passif, le code de sortie attendu est `3010`. Ne pas considérer
      ce cas comme testé s'il n'a pas été observé sur la VM.

| État avant installation                                     | Résultat attendu                                            |
| ----------------------------------------------------------- | ----------------------------------------------------------- |
| Visual C++ x64 absent ou version trop ancienne              | Installation ou mise à jour depuis le setup, hors ligne     |
| Visual C++ x86 seul                                         | Installation du runtime **x64**                             |
| Visual C++ x64 égal ou plus récent que `prerequisites.json` | Étape d'installation Visual C++ ignorée ; version conservée |
| WebView2 absent                                             | Installation depuis le setup, hors ligne                    |
| WebView2 déjà présent et reconnu                            | Réutilisation du runtime existant                           |

## 9. Erreurs et bascules du partage

- [ ] Partage arrêté, réserver le port avec les commandes suivantes puis tenter
      de l'activer : un message français explique l'échec, la case n'annonce pas
      un partage actif et l'application permet toujours de dicter. Libérer le
      port puis recommencer : le partage fonctionne.
- [ ] Effectuer deux bascules rapides : pas de double serveur, pas de case
      bloquée. Après stabilisation, vérifier le choix enregistré et l'état réel
      du serveur ; une erreur doit être affichée comme telle.
- [ ] Activer le partage, quitter Murmure, réserver le port puis relancer :
      dialogue français et état indisponible, sans faux statut de serveur actif.
      `api_enabled` peut rester à `true` pour retenter au prochain lancement.
      Libérer le port puis relancer Murmure : le serveur démarre normalement.
- [ ] Pendant une requête de transcription de test, désactiver le partage :
      noter le résultat reçu par le client, puis vérifier qu'une nouvelle
      activation permet de retranscrire. L'arrêt natif peut interrompre la
      requête en cours ; il n'attend pas nécessairement sa fin.

Conserver la fenêtre PowerShell ouverte pendant le test du port occupé :

```powershell
$testListener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 4800)
$testListener.Start()
```

Essayer maintenant d'activer **Partage Murmure** dans l'application. Après avoir
vérifié le message d'erreur, libérer le port dans la même fenêtre PowerShell :

```powershell
$testListener.Stop()
```

## 10. Désinstallation

- [ ] Quitter Murmure puis désinstaller en conservant les préférences : le
      programme et ses raccourcis disparaissent, ainsi que `isInstalled` et notre
      valeur `Run`. Les autres entrées de démarrage restent intactes et Murmure
      ne se lance plus à l'ouverture de session.
- [ ] Visual C++ et WebView2 restent disponibles pour les autres applications.
      Rejouer aussi la désinstallation avec suppression des données utilisateur
      pour vérifier cette option séparément.

Réutiliser les commandes du scénario 2 : après désinstallation, nos deux valeurs
doivent être introuvables.
