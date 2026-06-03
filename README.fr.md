# 🤖 LibrAgent

> **Une application desktop local-first pour des agents IA qui utilisent de vrais outils, travaillent en parallèle et restent sous votre contrôle.**
> _Connectez n'importe quel LLM, ajoutez n'importe quel serveur MCP, et laissez les agents lire des fichiers, lancer des shells, naviguer sur le web et automatiser de vraies tâches jusqu'au bout._

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent est un **espace de travail d'agents local-first** construit avec Tauri + Rust + React. Ce n'est pas juste une autre interface de chat : il est pensé pour l'accès réel aux fichiers, l'exécution shell, l'automatisation navigateur, l'extensibilité MCP et les workflows multi-agents qui tiennent pendant des heures au lieu de s'écrouler après une jolie démo.

Vous pouvez connecter des modèles cloud ou des runtimes locaux comme Ollama, importer les serveurs MCP des outils que vous utilisez déjà, puis confier aux agents l'inspection de code, l'édition de fichiers, l'exécution de commandes, la navigation web, la capture de connaissances et la délégation de sous-tâches — sans expédier tout votre workflow sur la VM cloud de quelqu'un d'autre.

**Commencez ici :** [Télécharger la dernière release](https://github.com/fritzprix/libr-agent/releases/latest) · [Aller au parcours d'intégration en 5 minutes](#parcours-dintégration-en-5-minutes) · [Voir les scénarios concrets](#-scénarios-concrets)

---

## Pourquoi LibrAgent ?

La plupart des produits d'agents imposent encore un compromis pénible :

- **Une UI facile, mais une exécution faible**
- **Une automatisation puissante, mais une expérience produit bancale**
- **Le confort du cloud, mais peu de contrôle sur la confidentialité**
- **Des frameworks flexibles, mais à vous de monter toute la stack**

LibrAgent vise le milieu que les gens veulent vraiment :

- **Un contrôle local-first** sur les fichiers, workspaces, sessions et état navigateur
- **Une extensibilité ouverte via MCP** au lieu d'un récit plugin fermé
- **Une vraie capacité d'exécution** sur shell, browser, workspace et knowledge
- **Une GUI utilisable par des humains normaux** sans sacrifier la profondeur power-user
- **Un passage naturel d'un agent à plusieurs** quand un seul assistant ne suffit plus

### À qui s'adresse LibrAgent

- **Développeurs solo** qui veulent des agents capables de vraiment lire, éditer, exécuter, naviguer et persister du contexte localement
- **Utilisateurs avancés et opérateurs** qui veulent composer leur propre stack depuis des modèles locaux, des fournisseurs API, des serveurs MCP et des workflows planifiés
- **Chercheurs et analystes** qui ont besoin d'automatisation du navigateur, de capture de connaissances, de playbooks répétables et de sessions longue durée
- **Équipes soucieuses de la confidentialité** qui veulent une exécution locale, une gouvernance explicite et un chemin d'un agent unique vers une organisation coordonnée

---

## 🎬 La plateforme en action

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

_D'un agent unique à un essaim coordonné — délégation récursive, outillage MCP et espace de travail persistant dans un substrat unifié._

---

## Ce que vous pouvez faire dans les 10 premières minutes

### 1. Relire un dépôt avec de vrais outils

- Connectez un dépôt local avec l'outil Workspace
- Ajoutez le preset GitHub MCP
- Demandez : _"Trouve les problèmes de sécurité dans la PR #42 et sauvegarde le rapport"_

### 2. Monter une stack d'agents 100 % locale

- Lancez `ollama pull qwen3:14b`
- Connectez Workspace + Shell
- Laissez un agent lire, modifier, tester et itérer sans envoyer votre code vers une VM cloud

### 3. Transformer une recherche en workflow réutilisable

- Ajoutez Browser + Knowledge
- Demandez : _"Suis ces 5 blogs concurrents et résume-moi ça chaque matin"_
- Transformez une tâche ponctuelle en pipeline planifié

### 4. Passer d'un assistant à une vraie équipe

- Créez des spécialistes avec `specialist-creator`
- Répartissez le travail avec `delegate`
- Faites de la collaboration récurrente un workspace partagé `teamwork` ou `org`

---

## Pourquoi ça tient après la démo

### 1. 🔐 Sécurité local-first — Vos données restent sur votre machine

LibrAgent traite la sécurité comme une préoccupation architecturale de premier ordre :

- **Isolation de session** : Chaque session d'agent reçoit sa propre instance dédiée `MCPServiceProxy` — zéro fuite de données inter-sessions
- **SecurityValidator intégré** : Attaques par traversée de chemin et injection de commandes bloquées au niveau système
- **Aucun substrat cloud requis** : L'exécution principale se fait localement ; les connexions externes se limitent surtout aux fournisseurs LLM cloud et aux services MCP/HTTP distants que vous choisissez d'utiliser, ainsi qu'aux vérifications de mise à jour en production
- **Support hors ligne complet** : Associez avec [Ollama](https://ollama.ai) pour un stack d'agents entièrement isolé

#### Ce qui reste local vs ce qui quitte votre machine

- **Toujours local** : espaces de travail, fichiers locaux, compétences groupées, état de session, configs serveurs MCP, état du navigateur et exécution d'outils locaux
- **Quitte votre machine quand c'est nécessaire** : requêtes vers des fournisseurs LLM cloud ou services MCP/HTTP distants que vous configurez explicitement, ainsi que les vérifications de mise à jour en production
- **Mode hors ligne complet** : utilisez Ollama ou un autre runtime local avec des serveurs MCP locaux pour un workflow isolé

### 2. 🧩 Écosystème natif MCP — Extensibilité infinie par conception

MCP (Model Context Protocol) est le standard ouvert derrière le modèle d'extensibilité de LibrAgent. LibrAgent le traite non pas comme une fonctionnalité — mais comme la colonne vertébrale architecturale :

- **Support complet des transports** : stdio, HTTP, SSE et OAuth 2.1 — la spécification complète
- **12+ serveurs intégrés** : Planning, Knowledge (RAG), Browser Automation, Workspace, Shell Execution, Content Store, et plus
- **Catalogue de préréglages** : Installez GitHub, Brave Search, Filesystem et d'autres serveurs populaires en un clic
- **Instances isolées par session** : Chaque session d'agent a un état de serveur MCP indépendant — aucune interférence entre agents parallèles
- **Importez depuis n'importe où** : Migrez automatiquement les configs MCP depuis Cursor, VS Code, Claude Code ou Windsurf

### 3. 🦾 Substrat d'exécution de niveau production

La plupart des outils IA sont impressionnants en démo et fragiles en production. LibrAgent est obsessionnellement conçu pour un travail réel et durable :

| Substrat      | Capacités                                                                                                                      |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| **Workspace** | Édition précise à la ligne, opérations multi-fichiers, recherche unifiée, injection de contexte `@file`/`@skill`/`@playbook`   |
| **Shell**     | Exécution isolée ET shells persistants — surveillance de processus asynchrone (`poll`, `read output`, `list`)                  |
| **Browser**   | Automatisation de navigateur headless avec un modèle d'interaction proche de Playwright et des garanties de cohérence du cache |
| **Knowledge** | Gestion des connaissances basée sur les graphes avec extraction entité/relation (v2), recherche plein texte BM25               |

**Ingénierie de fiabilité incluse** : Compaction du contexte, prévention des boucles, disjoncteurs et gardes contre les réponses périmées maintiennent les agents productifs dans des sessions qui durent des heures.

### 4. 🤝 Essaim → Équipe → Organisation : Multi-agent à toutes les échelles

LibrAgent a une histoire multi-agent cohérente de l'exécution solo à la coordination organisationnelle explicite :

- **`delegate`** : Les agents parents génèrent, informent et surveillent des sessions enfants avec un suivi de lignée explicite
- **`teamwork`** : Construisez un espace de travail de task-force complet (agents.md, MISSION.md, KANBAN.md) avec une seule commande
- **`org`** : Formalisez les équipes avec une identité d'organisation durable, la reprise de session racine et une hiérarchie de membres visible
- **`schedule`** : Automatisation basée sur CRON — les agents s'exécutent sans surveillance, selon un calendrier, avec une constitution d'espace de travail
- **Concurrency Gate** : Limites strictes sur les sessions parallèles et les processus shell pour éviter les deadlocks et les coûts incontrôlés

### 5. ⚡ Compétences groupées — Le moyen le plus rapide d'aller d'une installation vierge à un essaim opérationnel

LibrAgent est livré avec une bibliothèque croissante de **Compétences groupées**. Ce ne sont pas des prompts aléatoires — ce sont des procédures opérationnelles réutilisables que n'importe quel agent peut invoquer par nom.

Les compétences les plus importantes pour le premier jour :

| Compétence           | Ce qu'elle fait                                                                                                  |
| -------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `system-setup`       | Détecte et installe les runtimes manquants (Python, Node.js, uv) sur toutes les plateformes                      |
| `mcp-installer`      | Enregistre des serveurs MCP depuis des packages npm, des URLs GitHub ou des blocs de config JSON                 |
| `mcp-importer`       | Importe les configs MCP existantes depuis Cursor, VS Code, Windsurf et autres                                    |
| `specialist-creator` | Conçoit une config d'agent complète (prompt système, modèle, outils) depuis une description de rôle              |
| `crew-constructor`   | Analyse les outils disponibles et crée automatiquement une équipe de spécialistes adaptée                        |
| `agent-tooling`      | Audite les agents, détecte les inadéquations de capacités et rééquilibre dynamiquement les affectations d'outils |
| `delegate`           | Guide le transfert de session parent→enfant avec transfert de contexte explicite et suivi de lignée              |
| `teamwork`           | Construit la constitution d'espace de travail partagé pour le travail multi-agent coordonné                      |
| `org`                | Formalise l'identité d'organisation durable et la hiérarchie de membres visible                                  |
| `schedule`           | Crée et gère des groupes de tâches planifiées récurrentes pour l'automatisation sans surveillance                |
| `soul-awakening`     | Ancre un agent à un persona `SOUL.md` — ton, posture, identité                                                   |

Et ce n'est que la couche opérateur. LibrAgent fournit également des compétences de domaine pour :

- **Connaissance et recherche** : `deep-research-report`, `knowledge-distiller`
- **Flux de travail documentaire** : `document-to-markdown`, `docx`, `pptx`
- **Création de compétences et workflows** : `skill-creator`, `skill-deployer`, `playbook-creator`, `mcp-builder`
- **Opérations spécialisées** : `computer-diagnosis` et autres assistants spécialisés

_Important : `bootstrap` est une capacité intégrée souvent utilisée avec ces compétences. Les Compétences groupées sont les procédures réutilisables ; les intégrés et les outils MCP sont le substrat d'exécution sous-jacent._

---

## 🌍 Scénarios réels

### Développeur solo — Revue de code automatisée

1. Connectez votre dépôt local via l'outil Workspace
2. Installez le préréglage GitHub MCP (un clic)
3. Demandez : _"Trouvez les problèmes de sécurité dans la PR #42 et produisez un rapport Markdown"_
4. L'agent lit le code, effectue l'analyse, sauvegarde les résultats dans le serveur Knowledge

### Marketeur — Veille concurrentielle en pilote automatique

1. Configurez 5 blogs concurrents via l'outil Browser
2. Dites à un agent : _"Crée un brief concurrentiel planifié chaque matin à 7h"_ — l'agent peut utiliser la compétence `schedule` pour configurer le groupe de tâches récurrent
3. L'agent navigue, résume et ajoute au Knowledge store
4. Demandez à tout moment : _"Résumez les mouvements des concurrents de la semaine dernière"_

### Équipe ingénierie — Stack d'agents hors ligne

1. `ollama pull qwen3:14b` — aucune clé API, aucun cloud
2. Connectez les outils Workspace + Shell à votre codebase
3. La propriété intellectuelle sensible ne quitte jamais la machine
4. Les agents lisent, modifient, testent et commitent — entièrement local

### Utilisateur avancé — Pipeline de recherche multi-agent

1. Utilisez `crew-constructor` pour générer automatiquement : Researcher×3, Analyst×1, Writer×1
2. L'orchestrateur délègue en parallèle via la compétence `delegate`
3. Les résultats fusionnent dans un rapport structuré unique dans Content Store
4. Planifiez le workflow entier hebdomadairement via `schedule`

---

## 📖 Documentation et guides

- **[Guide de navigation](docs/guides/navigation-guide.md)** : Le hub Command & Control — `/assistants` (Définitions de rôles) et `/playbooks` (Blueprints de workflow).
- **[Guide d'architecture](docs/architecture/agent-workflow-architecture.md)** : Isolation de session, moteur d'orchestration et boucle Think-Act-Observe pilotée par Rust.
- **[Guide des outils intégrés](docs/guides/builtin_tool_bp.md)** : Standards de conception d'outils et patterns de réponse MCP.

---

## 📦 Démarrage

Téléchargez le dernier installateur pour votre plateforme depuis la **[page des Releases](https://github.com/fritzprix/libr-agent/releases/latest)**.

<!-- RELEASE_DOWNLOADS_START -->
- **Windows :** [`LibrAgent_0.8.3_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.3/LibrAgent_0.8.3_x64-setup.exe) · [`LibrAgent_0.8.3_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.3/LibrAgent_0.8.3_x64_en-US.msi)
- **macOS (Apple Silicon) :** [`LibrAgent_0.8.3_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.3/LibrAgent_0.8.3_aarch64.dmg)
- **Linux :** [`LibrAgent_0.8.3_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.3/LibrAgent_0.8.3_amd64.AppImage) · [`LibrAgent_0.8.3_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.3/LibrAgent_0.8.3_amd64.deb) · [`LibrAgent-0.8.3-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.3/LibrAgent-0.8.3-1.x86_64.rpm)
- **Tous les fichiers de release :** [page des Releases](https://github.com/fritzprix/libr-agent/releases/tag/v0.8.3)
<!-- RELEASE_DOWNLOADS_END -->

**Configuration développeur :**

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

### Parcours d'intégration en 5 minutes

**Étape 1 — Connectez un modèle** (Settings → LLM Providers)

- Cloud : collez une clé API OpenAI / Anthropic / Gemini / Groq
- Local : `ollama pull qwen3:14b` puis sélectionnez Ollama dans Settings
- Vous utilisez déjà Cursor ou VS Code ? Dites à n'importe quel agent : _"Importe mes serveurs MCP depuis Cursor"_ → `mcp-importer` s'en charge

**Étape 2 — Ajoutez des outils MCP** (barre latérale Extensions)

- Parcourez le catalogue de préréglages et cliquez sur Installer, ou
- Dites à un agent : _"Install @modelcontextprotocol/server-everything"_ → `mcp-installer` l'enregistre automatiquement

**Étape 3 — Créez votre premier agent**

- _"Créez un agent chercheur pour la veille concurrentielle"_ → `specialist-creator` conçoit la config complète
- _"Construisez une équipe de recherche avec mes outils actuels"_ → `crew-constructor` crée les spécialistes en lot
- _"Optimisez les affectations d'outils pour tous mes agents"_ → `agent-tooling` audite et rééquilibre automatiquement

**Étape 4 — Passez en parallèle avec `delegate`**

- Demandez à n'importe quel agent de déléguer des sous-tâches à des sessions enfants
- La compétence `delegate` gère le transfert de contexte, le suivi de lignée et la fusion des résultats

**Étape 5 — Construisez une équipe persistante**

- `teamwork` → construit l'espace de travail partagé avec `agents.md`, `MISSION.md`, `KANBAN.md`
- `org` → formalise l'équipe avec une identité durable et la gestion de session racine
- `schedule` → laisse un agent créer et gérer l'automatisation CRON pour vous, sans surveillance

### Premiers prompts à copier-coller

- _"Importe mes serveurs MCP depuis Cursor et montre-moi ce qui a été ajouté."_
- _"Crée un agent chercheur pour la veille concurrentielle avec mes outils actuels."_
- _"Installe le préréglage GitHub MCP et attache-le à un agent de codage."_
- _"Délègue l'analyse du dépôt à une session enfant et rapporte-moi un résumé."_
- _"Prépare un espace de travail teamwork pour ce dépôt, puis crée une équipe de spécialistes prête pour l'org."_
- _"Configure un brief concurrentiel quotidien planifié à 7h et maintiens tout dans l'espace de travail teamwork partagé."_

---

## Là où LibrAgent est le plus pertinent

| Si vous voulez...                                                   | LibrAgent est fort parce que...                                                                      |
| ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| **Une station de travail IA locale**                                | fichiers, sessions, workspaces et état navigateur restent sur votre machine par défaut               |
| **Un produit desktop vraiment natif MCP**                           | vous pouvez installer, importer et gérer des serveurs MCP sans traiter l'app comme un simple wrapper |
| **Des agents qui font un vrai boulot**                              | Workspace, Shell, Browser et Knowledge sont conçus pour de l'exécution longue durée                  |
| **Des workflows multi-agents sans construire un framework d'abord** | `delegate`, `teamwork`, `org` et `schedule` font déjà partie du produit                              |
| **Un pont entre profondeur power-user et confort GUI**              | vous gardez une interface desktop sans perdre l'extensibilité ni le contrôle                         |

---

## Philosophie de conception

- **Local First** : Vos données, clés et "âmes" d'agents restent sous votre contrôle exclusif. Aucun substrat cloud requis.
- **Harnais avant modèle** : L'environnement d'exécution — outils, état de session, délégation, gouvernance — compte plus qu'un modèle individuel. LibrAgent est conçu pour maximiser ce que n'importe quel modèle peut faire.
- **Stabilité avant fonctionnalités** : Le CHANGELOG reflète un focus obsessionnel sur la correction du runtime — isolation de session, compaction, prévention des boucles, gardes contre les réponses périmées.
- **MCP comme infrastructure** : Pas un système de plugins. Tout l'écosystème d'outils est organisé autour de MCP comme couche d'interopérabilité principale.
- **Standards ouverts** : Licence MIT. Pleinement engagé envers MCP, l'interopérabilité open source et la souveraineté des données utilisateur.

---

## Contribution & Licence

LibrAgent est sous licence MIT et développé en open source. Les contributions sont les bienvenues — qu'il s'agisse de nouvelles compétences groupées, d'intégrations MCP, de corrections de bugs ou d'améliorations architecturales.

- 📖 [Guide de contribution](CONTRIBUTING.md)
- 🐛 [Suivi des problèmes](https://github.com/fritzprix/libr-agent/issues)
- 💬 [Discussions](https://github.com/fritzprix/libr-agent/discussions)

**Licence** : MIT
