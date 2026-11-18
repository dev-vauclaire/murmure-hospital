# Validation Windows de Murmure Vauclaire

Cette fiche sert à vérifier un setup sur une VM Windows 11 x64 neuve, puis sur
un poste représentatif du pilote. Les cases sont à remplir après les essais :
un build réussi ne valide pas l'installation ni la dictée.

## Identifier le package testé

| Élément                                  | Valeur à renseigner |
| ---------------------------------------- | ------------------- |
| Date et testeur                          |                     |
| Version de Murmure Vauclaire             |                     |
| Commit et lien du build GitHub Actions   |                     |
| SHA-256 de `Murmure-Vauclaire-Setup.exe` |                     |
| Version et architecture de Windows       |                     |
| Microphone et applications testés        |                     |
| Résultat final et anomalies restantes    |                     |

Comparer l'empreinte du setup avec `SHA256SUMS.txt` fourni dans le même artefact.
Créer un instantané de la VM avant installation et deux comptes standards
fictifs, par exemple `TestA` et `TestB`. Utiliser uniquement des textes et des
enregistrements de démonstration.

## Installation et identité

Pour chaque ligne, noter **OK**, **KO** ou **Non testé**, avec une observation.
Une VM neuve peut déjà contenir WebView2 ou Visual C++ : vérifier leur présence
avant de conclure qu'un essai couvre leur absence.

| Résultat | Test                    | Action et résultat attendu                                                                                                                                                                                                                 |
| -------- | ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
|          | Installation hors ligne | Couper le réseau, lancer le setup depuis un compte standard et fournir l'élévation demandée. L'installation doit aboutir avec les prérequis embarqués, sans téléchargement nécessaire.                                                     |
|          | Prérequis absents       | Sur un instantané où Visual C++ x64 et/ou WebView2 manquent, installer puis lancer l'application. Aucune erreur de DLL ou de WebView2 ne doit empêcher la dictée. Marquer séparément les prérequis dont l'absence a réellement été testée. |
|          | Prérequis présents      | Avec Visual C++ x64 de version suffisante et WebView2 déjà présents, installer et lancer l'application. Vérifier dans le journal du setup que Visual C++ a été détecté et que son installateur n'a pas été exécuté.                        |
|          | Visual C++ plus récent  | Refaire l'essai avec un runtime x64 plus récent que celui embarqué, si disponible. Il doit être conservé. Noter toute demande de redémarrage ; si elle survient, redémarrer puis refaire une dictée.                                       |
|          | Refus d'élévation       | Repartir d'un instantané vierge, refuser l'élévation et vérifier qu'aucune installation fonctionnelle n'a été ajoutée. Un journal ou un cache du setup peut néanmoins exister.                                                             |
|          | Identité affichée       | Vérifier « Murmure Vauclaire » dans le setup, la fenêtre, le menu Démarrer et les applications installées ; l'éditeur doit être « Vauclaire ».                                                                                             |
|          | Fichiers installés      | Au chemin par défaut, vérifier `C:\Program Files\Murmure Vauclaire\murmure-vauclaire.exe`. Le programme doit se lancer sans droits administrateur et sans création de préférences dans le profil du compte ayant fourni l'élévation.       |

Tester les quatre combinaisons suivantes en restaurant l'instantané initial
entre les installations. Le raccourci du menu Démarrer doit rester disponible.

| Bureau  | Démarrage à la connexion | Vérification après installation et reconnexion de chacun des deux comptes                        |
| ------- | ------------------------ | ------------------------------------------------------------------------------------------------ |
| Coché   | Décoché                  | Valeurs proposées par défaut ; raccourci public présent, application non lancée automatiquement. |
| Décoché | Décoché                  | Aucun raccourci public sur le Bureau, aucun démarrage automatique.                               |
| Coché   | Coché                    | Raccourci public présent, application lancée masquée dans la barre système à la connexion.       |
| Décoché | Coché                    | Aucun raccourci public sur le Bureau, application lancée masquée à la connexion.                 |

## Profils et fonctions de dictée

| Résultat | Test                            | Action et résultat attendu                                                                                                                                                                                                                                                                                                                                                                            |
| -------- | ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
|          | Premier profil                  | Sous `TestA`, lancer l'application. Vérifier les fichiers `settings.json`, `dictionary.json` et `formatting_rules.json` dans `%APPDATA%\lan.ch-vauclaire.murmure\`. Comparer leur contenu fonctionnel avec les modèles `resources/hospital/default-*.json` du commit compilé, sans exiger le même ordre JSON.                                                                                         |
|          | Valeurs initiales               | Vérifier : langue française, microphone automatique, raccourci F9, mode appui pour démarrer/arrêter, effets sonores activés à 50 %, insertion Standard, conversion des nombres désactivée. Les raccourcis Annuler et Dernière transcription sont initialement non assignés.                                                                                                                           |
|          | Deux utilisateurs               | Modifier le microphone, un raccourci et le dictionnaire de `TestA`. Ouvrir ensuite une session `TestB` : son profil doit partir des modèles embarqués et ne pas reprendre les changements de `TestA`. Revenir sur `TestA` et retrouver ses choix.                                                                                                                                                     |
|          | Dictée F9                       | Dans le Bloc-notes, appuyer sur F9, dicter une phrase fictive puis appuyer à nouveau sur F9. La capture doit s'arrêter, l'overlay revenir à l'état de repos et le texte être inséré une seule fois au curseur.                                                                                                                                                                                        |
|          | Appuyer pour parler             | Choisir le mode maintien, maintenir F9 pendant la phrase puis relâcher. L'enregistrement doit s'arrêter au relâchement. Revenir ensuite au mode initial.                                                                                                                                                                                                                                              |
|          | Raccourcis personnalisés        | Affecter temporairement Ctrl+Espace à la dictée et tester. Aucun ancien mode masqué ne doit se déclencher. Affecter aussi des raccourcis libres à Annuler et Dernière transcription, puis tester leurs actions. Consigner un éventuel conflit avec Windows ou une autre application.                                                                                                                  |
|          | Microphone                      | Choisir un autre microphone, dicter, le débrancher puis le reconnecter. Vérifier la sélection affichée, les messages d'indisponibilité et la possibilité de reprendre une dictée après sélection d'une entrée disponible.                                                                                                                                                                             |
|          | Sons et volume                  | Écouter les sons de début/fin, diminuer puis augmenter leur volume, désactiver les sons et refaire une dictée. Les choix doivent être respectés et rester enregistrés après un arrêt complet de l'application.                                                                                                                                                                                        |
|          | Insertion dans les applications | Tester Standard puis Direct dans le Bloc-notes et dans chaque application cible du pilote, avec leurs noms et versions notés dans la fiche. Inclure accents, nombres, saut de ligne et insertion au milieu d'une phrase. Utiliser des comptes et niveaux de droits représentatifs ; un essai dans le Bloc-notes ne valide pas les autres logiciels.                                                   |
|          | Presse-papiers et focus         | En mode Standard, copier un texte fictif avant la dictée. Après insertion de la transcription, vérifier que l'ancien texte du presse-papiers est réutilisable. Lors d'un changement volontaire de focus pendant une capture, relever la fenêtre qui reçoit le texte et valider ce comportement avec l'usage prévu.                                                                                    |
|          | Dictionnaire                    | Ajouter le terme de démonstration « Xylomètre », vérifier sa présence après redémarrage, puis dicter plusieurs phrases le contenant. Noter le résultat de reconnaissance : l'ajout au dictionnaire ne garantit pas chaque transcription. Supprimer ensuite le terme.                                                                                                                                  |
|          | Commande personnelle            | Créer un remplacement « texte de démonstration » → `TEST`. Dicter cette expression et vérifier le remplacement, sa conservation après redémarrage, puis son retrait après suppression de la règle.                                                                                                                                                                                                    |
|          | Ponctuation et pauses           | Dicter « Bonjour insérer une virgule ceci est un test insérer un point » en prononçant les commandes et en faisant des pauses avant/après. Vérifier une seule virgule et un seul point finaux, sans commande résiduelle. Tester aussi « saut de ligne », des parenthèses et plusieurs commandes successives.                                                                                          |
|          | Nombres                         | Tester le réglage de conversion désactivé puis activé avec une même phrase fictive. Vérifier également un nombre décimal et l'absence de ponctuation doublée. Noter les résultats ; restaurer le réglage initial.                                                                                                                                                                                     |
|          | Conservation des choix          | Quitter complètement depuis la barre système, relancer, puis redémarrer Windows. Vérifier microphone, mode, raccourcis, sons, dictionnaire et règles personnelles. Un profil existant ne doit pas être réinitialisé par les modèles embarqués.                                                                                                                                                        |
|          | Historique et fermeture         | Faire six dictées distinctes : seules les cinq dernières doivent apparaître. Fermer la fenêtre avec sa croix ou Alt+F4 : l'application reste dans la barre système et l'historique reste disponible. Choisir ensuite Quitter puis relancer : l'historique doit être vide avec le modèle actuel (`persist_history=false`).                                                                             |
|          | Audio temporaire                | Après les dictées et un arrêt normal, vérifier l'absence de WAV conservés dans `%TEMP%\lan.ch-vauclaire.murmure\recordings\`, avec `keep_recordings=false`. Signaler les fichiers restants sans les ouvrir ni joindre de contenu sensible à un rapport.                                                                                                                                               |
|          | Guide et crédits                | Hors ligne, cliquer sur Guide d'utilisation : le navigateur doit afficher le guide local avec ses images et le message prévu. Vérifier aussi les logos et les crédits de Murmure dans la page de présentation.                                                                                                                                                                                        |
|          | Mises à jour maîtrisées         | Vérifier l'absence de bouton de mise à jour. Avec un outil réseau/pare-feu disponible, observer uniquement le processus de l'application et les processus WebView2 qui lui sont associés : aucun appel de recherche de version officielle ne doit apparaître. Ne pas attribuer au programme l'activité globale de Microsoft Edge Update. L'essai hors ligne seul ne prouve pas l'absence de requêtes. |

## SpeechMike SMP3700, si utilisé pendant le pilote

Rediriger le périphérique USB complet vers la VM : la redirection du son seul
ne transmet pas les boutons. Choisir le **mode Event**, suivant le guide du
microphone embarqué, puis sélectionner son entrée audio dans Murmure Vauclaire.

| Résultat | Test                      | Action et résultat attendu                                                                                                                                                                                                                                                                 |
| -------- | ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
|          | Profil et initialisation  | L'image et les trois commandes doivent remplacer les raccourcis clavier dans Système ; le choix du microphone reste accessible. Si demandé, appuyer puis relâcher REC une première fois, sans dicter, et attendre « SpeechMike prêt ». Le branchement seul ne doit pas lancer une capture. |
|          | REC                       | Tester démarrage/arrêt en mode bascule, puis maintien/relâchement en mode appuyer pour parler. Le comportement doit suivre le mode sélectionné.                                                                                                                                            |
|          | F1 et F2                  | Pendant une dictée, F1 doit annuler sans insérer le texte en cours. Après une dictée terminée, F2 doit réinsérer la dernière transcription disponible.                                                                                                                                     |
|          | Exclusivité des commandes | Avec le profil SpeechMike actif, les trois raccourcis clavier de dictée doivent rester inactifs. Choisir une autre entrée audio : l'interface clavier doit revenir et les boutons du SpeechMike ne doivent plus piloter la dictée. Les préférences clavier doivent être conservées.        |
|          | Déconnexion               | Débrancher le SpeechMike pendant une capture déclenchée par REC : la capture doit être annulée sans texte partiel inséré. Rebrancher, réinitialiser si demandé et vérifier qu'une nouvelle capture exige une nouvelle action volontaire.                                                   |

Une validation sur VM ne remplace pas un essai du SpeechMike sur un poste
Windows natif représentatif de l'établissement.

## Maintenance et décision de pilote

| Résultat | Test               | Action et résultat attendu                                                                                                                                                                                                                                                                                                                                         |
| -------- | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
|          | Réparation         | Relancer le même setup et choisir Réparer. Vérifier le lancement et une dictée, la conservation des choix Bureau/démarrage et celle des préférences de chaque compte.                                                                                                                                                                                              |
|          | Mise à jour future | Quand une version applicative plus récente existe, l'installer au-dessus de celle testée : une seule application hospitalière doit rester, les préférences et les choix d'installation doivent être conservés. Réinstaller le même setup ne prouve pas une mise à jour ; noter cet essai Non testé tant qu'un package réellement plus récent n'est pas disponible. |
|          | Désinstallation    | Quitter l'application et désinstaller via le setup. Vérifier le retrait du programme, des raccourcis et de son démarrage automatique. Visual C++ et WebView2, partagés, doivent rester disponibles. Les profils utilisateur ne sont pas purgés ; une réinstallation peut donc retrouver leurs préférences.                                                         |

Avant le pilote, bloquer la distribution si l'un de ces points reste en échec :

- Installation hors ligne ou premier lancement impossible sur la configuration cible.
- Dictée impossible, texte perdu/dupliqué ou inséré dans la mauvaise application cible.
- Préférences mélangées entre comptes ou non conservées après un arrêt normal.
- Capture impossible à arrêter/annuler, ou démarrage sans action volontaire.
- Commandes SpeechMike défaillantes si ce matériel est prévu pour les utilisateurs du pilote.

Consigner les essais non réalisés, les applications effectivement validées et
les anomalies avant de décider du périmètre du pilote. Conserver le setup et
son empreinte avec la fiche remplie pour pouvoir reproduire l'essai.
