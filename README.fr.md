# 🤖 LibrAgent

> **Une plateforme d'agents IA autonomes, légère et avec état.**

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent est un exécuteur d'agents local-first conçu pour maintenir le contexte entre les interactions. Contrairement aux clients sans état, il garde les onglets du navigateur et les sessions du terminal actifs entre les tours, permettant aux agents de travailler plus fluidement dans un espace de travail persistant.

Il implémente des standards ouverts comme **MCP (Model Context Protocol)** et **Skills** pour rester modulaire et extensible.

---

## Pourquoi LibrAgent ?

L'objectif de ce projet est de rendre les agents autonomes accessibles. De nombreux outils existants restent piégés derrière des commandes de terminal et des configurations JSON manuelles, créant un fossé qui exclut de nombreux utilisateurs potentiels. LibrAgent vise à combler ce fossé en fournissant un environnement local-first où n'importe qui peut déployer et gérer des agents sans avoir besoin d'être un développeur.

---

## 🎬 Démo

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

_Automatisation du navigateur et exécution du shell dans un seul flux de travail avec état._

---

## Caractéristiques Principales

### 1. Espace de Travail Persistant

Les agents opèrent dans un environnement à longue durée de vie plutôt que de lancer de nouveaux processus à chaque tour.

- **Webview en Direct**: Automatisation du navigateur en temps réel via Tauri webviews. Les sessions et les cookies persistent.
- **Terminal Unifié**: Un shell persistant et sandboxé (supporte Python/Node.js) qui partage son état avec l'espace de travail.

### 2. Orchestration Multi-Agents

LibrAgent permet aux agents de déléguer des tâches à des sous-agents spécialisés.

- **Assistants**: Gérez les profils d'agents avec des prompts système et des configurations d'outils uniques.
- **Intelligence en Essaim (Swarm)**: Les agents parents peuvent créer, envoyer des messages et attendre les résultats des sous-agents.

### 3. Extensibilité

La plateforme est conçue pour être étendue via des standards communautaires.

- **Extensions (MCP)**: Support complet du protocole MCP. Connectez-vous instantanément à n'importe quel serveur MCP.
- **Préréglages en un Clic**: Catalogue sélectionné pour GitHub, Brave Search, etc., disponible directement dans l'UI.
- **Skills & Playbooks**: Snippets de comportement réutilisables et modèles de flux de travail structurés.

### 4. Autonomie & Planification

- **Mode YOLO**: Exécution autonome optionnelle pour les outils sensibles sans approbation manuelle.
- **Tâches Planifiées**: Automatisation basée sur Cron avec ciblage d'espace de travail et récupération automatique après redémarrage.

### 5. Contexte & Métriques

- **@mentions**: Injection directe de fichiers, de compétences ou de playbooks dans le chat.
- **Multimodal**: Gère les images et l'audio pour les modèles OpenAI, Anthropic et Gemini.
- **Observabilité**: Métriques TPS en temps réel et hits de cache de prompt (pour Anthropic/Gemini).

---

## 📦 Installation

Téléchargez les derniers binaires pour Windows, macOS ou Linux depuis la [page des Releases](https://github.com/fritzprix/libr-agent/releases/latest).

**Build depuis la source :**

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## Choix de Conception

- **Local First**: Vos données et clés API restent sur votre machine.
- **Tauri + Rust**: Choisi pour la sécurité (sécurité mémoire), la performance et la taille réduite du binaire.
- **SQLite (SeaORM)**: Utilisé pour une persistance locale robuste des sessions et des configurations.

---

## Contribution & Licence

Les contributions sont les bienvenues. Veuillez consulter `CONTRIBUTING.md`.

**Licence** : MIT
