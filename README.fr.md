<p align="center">
  <img src="./src-tauri/icons/128x128.png" width="96" alt="LibrAgent" />
</p>

# LibrAgent

> **Un environnement de bureau pour agents IA exécuté en local sous votre contrôle total.**
> Connectez le modèle de votre choix, installez des outils en un clic et automatisez vos flux de travail grâce à des modèles de coordination multi-agents éprouvés.
> _Sans dépendance envers un fournisseur fermé et sans configuration JSON manuelle. Tous vos livrables sont enregistrés directement sur votre machine locale._

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

![Démo principale — Installez des outils comme des applications, choisissez votre modèle, conservez vos fichiers locaux.](./assets/hero-demo-60s.gif)

[Voir la vidéo HD WebM](./assets/hero-demo-60s.webm) · _Installez des outils comme des applications, choisissez votre modèle, conservez vos fichiers locaux._

---

## Ce qui différencie LibrAgent

De nombreux frameworks d'agents partent du principe que vous maîtrisez les commandes de terminal, que vous modifiez manuellement des fichiers JSON complexes et que vous assemblez l'orchestration directement dans le code. D'autres solutions vous enferment dans l'écosystème d'un fournisseur unique.

LibrAgent résout ces contraintes en proposant une **application de bureau complète et prête à l'emploi** :

| Approche traditionnelle | Avec LibrAgent |
| ----------------------- | -------------- |
| Éditer manuellement des fichiers de configuration JSON MCP | **Extensions en un clic** — Préréglages immédiats (GitHub, Brave Search, système de fichiers) et importation transparente depuis Cursor, VS Code et Claude Code |
| Développer soi-même des systèmes multi-agents en code | **Modèles de coordination intégrés** — Compétences packagées prêtes à l'emploi (`pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`) |
| Dépendance stricte aux modèles et outils d'un seul fournisseur | **Liberté technologique totale** — Connectez vos clés d'API cloud ou un modèle local ([Ollama](https://ollama.com)) sous licence open source MIT |

[Télécharger la dernière version](https://github.com/fritzprix/libr-agent/releases/latest) · [Démarrage en 5 minutes](#guide-de-démarrage-en-5-minutes) · [Spécifications de la démo](docs/contributing/hero-demo-spec.md)

---

## Ce que vous pouvez accomplir en 10 minutes

### 1. Installer des outils en un clic et générer un livrable

- Ouvrez **Extensions** et installez un préréglage (ex. GitHub) sans aucune configuration JSON.
- Associez votre **Workspace** à un dossier de projet local.
- Soumettez la consigne : _"Analyse ce dépôt pour identifier les risques principaux pour un nouveau contributeur et enregistre le rapport dans `DELIVERABLE.md`"_

### 2. Déployer une recette de flux de travail automatisée

- Lancez la recette **Morning Briefing** depuis l'accueil de discussion ou les [Tâches planifiées](docs/user/guides/scheduled-tasks.md).
- Les outils Hacker News et Yahoo Finance sont configurés automatiquement pour une exécution quotidienne à 09h00.
- Recevez automatiquement chaque matin une synthèse d'actualité technologique et financière.

### 3. Choisir un modèle de coordination sans programmation

- Décrivez simplement votre méthode de travail dans l'invite :
  - _"@skill:pipeline — recherche, puis rédaction, puis relecture ; produis un rapport final unique"_
  - _"@skill:divide-conquer — divise cette tâche en étapes indépendantes exécutées en parallèle et fusionne les résultats"_
- Les modèles de coordination sont des compétences intégrées ne nécessitant aucun SDK externe. Consultez le guide [Sous-agents et orchestration](docs/user/guides/sub-agents.md).

### 4. Conserver le contrôle total de vos modèles

- **Modèles Cloud** : Renseignez une clé d'API pour OpenAI, Anthropic, Gemini ou Groq.
- **Modèles Locaux** : Exécutez `ollama pull qwen3:14b` et sélectionnez Ollama dans les paramètres pour un fonctionnement 100 % hors ligne.

---

## Trois engagements fondamentaux

1. **Une interface graphique intuitive sans configuration fastidieuse** — Interface moderne, extensions en un clic, recettes automatisées et approbations intégrées éliminant le recours obligatoire aux terminaux.
2. **L'orchestration comme fonctionnalité native** — Choisissez des flux séquentiels, en étoile (Hub-and-spoke) ou distribués (Swarm) via des compétences, et évoluez vers des équipes permanentes (`teamwork`) ou des tâches récurrentes (`schedule`).
3. **Autonomie complète des modèles et des données** — Conçu sur le standard ouvert MCP (Model Context Protocol). Vos espaces de travail, historiques de session et données de navigation restent strictement stockés sur votre machine locale.

**Public cible :** Professionnels souhaitant exploiter la puissance des agents IA sans complexité technique ; développeurs refusant l'enfermement propriétaire ; chercheurs ayant besoin d'automatisation de navigateur, de bases de connaissances et de planification dans un seul outil.

---

## Modèles de coordination intégrés (Compétences packagées)

Sélectionnez le modèle adapté à la structure de votre tâche directement depuis le panneau de discussion :

| Nom de compétence | Modèle de coordination | Cas d'usage recommandé |
| ----------------- | ---------------------- | ---------------------- |
| `pipeline` | Étapes séquentielles | La sortie d'une étape sert d'entrée à la suivante (Recherche → Rédaction → Relecture) |
| `hub-spoke` | Hub-and-Spoke | Un agent coordinateur central pilote et intègre plusieurs agents spécialisés |
| `divide-conquer` | Division parallèle | Découper le travail en sous-tâches indépendantes en parallèle, puis fusionner |
| `consensus-delegation` | Multi-perspectives | Soumettre la même question à plusieurs agents experts pour concilier les analyses |
| `gatekeeper` / `pair-programming` | Boucles de revue | Relecture stricte de code ou programmation collaborative en binôme |
| `delegate` | Délégation ciblée | Déléguer une tâche précise à une sous-session avec suivi complet de la lignée |
| `teamwork` → `org` | Équipe permanente | Organiser des équipes durables avec charte partagée et organigramme interactif |
| `schedule` / `loop` / `call-me-back` | Événements & temps | Planifications Cron, délais programmés ou reprise sur événement externe |

Guide de sélection : [Sélection de framework](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · Guide complet : [Sous-agents](docs/user/guides/sub-agents.md)

Autres compétences prêtes à l'emploi : `setup-wizard`, `tool-installer`, `playbook-creator` et plus — voir **[Compétences intégrées](docs/user/guides/skills.md)**

---

## Infrastructure d'exécution centrale MCP

- **Prise en charge complète des protocoles** : stdio, HTTP, SSE et OAuth 2.1
- **Plus de 15 serveurs intégrés** : Workspace, Shell, Browser, Knowledge, Planning, Scheduled Tasks, etc.
- **Préréglages et installation interactive** : Déploiement guidé par agent avec `tool-installer`
- **Isolation et sécurité strictes** : Isolation des outils par session, restrictions d'accès aux fichiers et commandes, modes d'automatisation sécurisés

### Capacités centrales d'exécution

| Domaine | Fonctionnalités |
| ------- | --------------- |
| **Workspace** | Édition précise de code par ligne, opérations multifichiers, injection de contexte `@file` / `@skill` / `@playbook` |
| **Shell** | Environnements de terminal isolés et persistants avec surveillance asynchrone des processus |
| **Browser** | Navigateur sandboxé dédié et passerelle d'extension pour interagir avec vos sessions Chrome habituelles |
| **Knowledge** | Graphe de connaissances et moteur de recherche hybride BM25 haute performance |
| **Export** | Rapports Markdown structurés et exportation standardisée des trajectoires de session ATIF ([Exportation de session](docs/user/guides/session-export.md)) |

La compression automatique de contexte, les disjoncteurs antiboucle et la validation des réponses garantissent des sessions longues stables et performantes.

---

## Scénarios d'utilisation concrets

### Opérateur métier — Synthèse matinale automatisée

1. Exécutez la recette **Morning Briefing** (installation des outils, configuration de l'assistant, planification quotidienne à 09h00).
2. Cliquez sur **Run now** pour valider immédiatement le fonctionnement.
3. Consultez chaque matin votre rapport d'actualité sans jamais avoir à ouvrir un terminal.

### Développeur indépendant — Intégration immédiate d'outils

1. Installez le préréglage GitHub MCP depuis le menu Extensions.
2. Associez un dépôt de code local via le Workspace.
3. Demandez à l'agent de produire un rapport complet de sécurité et d'architecture sauvegardé localement.

### Utilisateur avancé — Délégation par modèles éprouvés

1. Saisissez `@skill:pipeline` (ou `hub-spoke`, `divide-conquer`) dans le champ de discussion.
2. Les agents collaborent harmonieusement selon le modèle retenu.
3. Obtenez un livrable consolidé dans votre espace de travail sans devoir maintenir de bibliothèque de code externe.

### Équipe soucieuse de la confidentialité — Exécution 100 % locale

1. Exécutez `ollama pull qwen3:14b`.
2. Toutes les opérations de Workspace et de Shell restent strictement traitées sur votre machine locale.
3. Passez librement à des modèles cloud selon vos besoins, sans changer vos habitudes d'utilisation.

---

## Documentation

- **[Guide utilisateur (User Guide)](docs/user/README.md)** — Installation, premiers pas, modèles et compétences ([Site de documentation](https://fritzprix.github.io/libr-agent/))
- **[Spécifications de la démo (Hero Demo Spec)](docs/contributing/hero-demo-spec.md)** — Spécifications de la vidéo de démonstration (sous-titres EN/KO/ZH)
- **[Guide de positionnement produit](docs/contributing/product-messaging-guide.md)** — Lignes directrices de positionnement et terminologie
- **[Recettes](docs/user/guides/recipes.md)** · **[Tâches planifiées](docs/user/guides/scheduled-tasks.md)** · **[Sous-agents](docs/user/guides/sub-agents.md)** · **[Compétences](docs/user/guides/skills.md)**
- **[API HTTP](docs/api/http_api.md)** — Contrôle à distance et approbations programmatiques
- **[Architecture système](docs/architecture/agent-workflow-architecture.md)** — Isolation de session et boucle Think-Act-Observe

---

## Démarrage rapide

Téléchargez le dernier programme d'installation depuis la **[Page des versions (Releases)](https://github.com/fritzprix/libr-agent/releases/latest)**.

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.22_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_x64-setup.exe) · [`LibrAgent_0.9.22_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.22_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.22_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_amd64.AppImage) · [`LibrAgent_0.9.22_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_amd64.deb) · [`LibrAgent-0.9.22-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent-0.9.22-1.x86_64.rpm)
- **Tous les fichiers publiés:** [Page des versions](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.22)
<!-- RELEASE_DOWNLOADS_END -->

### Guide de démarrage en 5 minutes

**Étape 1 — Connecter un modèle** (Settings → LLM Providers)

- Services Cloud : Saisissez votre clé d'API OpenAI, Anthropic, Gemini ou Groq.
- Exécution locale : Exécutez `ollama pull qwen3:14b` et sélectionnez Ollama dans les paramètres.

**Étape 2 — Ajouter des outils sans configuration JSON**

- Installez un préréglage (ex. GitHub) dans le menu Extensions, ou
- Indiquez à l'agent : _"Importe mes serveurs MCP depuis Cursor"_

**Étape 3 — Associer un espace de travail et générer un livrable**

- Sélectionnez votre dossier de projet local dans Workspace.
- Indiquez à l'agent : _"Analyse cet espace de travail et consigne les conclusions dans `DELIVERABLE.md`"_

**Étapes suivantes — Coordination et automatisation avancées**

- _"@skill:pipeline — effectue des recherches sur ce sujet, rédige une synthèse, relis-la et enregistre le rapport final"_
- _"Prépare un espace de travail teamwork pour ce dépôt"_
- _"Mets en place une tâche planifiée quotidienne de veille concurrentielle à 07h00"_ (ou utilisez la recette Morning Briefing)

### Exemples d'invites prêtes à l'emploi

- _"Importe ma configuration de serveurs MCP depuis Cursor et indique-moi les outils ajoutés."_
- _"Installe le préréglage GitHub MCP et connecte-le à un agent de développement."_
- _"Passe en revue cet espace de travail et rédige des pistes d'amélioration concrètes dans `DELIVERABLE.md`."_
- _"@skill:pipeline — approfondis ce sujet d'étude, rédige une synthèse, effectue une revue croisée et sauvegarde le rapport final."_
- _"Configure une tâche planifiée pour générer automatiquement une synthèse d'actualité financière chaque matin à 07h00."_

### Configuration pour développeurs

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## Philosophie de conception

- **Produit fini plutôt qu'un kit d'assemblage** : Utilisable immédiatement sans programmation préalable complexe.
- **L'orchestration incarnée par des compétences** : Les modèles de collaboration sont standardisés et documentés avec clarté.
- **Liberté totale de la pile technique** : L'utilisateur conserve le choix de ses modèles et de ses outils sans dépendance imposée.
- **Priorité au local (Local First)** : Espaces de travail, sessions et états de navigation restent préservés sur votre équipement.
- **Priorité à l'environnement d'exécution plutôt qu'au modèle seul** : L'intégration rigoureuse des outils, la gestion d'état et la gouvernance produisent des résultats plus fiables que les seules performances d'un modèle isolé.
- **Stabilité privilégiée face à la surenchère de fonctionnalités** : L'isolation de contexte, la compression mémoire et la prévention des boucles sont rigoureusement validées.
- **Engagement résolu pour les standards ouverts** : Licence libre MIT avec adoption du protocole MCP (Model Context Protocol) comme socle fondamental d'interopérabilité.

---

## Contributions et licence

LibrAgent est un projet open source sous licence MIT. Toutes les contributions (compétences, intégrations MCP, corrections de bugs ou optimisations d'architecture) sont chaleureusement encouragées.

- [Guide de contribution (Contributing Guide)](CONTRIBUTING.md)
- [Gestionnaire de tickets (Issue Tracker)](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- [Discussions communautaires](https://github.com/fritzprix/libr-agent/discussions)
- Bancs d'essai (Harbor / Terminal-Bench) : consultez le [guide Harbor](benchmarks/harbor/README.md) (`pnpm bench:diverse`, `pnpm bench:terminal`, …)

**Licence** : MIT
