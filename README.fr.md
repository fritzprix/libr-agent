# 🤖 LibrAgent

> **Un environnement d'exécution d'agents que vous faites tourner — choisissez le modèle, outils en un clic, choisissez le motif de coordination.**
> _Pas de harnais vendeur. Pas de devoirs JSON. Le travail se termine en fichiers sur votre machine._

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

![Démo hero — Installez les outils comme des apps. Gardez votre modèle. Gardez le fichier.](./assets/hero-demo-60s.gif)

[HD WebM](./assets/hero-demo-60s.webm) · _Installez les outils comme des apps. Gardez votre modèle. Gardez le fichier._

---

## Ce qui distingue LibrAgent

La plupart des harnais d'agents supposent que vous éditez du JSON MCP, vivez dans un terminal et assemblez l'orchestration en code (ou vous enferment dans la pile d'un seul vendeur).

LibrAgent est un **produit bureau** pour le même travail :

| Au lieu de… | Vous obtenez… |
| ----------- | ------------- |
| Éditer à la main les configs MCP | **Extensions** — préréglages en un clic (GitHub, Brave Search, Filesystem, …) et import depuis Cursor / VS Code / Claude Code / Windsurf |
| « On a du multi-agent » | Des **motifs de coordination nommés** en compétences groupées — `pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, … |
| Le modèle + outils d'un fournisseur | **Votre** LLM (clé API ou [Ollama](https://ollama.com)) et **votre** pile MCP — MIT, local d'abord |

[Télécharger la dernière version](https://github.com/fritzprix/libr-agent/releases/latest) · [Démarrage en 5 minutes](#démarrage-en-5-minutes) · [Spécification démo hero](docs/contributing/hero-demo-spec.md)

---

## Ce que vous pouvez faire dans les 10 premières minutes

### 1. Outils en un clic, puis un livrable

- Ouvrez **Extensions** et installez un préréglage (ex. GitHub) — pas de JSON
- Pointez **Workspace** vers un vrai dossier
- Demandez : _"Examine ce dépôt pour le principal risque pour un nouveau contributeur et enregistre `DELIVERABLE.md`"_

### 2. Déployer une recette de workflow en un clic

- Lancez la recette **Morning Briefing** depuis l'accueil Chat ou [Scheduled Tasks](docs/user/guides/scheduled-tasks.md)
- Installe les préréglages Hacker News + Yahoo Finance, configure un assistant et planifie une exécution quotidienne à 9 h
- Réveillez-vous avec un briefing tech & marchés synthétisé — sans surveillance

### 3. Choisir un motif de coordination (sans assembler un framework)

- Décrivez la forme du travail, ou attachez une compétence par nom :
  - _"@skill:pipeline — recherche, puis brouillon, puis relecture ; un seul rapport final"_
  - _"@skill:divide-conquer — découpe en pièces indépendantes et fusionne les résultats"_
- Les motifs sont des compétences productisées — pas un SDK à câbler soi-même. Voir [Sub-agents & orchestration](docs/user/guides/sub-agents.md).

### 4. Garder la liberté du modèle

- Cloud : collez une clé API OpenAI / Anthropic / Gemini / Groq
- Local : `ollama pull qwen3:14b` et sélectionnez Ollama — même harnais dans les deux cas

---

## Trois promesses produit

1. **Surface sans devoirs de harnais** — GUI, Extensions en un clic, recettes, approbations in-app, `@skill:` — pas « ouvre d'abord une config et un shell ».
2. **Orchestration comme produit** — choisissez des flux Sequential / Hub-and-spoke / style Swarm via skills ; passez à `teamwork` → `org` et `schedule` pour des équipes durables ou du cron — toujours sans assembler LangGraph/CrewAI vous-même.
3. **Liberté fournisseur & pile** — tout LLM pris en charge, MCP comme infrastructure, import des configs MCP IDE existantes, licence MIT, espaces de travail et état navigateur locaux par défaut.

**Meilleur profil :** opérateurs et power users qui veulent la profondeur du harnais sans vivre dans le JSON ; développeurs qui refusent une pile agent mono-vendeur ; chercheurs qui ont besoin navigateur + connaissance + plannings dans un seul produit.

---

## Motifs de coordination (compétences groupées)

Choisissez le modèle selon la **forme du travail**, puis lancez-le depuis le chat :

| Compétence | Motif | Quand l'utiliser |
| ---------- | ----- | ---------------- |
| `pipeline` | Étapes séquentielles | Les sorties alimentent l'étape suivante (recherche → brouillon → relecture) |
| `hub-spoke` | Hub-and-spoke | Un coordinateur intègre de nombreux workers |
| `divide-conquer` | Découpe parallèle | Pièces indépendantes, puis fusion |
| `consensus-delegation` | Multi-perspectives | Même question à plusieurs spécialistes, puis réconciliation |
| `gatekeeper` / `pair-programming` | Boucles de relecture | Relecture stricte ou codage à deux agents |
| `delegate` | Passation légère | Une session enfant, lignage suivi |
| `teamwork` → `org` | Équipe durable | Constitution partagée + Org UI |
| `schedule` / `loop` / `call-me-back` | Temps & événements | Cron, délais en session, ou reprise sur process/webhook |

Heuristiques de sélection : [framework-selection](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · guide complet : [Sub-agents](docs/user/guides/sub-agents.md).

Autres compétences du premier jour : `setup-wizard`, `tool-installer`, `playbook-creator`, et plus — **[Bundled Skills](docs/user/guides/skills.md)**.

---

## Plateforme MCP (toujours capable pour power users)

- Transports : stdio, HTTP, SSE, OAuth 2.1
- 15+ serveurs intégrés (Workspace, Shell, Browser, Knowledge, Planning, Scheduled Tasks, …)
- Préréglages en un clic + installation assistée par agent (`tool-installer`)
- Isolation des outils par session ; gardes chemin/commande ; modes YOLO / unsafe optionnels pour l'automatisation

### Substrat d'exécution

| Substrat | Capacités |
| -------- | --------- |
| **Workspace** | Édition ligne à ligne, ops multi-fichiers, contexte `@file` / `@skill` / `@playbook` |
| **Shell** | Shells isolés et persistants avec suivi de processus async |
| **Browser** | Sidecar navigateur isolé ; profils de connexion enregistrés optionnels |
| **Knowledge** | Connaissance graphe + recherche BM25 |
| **Export** | Rapports Markdown et exports de trajectoire ATIF ([session export](docs/user/guides/session-export.md)) |

Les longues sessions restent productives via compaction de contexte, prévention de boucles, disjoncteurs et gardes de réponses obsolètes.

---

## Scénarios réels

### Opérateur — de l'app vide au briefing quotidien

1. Lancez la recette **Morning Briefing** (préréglages + assistant + planning 9 h)
2. Cliquez **Run now** une fois pour vérifier
3. Laissez tourner — le rapport arrive sans ouvrir un terminal

### Développeur solo — préréglage, pas de fichiers de config

1. Extensions → installez le préréglage GitHub MCP
2. Attachez un dépôt local via Workspace
3. Demandez un rapport Markdown sécurité/relecture que vous garderez sur disque

### Power user sans framework — orchestration nommée

1. `@skill:pipeline` (ou `hub-spoke` / `divide-conquer`) selon la forme du travail
2. Les agents se coordonnent sous ce motif
3. Un livrable fusionné dans le workspace — aucune bibliothèque d'orchestration à maintenir

### Équipe sensible à la vie privée — même produit, modèle local

1. `ollama pull qwen3:14b`
2. Workspace + Shell restent sur la machine
3. Échangez les clés cloud plus tard si vous voulez — le harnais ne change pas de vendeur pour vous

---

## Documentation

- **[User Guide](docs/user/README.md)** — installation, premier chat, modèles, compétences ([docs site](https://fritzprix.github.io/libr-agent/))
- **[Hero Demo Spec](docs/contributing/hero-demo-spec.md)** — démo produit canonique (sous-titres EN/KO/ZH)
- **[Product Messaging Guide](docs/contributing/product-messaging-guide.md)** — positionnement et copy
- **[Recipes](docs/user/guides/recipes.md)** · **[Scheduled Tasks](docs/user/guides/scheduled-tasks.md)** · **[Sub-agents](docs/user/guides/sub-agents.md)** · **[Skills](docs/user/guides/skills.md)**
- **[HTTP API](docs/api/http_api.md)** — contrôle distant et approbations programmatiques
- **[Architecture](docs/architecture/agent-workflow-architecture.md)** — isolation de session et Think-Act-Observe

---

## Premiers pas

Téléchargez le dernier installateur depuis la **[page Releases](https://github.com/fritzprix/libr-agent/releases/latest)**.

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.21_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_x64-setup.exe) · [`LibrAgent_0.9.21_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.21_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.21_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_amd64.AppImage) · [`LibrAgent_0.9.21_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_amd64.deb) · [`LibrAgent-0.9.21-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent-0.9.21-1.x86_64.rpm)
- **All release assets:** [Releases page](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.21)
<!-- RELEASE_DOWNLOADS_END -->

### Démarrage en 5 minutes

**Étape 1 — Connecter un modèle** (Settings → LLM Providers)

- Cloud : collez une clé API OpenAI / Anthropic / Gemini / Groq
- Local : `ollama pull qwen3:14b`, puis sélectionnez Ollama dans Settings

**Étape 2 — Ajouter des outils sans JSON**

- Extensions → installez un préréglage (ex. GitHub), **ou**
- Dites à un agent : _"Importe mes serveurs MCP depuis Cursor"_

**Étape 3 — Attacher un workspace et demander un fichier que vous gardez**

- Pointez Workspace vers un vrai dossier de projet
- _"Examine ce workspace, puis écris les findings dans `DELIVERABLE.md`."_

**Ensuite — coordination et automatisation**

- _"@skill:pipeline — recherche, brouillon, puis relecture ; un rapport final."_
- _"Prépare un workspace teamwork pour ce dépôt."_
- _"Configure un brief concurrentiel quotidien à 7 h."_ (ou lancez la recette Morning Briefing)

### Premiers prompts à copier-coller

- _"Importe mes serveurs MCP depuis Cursor et montre-moi ce qui a été ajouté."_
- _"Installe le préréglage GitHub MCP et attache-le à un agent de codage."_
- _"Examine ce workspace, puis écris les findings dans `DELIVERABLE.md`."_
- _"@skill:pipeline — recherche ce sujet, rédige un résumé, puis relis ; enregistre le rapport final."_
- _"Configure un brief concurrentiel quotidien à 7 h."_

### Configuration développeur

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## Où LibrAgent convient le mieux

| Si vous voulez… | LibrAgent est fort parce que… |
| --------------- | ----------------------------- |
| **Profondeur de harnais sans devoirs de harnais** | Préréglages Extensions, recettes, motifs `@skill:`, et approbations — pas un onboarding JSON d'abord |
| **Orchestration sans construire un framework** | `pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, `teamwork` / `org`, `schedule` livrés comme produit |
| **Liberté face à une pile agent vendeur** | Apportez votre modèle et vos outils MCP ; MIT ; local d'abord par défaut |
| **Un vrai substrat d'exécution** | Workspace, shell, browser, knowledge, playbooks, et gardes de sessions longues |
| **Un produit bureau natif MCP** | Préréglages, import, et 15+ builtins — pas une fine enveloppe de chat |

---

## Philosophie de conception

- **Produit plutôt que kit** : le harnais est utilisable sans l'assembler.
- **Orchestration comme compétences** : les motifs de coordination sont nommés, sélectionnables et documentés — pas enterrés dans des dépôts d'exemples.
- **Liberté de pile** : modèles et outils sont des choix utilisateur ; le produit n'exige pas un vendeur IA.
- **Local First** : workspaces, sessions, skills et état navigateur restent sous votre contrôle. Cloud LLM / MCP distant seulement si vous optez pour.
- **Harnais plutôt que modèle** : outils, état de session, délégation et gouvernance comptent plus qu'un seul modèle.
- **Stabilité plutôt que fonctionnalités** : isolation, compaction, prévention de boucles — avant la course aux features.
- **Standards ouverts** : MIT. MCP comme couche d'interopérabilité.

---

## Contribution & licence

LibrAgent est sous licence MIT et construit au grand jour. Les contributions sont les bienvenues — compétences groupées, intégrations MCP, correctifs ou améliorations d'architecture.

- 📖 [Contributing Guide](CONTRIBUTING.md)
- 🐛 [Issue Tracker](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- 💬 [Discussions](https://github.com/fritzprix/libr-agent/discussions)
- 🧪 Benchmarks (Harbor / Terminal-Bench) : voir [Harbor guide](benchmarks/harbor/README.md) (`pnpm bench:diverse`, `pnpm bench:terminal`, …)

**License**: MIT
