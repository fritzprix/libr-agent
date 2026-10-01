# 🤖 LibrAgent

> **Eine Agenten-Betriebsumgebung, die Sie selbst betreiben — Modell wählen, Tools per Ein-Klick, Koordinationsmuster wählen.**
> _Kein Vendor-Harness. Keine JSON-Hausaufgaben. Arbeit endet als Dateien auf Ihrer Maschine._

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

![Hero-Demo — Tools wie Apps installieren. Modell behalten. Datei behalten.](./assets/hero-demo-60s.gif)

[HD WebM](./assets/hero-demo-60s.webm) · _Tools wie Apps installieren. Modell behalten. Datei behalten._

---

## Was LibrAgent anders macht

Die meisten Agenten-Harnesses setzen voraus, dass Sie MCP-JSON bearbeiten, im Terminal leben und Orchestrierung im Code zusammensetzen (oder sperren Sie in den Stack eines einzelnen Vendors).

LibrAgent ist ein **Desktop-Produkt** für dieselbe Aufgabe:

| Statt… | Erhalten Sie… |
| ------ | ------------- |
| MCP-Configs von Hand editieren | **Extensions** — Ein-Klick-Presets (GitHub, Brave Search, Filesystem, …) und Import aus Cursor / VS Code / Claude Code / Windsurf |
| „Wir haben Multi-Agent“ | **Benannte Koordinationsmuster** als gebündelte Skills — `pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, … |
| Modell + Tools eines Anbieters | **Ihr** LLM (API-Schlüssel oder [Ollama](https://ollama.com)) und **Ihr** MCP-Stack — MIT, local-first |

[Neueste Version herunterladen](https://github.com/fritzprix/libr-agent/releases/latest) · [5-Minuten-Onboarding](#5-minuten-onboarding) · [Hero-Demo-Spezifikation](docs/contributing/hero-demo-spec.md)

---

## Was Sie in den ersten 10 Minuten tun können

### 1. Ein-Klick-Tools, dann ein Deliverable

- Öffnen Sie **Extensions** und installieren Sie ein Preset (z. B. GitHub) — kein JSON
- Zeigen Sie **Workspace** auf einen echten Ordner
- Fragen Sie: _"Prüfe dieses Repo auf das größte Risiko für neue Beiträger und speichere `DELIVERABLE.md`"_

### 2. Ein Ein-Klick-Workflow-Rezept deployen

- Starten Sie das **Morning Briefing**-Rezept von der Chat-Startseite oder [Scheduled Tasks](docs/user/guides/scheduled-tasks.md)
- Installiert Hacker-News- + Yahoo-Finance-Presets, konfiguriert einen Assistenten und plant einen täglichen Lauf um 9 Uhr
- Aufwachen mit einem synthetisierten Tech- & Markt-Briefing — unbeaufsichtigt

### 3. Ein Koordinationsmuster wählen (ohne Framework-Zusammenbau)

- Beschreiben Sie die Arbeitsform oder hängen Sie einen Skill per Namen an:
  - _"@skill:pipeline — recherchieren, dann entwerfen, dann reviewen; einen finalen Bericht lassen"_
  - _"@skill:divide-conquer — in unabhängige Teile aufteilen und Ergebnisse zusammenführen"_
- Muster sind produktisierte Skills — kein SDK, das Sie selbst verdrahten. Siehe [Sub-agents & orchestration](docs/user/guides/sub-agents.md).

### 4. Modelfreiheit behalten

- Cloud: OpenAI- / Anthropic- / Gemini- / Groq-API-Schlüssel einfügen
- Lokal: `ollama pull qwen3:14b` und Ollama wählen — dasselbe Harness in beiden Fällen

---

## Drei Produktversprechen

1. **Oberfläche ohne Harness-Hausaufgaben** — GUI, Extensions Ein-Klick, Rezepte, In-App-Freigaben, `@skill:` — nicht „öffne zuerst Config und Shell“.
2. **Orchestrierung als Produkt** — Sequential- / Hub-and-spoke- / Swarm-ähnliche Flows per Skills wählen; bei Bedarf zu `teamwork` → `org` und `schedule` wachsen — weiterhin ohne LangGraph/CrewAI selbst zu bauen.
3. **Anbieter- & Stack-Freiheit** — jedes unterstützte LLM, MCP als Infrastruktur, Import bestehender IDE-MCP-Configs, MIT-Lizenz, lokale Workspaces und Browser-Zustand standardmäßig.

**Beste Passung:** Operatoren und Power-User, die Harness-Tiefe ohne JSON-Leben wollen; Entwickler, die einen Single-Vendor-Agent-Stack ablehnen; Forscher, die Browser + Wissen + Zeitpläne in einem Produkt brauchen.

---

## Koordinationsmuster (gebündelte Skills)

Wählen Sie das Modell nach der **Form der Arbeit**, dann starten Sie es im Chat:

| Skill | Muster | Wann nutzen |
| ----- | ------ | ----------- |
| `pipeline` | Sequentielle Stufen | Ausgaben speisen den nächsten Schritt (Recherche → Entwurf → Review) |
| `hub-spoke` | Hub-and-spoke | Ein Koordinator integriert viele Worker |
| `divide-conquer` | Parallele Teilung | Unabhängige Teile, dann Merge |
| `consensus-delegation` | Mehrperspektivisch | Dieselbe Frage an mehrere Spezialisten, dann abstimmen |
| `gatekeeper` / `pair-programming` | Review-Schleifen | Strenges Review oder Zwei-Agenten-Coding |
| `delegate` | Leichter Handoff | Eine Kind-Session, Lineage getrackt |
| `teamwork` → `org` | Dauerhaftes Team | Geteilte Verfassung + Org UI |
| `schedule` / `loop` / `call-me-back` | Zeit & Ereignisse | Cron, In-Session-Verzögerungen oder Fortsetzen per Prozess/Webhook |

Auswahlheuristiken: [framework-selection](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · voller Guide: [Sub-agents](docs/user/guides/sub-agents.md).

Weitere Day-one-Skills: `setup-wizard`, `tool-installer`, `playbook-creator` und mehr — **[Bundled Skills](docs/user/guides/skills.md)**.

---

## MCP-Plattform (weiterhin power-user-fähig)

- Transports: stdio, HTTP, SSE, OAuth 2.1
- 15+ eingebaute Server (Workspace, Shell, Browser, Knowledge, Planning, Scheduled Tasks, …)
- Ein-Klick-Presets + agentengestützte Installation (`tool-installer`)
- Tool-Isolation pro Session; Pfad-/Befehls-Guards; optionale YOLO- / unsafe-Modi für Automatisierung

### Ausführungssubstrat

| Substrat | Fähigkeiten |
| -------- | ----------- |
| **Workspace** | Zeilengenaues Editieren, Multi-Datei-Ops, `@file`- / `@skill`- / `@playbook`-Kontext |
| **Shell** | Isolierte und persistente Shells mit async Prozessüberwachung |
| **Browser** | Isolierter Browser-Sidecar; optionale gespeicherte Login-Profile |
| **Knowledge** | Graph-Wissen + BM25-Suche |
| **Export** | Markdown-Berichte und ATIF-Trajectory-Exports ([session export](docs/user/guides/session-export.md)) |

Lange Sessions bleiben produktiv durch Kontext-Kompression, Loop-Prävention, Circuit Breaker und Stale-Response-Guards.

---

## Szenarien aus der Praxis

### Operator — von leerer App zum täglichen Briefing

1. **Morning Briefing**-Rezept starten (Presets + Assistent + 9-Uhr-Plan)
2. Einmal **Run now** klicken zum Prüfen
3. Laufen lassen — der Bericht landet ohne Terminal

### Solo-Entwickler — Preset, keine Config-Dateien

1. Extensions → GitHub-MCP-Preset installieren
2. Lokales Repo über Workspace anbinden
3. Markdown-Sicherheits-/Review-Bericht verlangen, den Sie auf Disk behalten

### Power-User ohne Framework — benannte Orchestrierung

1. `@skill:pipeline` (oder `hub-spoke` / `divide-conquer`) für die Arbeitsform
2. Agenten koordinieren unter diesem Muster
3. Ein gemergtes Deliverable im Workspace — keine Orchestrierungsbibliothek zu pflegen

### Datenschutzsensibles Team — gleiches Produkt, lokales Modell

1. `ollama pull qwen3:14b`
2. Workspace + Shell bleiben auf der Maschine
3. Später Cloud-Keys tauschen, wenn gewünscht — das Harness wechselt den Vendor nicht für Sie

---

## Dokumentation

- **[User Guide](docs/user/README.md)** — Installation, erster Chat, Modelle, Skills ([docs site](https://fritzprix.github.io/libr-agent/))
- **[Hero Demo Spec](docs/contributing/hero-demo-spec.md)** — kanonische Produktdemo (EN/KO/ZH-Untertitel)
- **[Product Messaging Guide](docs/contributing/product-messaging-guide.md)** — Positionierung und Copy
- **[Recipes](docs/user/guides/recipes.md)** · **[Scheduled Tasks](docs/user/guides/scheduled-tasks.md)** · **[Sub-agents](docs/user/guides/sub-agents.md)** · **[Skills](docs/user/guides/skills.md)**
- **[HTTP API](docs/api/http_api.md)** — Fernsteuerung und programmatische Freigaben
- **[Architecture](docs/architecture/agent-workflow-architecture.md)** — Session-Isolation und Think-Act-Observe

---

## Erste Schritte

Laden Sie den neuesten Installer von der **[Releases-Seite](https://github.com/fritzprix/libr-agent/releases/latest)** herunter.

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.21_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_x64-setup.exe) · [`LibrAgent_0.9.21_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.21_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.21_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_amd64.AppImage) · [`LibrAgent_0.9.21_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_amd64.deb) · [`LibrAgent-0.9.21-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent-0.9.21-1.x86_64.rpm)
- **All release assets:** [Releases page](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.21)
<!-- RELEASE_DOWNLOADS_END -->

### 5-Minuten-Onboarding

**Schritt 1 — Modell verbinden** (Settings → LLM Providers)

- Cloud: OpenAI- / Anthropic- / Gemini- / Groq-API-Schlüssel einfügen
- Lokal: `ollama pull qwen3:14b`, dann Ollama in Settings wählen

**Schritt 2 — Tools ohne JSON hinzufügen**

- Extensions → Preset installieren (z. B. GitHub), **oder**
- Einem Agenten sagen: _"Importiere meine MCP-Server aus Cursor"_

**Schritt 3 — Workspace anbinden und eine Datei verlangen, die Sie behalten**

- Workspace auf einen echten Projektordner zeigen
- _"Reviewe diesen Workspace und schreibe Findings nach `DELIVERABLE.md`."_

**Als Nächstes — Koordination und Automatisierung**

- _"@skill:pipeline — recherchieren, entwerfen, dann reviewen; ein finaler Bericht."_
- _"Bereite einen teamwork-Workspace für dieses Repo vor."_
- _"Richte ein tägliches Competitor-Brief um 7 Uhr ein."_ (oder Morning-Briefing-Rezept)

### Erste Prompts zum Kopieren

- _"Importiere meine MCP-Server aus Cursor und zeige, was hinzugefügt wurde."_
- _"Installiere das GitHub-MCP-Preset und hänge es an einen Coding-Agenten."_
- _"Reviewe diesen Workspace und schreibe Findings nach `DELIVERABLE.md`."_
- _"@skill:pipeline — recherchiere dieses Thema, entwirf eine Zusammenfassung, dann review; speichere den finalen Bericht."_
- _"Richte ein tägliches Competitor-Brief um 7 Uhr ein."_

### Entwickler-Setup

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## Wo LibrAgent am besten passt

| Wenn Sie wollen… | LibrAgent ist stark, weil… |
| ---------------- | -------------------------- |
| **Harness-Tiefe ohne Harness-Hausaufgaben** | Extensions-Presets, Rezepte, `@skill:`-Muster und Freigaben — kein JSON-first-Onboarding |
| **Orchestrierung ohne Framework-Bau** | `pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, `teamwork` / `org`, `schedule` kommen als Produkt |
| **Freiheit von einem Vendor-Agent-Stack** | Bringen Sie Modell und MCP-Tools mit; MIT; standardmäßig local-first |
| **Ein echtes Ausführungssubstrat** | Workspace, Shell, Browser, Knowledge, Playbooks und Guards für lange Sessions |
| **Ein MCP-natives Desktop-Produkt** | Presets, Import und 15+ Builtins — kein dünner Chat-Wrapper |

---

## Designphilosophie

- **Produkt statt Kit**: Das Harness ist nutzbar, ohne es zusammenzubauen.
- **Orchestrierung als Skills**: Koordinationsmuster sind benannt, wählbar und dokumentiert — nicht in Sample-Repos vergraben.
- **Stack-Freiheit**: Modelle und Tools sind Nutzerentscheidungen; das Produkt verlangt keinen einzelnen KI-Vendor.
- **Local First**: Workspaces, Sessions, Skills und Browser-Zustand bleiben unter Ihrer Kontrolle. Cloud-LLM / Remote-MCP nur bei Opt-in.
- **Harness vor Modell**: Tools, Session-State, Delegation und Governance zählen mehr als jedes einzelne Modell.
- **Stabilität vor Features**: Isolation, Kompression, Loop-Prävention — vor dem Feature-Jagd.
- **Offene Standards**: MIT. MCP als Interoperabilitäts-Schicht.

---

## Mitwirken & Lizenz

LibrAgent steht unter MIT und wird offen gebaut. Beiträge sind willkommen — gebündelte Skills, MCP-Integrationen, Bugfixes oder Architekturverbesserungen.

- 📖 [Contributing Guide](CONTRIBUTING.md)
- 🐛 [Issue Tracker](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- 💬 [Discussions](https://github.com/fritzprix/libr-agent/discussions)
- 🧪 Benchmarks (Harbor / Terminal-Bench): siehe [Harbor guide](benchmarks/harbor/README.md) (`pnpm bench:diverse`, `pnpm bench:terminal`, …)

**License**: MIT
