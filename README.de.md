# 🤖 LibrAgent

> **Eine leichtgewichtige, stateful Plattform für autonome KI-Agenten.**

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent ist ein local-first Agenten-Runner, der darauf ausgelegt ist, den Kontext über Interaktionen hinweg beizubehalten. Im Gegensatz zu zustandslosen Clients hält er Browser-Tabs und Terminal-Sitzungen zwischen den Runden aktiv, sodass Agenten flüssiger in einem persistenten Arbeitsbereich arbeiten können.

Es implementiert offene Standards wie **MCP (Model Context Protocol)** und **Skills**, um modular und erweiterbar zu bleiben.

---

## Warum LibrAgent?

Das Ziel dieses Projekts ist es, autonome Agenten zugänglich zu machen. Viele bestehende Tools bleiben hinter Terminalbefehlen und manuellen JSON-Konfigurationen verborgen, was eine Lücke schafft, die viele potenzielle Nutzer ausschließt. LibrAgent zielt darauf ab, diese Lücke zu schließen, indem es eine local-first Umgebung bereitstellt, in der jeder Agenten bereitstellen und verwalten kann, ohne Entwickler sein zu müssen.

---

## 🎬 Demo

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

*Browser-Automatisierung und Shell-Ausführung in einem einzigen, zustandsorientierten Workflow.*

---

## Kernfunktionen

### 1. Persistenter Arbeitsbereich
Agenten agieren in einer langlebigen Umgebung, anstatt für jede Runde neue Prozesse zu starten.
- **Live-Webansicht**: Echtzeit-Browser-Automatisierung mit Tauri-Webviews. Sitzungen und Cookies bleiben über Runden hinweg erhalten.
- **Einheitliches Terminal**: Eine persistente, sandbox-geschützte Shell (Unterstützung für Python/Node.js), die den Zustand mit dem Arbeitsbereich teilt.

### 2. Multi-Agenten-Orchestrierung
LibrAgent ermöglicht es Agenten, Aufgaben an spezialisierte Sub-Agenten zu delegieren.
- **Assistenten**: Verwalten Sie Agentenprofile mit individuellen System-Prompts und Tool-Konfigurationen.
- **Schwarmintelligenz**: Eltern-Agenten können Sub-Agenten erstellen, benachrichtigen und auf Ergebnisse warten, um komplexe Aufgaben zu lösen.

### 3. Erweiterbarkeit
Die Plattform ist so konzipiert, dass sie über Community-Standards erweitert werden kann.
- **Erweiterungen (MCP)**: Volle Unterstützung für das Model Context Protocol. Verbinden Sie sich sofort mit jedem MCP-Server.
- **Ein-Klick-Presets**: Kuratierter Katalog für GitHub, Brave Search usw., direkt in der UI verfügbar.
- **Skills & Playbooks**: Wiederverwendbare Verhaltens-Snippets und strukturierte Workflow-Vorlagen.

### 4. Autonomie & Planung
- **YOLO-Modus**: Optionale autonome Ausführung für sensible Tools ohne manuelle Genehmigung.
- **Geplante Aufgaben**: Cron-basierte Automatisierung mit Workspace-Targeting und automatischer Wiederherstellung nach Neustarts.

### 5. Kontext & Metriken
- **@mentions**: Direkte Injektion von Dateien, Skills oder Playbooks in den Chat.
- **Multimodal**: Verarbeitet Bilder und Audio für OpenAI-, Anthropic- und Gemini-Modelle.
- **Beobachtbarkeit**: Echtzeit-TPS-Metriken und Prompt-Caching-Hits (für Anthropic/Gemini).

---

## 📦 Installation

Laden Sie die neuesten Binärdateien für Windows, macOS oder Linux von der [Release-Seite](https://github.com/fritzprix/libr-agent/releases/latest) herunter.

**Vom Quellcode bauen:**
```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## Design-Entscheidungen

- **Local First**: Ihre Daten und API-Schlüssel bleiben auf Ihrem Rechner.
- **Tauri + Rust**: Gewählt für Sicherheit (Speichersicherheit), Performance und geringe Binärgröße.
- **SQLite (SeaORM)**: Wird für eine robuste, lokale Persistenz von Sitzungen und Konfigurationen verwendet.

---

## Mitwirken & Lizenz

Beiträge sind willkommen. Bitte lesen Sie `CONTRIBUTING.md`.

**Lizenz**: MIT
