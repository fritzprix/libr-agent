# 🤖 LibrAgent

> **Der Agenten-Harness für das Zeitalter der autonomen Intelligenz.**
> _Nicht nur eine Chat-App. Ein Ausführungssubstrat, in dem Agenten arbeiten, zusammenarbeiten und skalieren._

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent ist ein **local-first Agenten-Betriebssystem**, gebaut auf Tauri + Rust + React. Es geht weit über Chat-Interfaces hinaus — es bietet ein sicheres Ausführungssubstrat, ein nativ MCP-basiertes Tool-Ökosystem und eine rekursive Delegationsarchitektur, die einen einzelnen Agenten zu einem koordinierten Schwarm skaliert.

Verbinde jeden LLM (Cloud oder lokal via Ollama), erweitere mit jedem MCP-Server, und lass Agenten echte Arbeit erledigen: Dateien bearbeiten, Shells ausführen, im Web navigieren, Wissen verwalten — autonom, so lange wie nötig.

---

## Warum LibrAgent?

Der Fokus der KI-Branche hat sich verschoben. In der Praxis kann **dasselbe Modell je nach Harness deutliche Unterschiede bei der Aufgabenerfolgsrate** zeigen. Das Modell ist der Motor — aber der Harness bestimmt, wie weit er kommt.

Jede aktuelle Option erzwingt noch immer einen Kompromiss:

| Plattform                | Das Problem                                                                                                                                                              |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **OpenClaw**             | Sehr flexibles offenes Ökosystem, aber Analysen Anfang 2026 berichteten von exponierten Instanzen, Secrets im Klartext und Prompt-Injection-Risiken in Community-Skills. |
| **Claude Cowork**        | Solide lokale UX, aber noch limitiert bei komplexen autonomen Aufgaben. Geschlossenes Ökosystem. Nicht erweiterbar.                                                      |
| **Claude Code / Cursor** | Nur für Entwickler. Erfordert Terminal-Kenntnisse. Kein Allzweck-Tool.                                                                                                   |
| **Google Mariner**       | Deine Arbeit läuft auf Googles Cloud-VMs. Du kontrollierst deine Daten nicht.                                                                                            |
| **LangGraph / CrewAI**   | Leistungsstarke Frameworks, aber du musst alles selbst zusammenbauen. Kein Produkterlebnis.                                                                              |

**LibrAgent ist gebaut, um diesen Kompromiss aufzuheben.** Local-first Sicherheit. Native MCP-Erweiterbarkeit. Multi-Agenten-Koordination Schwarm→Organisation. Eine ausgefeilte GUI, die auch für Nicht-Entwickler funktioniert. Alles in einer Open-Source-Desktop-App.

### Für wen LibrAgent gedacht ist

- **Solo-Entwickler**, die Agenten wollen, die wirklich lokal lesen, bearbeiten, ausführen, navigieren und Kontext persistieren können
- **Power-User und Operatoren**, die ihren eigenen Stack aus lokalen Modellen, API-Anbietern, MCP-Servern und geplanten Workflows zusammenstellen wollen
- **Forscher und Analysten**, die Browser-Automatisierung, Wissenserfassung, wiederholbare Playbooks und Langzeitsitzungen benötigen
- **Datenschutzbewusste Teams**, die lokale Ausführung, explizite Governance und einen Weg von einem einzelnen Agenten zu einer koordinierten Organisation wollen

---

## 🎬 Die Plattform in Aktion

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

_Von einem einzelnen Agenten zu einem koordinierten Schwarm — rekursive Delegation, MCP-Tools und persistenter Arbeitsbereich in einem einheitlichen Substrat._

---

## Kernpfeiler

### 1. 🔐 Local-First Sicherheit — Deine Daten bleiben auf deiner Maschine

LibrAgent behandelt Sicherheit als erstklassiges architektonisches Anliegen:

- **Sitzungsisolation**: Jede Agentensitzung erhält ihre eigene dedizierte `MCPServiceProxy`-Instanz — null sitzungsübergreifende Datenlecks
- **Eingebauter SecurityValidator**: Pfadtraversal- und Befehlsinjektionsangriffe auf Systemebene blockiert
- **Kein Cloud-Substrat erforderlich**: Die Kernausführung erfolgt lokal; externer Netzwerkverkehr beschränkt sich auf die LLM-Anbieter und entfernten MCP/HTTP-Dienste, die du bewusst nutzt
- **Vollständiger Offline-Support**: Kombiniere mit [Ollama](https://ollama.ai) für einen vollständig isolierten Agenten-Stack

#### Was lokal bleibt vs. was deine Maschine verlässt

- **Immer lokal**: Arbeitsbereiche, lokale Dateien, gebündelte Skills, Sitzungsstatus, MCP-Server-Configs, Browser-Status und lokale Tool-Ausführung
- **Verlässt deine Maschine nur wenn du es wählst**: Anfragen an Cloud-LLM-Anbieter oder entfernte MCP/HTTP-Dienste, die du explizit konfigurierst
- **Vollständiger Offline-Modus**: Nutze Ollama oder eine andere lokale Runtime mit lokalen MCP-Servern für einen isolierten Workflow

### 2. 🧩 Natives MCP-Ökosystem — Unendliche Erweiterbarkeit by Design

MCP (Model Context Protocol) ist der offene Standard hinter dem Erweiterbarkeitsmodell von LibrAgent. LibrAgent behandelt es nicht als Feature — sondern als architektonisches Rückgrat:

- **Vollständige Transport-Unterstützung**: stdio, HTTP, SSE und OAuth 2.1 — die vollständige Spezifikation
- **12+ eingebaute Server**: Planning, Knowledge (RAG), Browser Automation, Workspace, Shell Execution, Content Store und mehr
- **Preset-Katalog**: Installiere GitHub, Brave Search, Filesystem und andere beliebte Server mit einem Klick
- **Pro-Sitzung isolierte Instanzen**: Jede Agentensitzung hat unabhängigen MCP-Server-Status — keine Interferenz zwischen parallelen Agenten
- **Von überall importieren**: Migriere MCP-Configs automatisch von Cursor, VS Code, Claude Code oder Windsurf

### 3. 🦾 Produktionsreifes Ausführungssubstrat

Die meisten KI-Tools sind in Demos beeindruckend und in der Produktion fragil. LibrAgent ist obsessiv für echte, dauerhafte Arbeit konzipiert:

| Substrat      | Fähigkeiten                                                                                                           |
| ------------- | --------------------------------------------------------------------------------------------------------------------- |
| **Workspace** | Zeilenpräzise Bearbeitung, Multi-Datei-Operationen, einheitliche Suche, `@file`/`@skill`/`@playbook`-Kontextinjektion |
| **Shell**     | Isolierte Ausführung UND persistente Shells — asynchrones Prozess-Monitoring (`poll`, `read output`, `list`)          |
| **Browser**   | Headless-Browser-Automatisierung mit Playwright-ähnlichem Interaktionsmodell und Cache-Konsistenzgarantien |
| **Knowledge** | Graphbasiertes Wissensmanagement mit Entitäts-/Beziehungsextraktion (v2), BM25-Volltextsuche                          |

**Zuverlässigkeitsingenieurwesen inklusive**: Kontextkomprimierung, Schleifenprävention, Schaltkreisunterbrecher und Schutz vor veralteten Antworten halten Agenten in stundenlangen Sitzungen produktiv.

### 4. 🤝 Schwarm → Team → Organisation: Multi-Agent in jeder Größenordnung

LibrAgent hat eine kohärente Multi-Agenten-Geschichte von der Solo-Ausführung bis zur expliziten Organisationskoordination:

- **`delegate`**: Elternagenten erzeugen, informieren und überwachen Kindersitzungen mit expliziter Abstammungsverfolgung
- **`teamwork`**: Baut einen vollständigen Task-Force-Arbeitsbereich (agents.md, MISSION.md, KANBAN.md) mit einem einzigen Befehl
- **`org`**: Formalisiert Teams mit dauerhafter Organisationsidentität, Root-Sitzungswiederherstellung und sichtbarer Mitgliederhierarchie
- **`schedule`**: CRON-basierte Automatisierung — Agenten laufen unbeaufsichtigt, nach Zeitplan, mit Arbeitsbereichsbereitstellung
- **Concurrency Gate**: Strenge Limits für parallele Sitzungen und Shell-Prozesse zur Vermeidung von Deadlocks und unkontrollierten Kosten

### 5. ⚡ Gebündelte Skills — Der schnellste Weg von einer leeren Installation zu einem betriebsbereiten Schwarm

LibrAgent wird mit einer wachsenden Bibliothek **gebündelter Skills** geliefert. Das sind keine zufälligen Prompts — das sind wiederverwendbare operative Verfahren, die jeder Agent namentlich aufrufen kann.

Die wichtigsten Skills für den ersten Tag:

| Skill                | Was er tut                                                                                            |
| -------------------- | ----------------------------------------------------------------------------------------------------- |
| `system-setup`       | Erkennt und installiert fehlende Runtimes (Python, Node.js, uv) plattformübergreifend                 |
| `mcp-installer`      | Registriert MCP-Server aus npm-Paketen, GitHub-URLs oder JSON-Config-Blöcken                          |
| `mcp-importer`       | Importiert bestehende MCP-Configs aus Cursor, VS Code, Windsurf u.ä.                                  |
| `specialist-creator` | Entwirft eine vollständige Agenten-Config (System-Prompt, Modell, Tools) aus einer Rollenbeschreibung |
| `crew-constructor`   | Scannt verfügbare Tools und erstellt automatisch ein passendes Spezialistenteam                       |
| `agent-tooling`      | Prüft Agenten, erkennt Fähigkeitsmismatch und balanciert Tool-Zuweisungen dynamisch neu               |
| `delegate`           | Führt durch die Eltern→Kind-Sitzungsübergabe mit explizitem Kontexttransfer und Abstammungsverfolgung |
| `teamwork`           | Baut die geteilte Arbeitsbereichsverfassung für koordinierte Multi-Agenten-Arbeit                     |
| `org`                | Formalisiert dauerhafte Organisationsidentität und sichtbare Mitgliederhierarchie                     |
| `schedule`           | Erstellt und verwaltet wiederkehrende geplante Task-Gruppen für unbeaufsichtigte Automatisierung      |
| `soul-awakening`     | Verankert einen Agenten an eine `SOUL.md`-Persona — Ton, Haltung, Identität                           |

Das ist nur die Operatorschicht. LibrAgent bietet auch Domain-Skills für:

- **Wissen & Recherche**: `deep-research-report`, `knowledge-distiller`
- **Dokumenten-Workflows**: `document-to-markdown`, `docx`, `pptx`
- **Skill- und Workflow-Erstellung**: `skill-creator`, `skill-deployer`, `playbook-creator`, `mcp-builder`
- **Spezialisierte Operationen**: `computer-diagnosis` und andere spezialisierte Assistenten

_Wichtig: `bootstrap` ist eine eingebaute Fähigkeit, die häufig zusammen mit diesen Skills verwendet wird. Gebündelte Skills sind die wiederverwendbaren Verfahren; die eingebauten und MCP-Tools sind das zugrundeliegende Ausführungssubstrat._

---

## 🌍 Reale Szenarien

### Solo-Entwickler — Automatisiertes Code-Review

1. Verbinde dein lokales Repository über das Workspace-Tool
2. Installiere das GitHub MCP-Preset (ein Klick)
3. Frage: _"Finde Sicherheitsprobleme in PR #42 und erstelle einen Markdown-Bericht"_
4. Der Agent liest den Code, führt die Analyse durch, speichert die Ergebnisse im Knowledge-Server

### Marketingmitarbeiter — Wettbewerbsintelligenz auf Autopilot

1. Konfiguriere 5 Konkurrenz-Blogs über das Browser-Tool
2. Sage einem Agenten: _"Erstelle jeden Morgen um 7 Uhr ein geplantes Wettbewerbs-Briefing"_ — der Agent kann den `schedule`-Skill nutzen, um die wiederkehrende Task-Gruppe einzurichten
3. Der Agent navigiert, fasst zusammen und fügt dem Knowledge Store hinzu
4. Frage jederzeit: _"Fasse die Wettbewerbsbewegungen der letzten Woche zusammen"_

### Engineering-Team — Offline-Agenten-Stack

1. `ollama pull qwen3:14b` — keine API-Schlüssel, keine Cloud
2. Verbinde Workspace + Shell-Tools mit deiner Codebasis
3. Sensibles geistiges Eigentum verlässt die Maschine nie
4. Agenten lesen, modifizieren, testen und committen — vollständig lokal

### Power-User — Multi-Agenten-Recherche-Pipeline

1. Nutze `crew-constructor`, um automatisch zu generieren: Researcher×3, Analyst×1, Writer×1
2. Der Orchestrator delegiert parallel über den `delegate`-Skill
3. Ergebnisse werden zu einem einzigen strukturierten Bericht im Content Store zusammengeführt
4. Plane den gesamten Workflow wöchentlich via `schedule`

---

## 📖 Dokumentation und Leitfäden

- **[Navigationsleitfaden](docs/guides/navigation-guide.md)**: Das Command & Control-Hub — `/assistants` (Rollendefinitionen) und `/playbooks` (Workflow-Blueprints).
- **[Architekturleitfaden](docs/architecture/agent-workflow-architecture.md)**: Sitzungsisolation, Orchestrierungs-Engine und die Rust-gesteuerte Think-Act-Observe-Schleife.
- **[Leitfaden für eingebaute Tools](docs/guides/builtin_tool_bp.md)**: Tool-Design-Standards und MCP-Antwortmuster.

---

## 📦 Erste Schritte

Lade das neueste Installationsprogramm für deine Plattform von der **[Releases-Seite](https://github.com/fritzprix/libr-agent/releases/latest)** herunter.

```
Windows  →  LibrAgent_x.x.x_x64-setup.exe
macOS    →  LibrAgent_x.x.x_aarch64.dmg
Linux    →  libragent_x.x.x_amd64.AppImage
```

**Entwickler-Setup:**

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

### Der 5-Minuten-Onboarding-Pfad

**Schritt 1 — Verbinde ein Modell** (Settings → LLM Providers)

- Cloud: Füge einen OpenAI / Anthropic / Gemini / Groq API-Schlüssel ein
- Lokal: `ollama pull qwen3:14b` und dann Ollama in den Settings auswählen
- Nutzt du bereits Cursor oder VS Code? Sage einem beliebigen Agenten: _"Importiere meine MCP-Server aus Cursor"_ → `mcp-importer` kümmert sich darum

**Schritt 2 — MCP-Tools hinzufügen** (Extensions-Seitenleiste)

- Durchsuche den Preset-Katalog und klicke auf Installieren, oder
- Sage einem Agenten: _"Install @modelcontextprotocol/server-everything"_ → `mcp-installer` registriert es automatisch

**Schritt 3 — Erstelle deinen ersten Agenten**

- _"Erstelle einen Recherche-Agenten für Wettbewerbsintelligenz"_ → `specialist-creator` entwirft die vollständige Config
- _"Baue ein Recherche-Team mit meinen aktuellen Tools"_ → `crew-constructor` erstellt Spezialisten im Batch
- _"Optimiere Tool-Zuweisungen für alle meine Agenten"_ → `agent-tooling` prüft und balanciert automatisch neu

**Schritt 4 — Parallelisiere mit `delegate`**

- Bitte jeden Agenten, Teilaufgaben an Kind-Sitzungen zu delegieren
- Der `delegate`-Skill übernimmt Kontextübergabe, Abstammungsverfolgung und Ergebniszusammenführung

**Schritt 5 — Baue ein persistentes Team**

- `teamwork` → baut den geteilten Arbeitsbereich mit `agents.md`, `MISSION.md`, `KANBAN.md`
- `org` → formalisiert das Team mit dauerhafter Identität und Root-Sitzungsverwaltung
- `schedule` → lass einen Agenten die CRON-Automatisierung für dich erstellen und verwalten, unbeaufsichtigt

### Erste Prompts zum Kopieren und Einfügen

- _"Importiere meine MCP-Server aus Cursor und zeige mir, was hinzugefügt wurde."_
- _"Erstelle einen Recherche-Agenten für Wettbewerbsintelligenz mit meinen aktuellen Tools."_
- _"Installiere das GitHub MCP-Preset und verknüpfe es mit einem Coding-Agenten."_
- _"Delegiere Repository-Analyse an eine Kind-Sitzung und bringe mir eine Zusammenfassung."_
- _"Bereite einen Teamwork-Arbeitsbereich für dieses Repository vor und erstelle ein Spezialistenteam, das für Org bereit ist."_
- _"Richte ein tägliches Wettbewerbs-Briefing um 7 Uhr ein und halte alles im geteilten Teamwork-Arbeitsbereich."_

---

## LibrAgent im Vergleich

```
                    Privacy/Local  MCP Ecosystem  Non-Dev UX  Multi-Agent  Open Source
LibrAgent              ★★★★★          ★★★★★         ★★★★☆       ★★★★★           ✅
OpenClaw               ★★☆☆☆          ★★★★☆         ★★★☆☆       ★★★☆☆           ✅
Claude Cowork          ★★★★☆          ★★☆☆☆         ★★★★★       ★★☆☆☆           ❌
Claude Code            ★★★★☆          ★★★☆☆         ★☆☆☆☆       ★★★☆☆           ❌
Google Mariner         ★★☆☆☆          ★★★☆☆         ★★★★☆       ★★★★☆           ❌
LangGraph / CrewAI     ★★★☆☆          ★★★☆☆         ★★☆☆☆       ★★★☆☆           ✅
```

---

## Designphilosophie

- **Local First**: Deine Daten, Schlüssel und Agenten-"Seelen" bleiben unter deiner alleinigen Kontrolle. Kein Cloud-Substrat erforderlich.
- **Harness über Modell**: Die Ausführungsumgebung — Tools, Sitzungsstatus, Delegation, Governance — ist wichtiger als jedes einzelne Modell. LibrAgent ist darauf ausgelegt, das Potenzial jedes Modells zu maximieren.
- **Stabilität über Features**: Das CHANGELOG spiegelt einen obsessiven Fokus auf Runtime-Korrektheit wider — Sitzungsisolation, Komprimierung, Schleifenprävention, Schutz vor veralteten Antworten.
- **MCP als Infrastruktur**: Kein Plugin-System. Das gesamte Tool-Ökosystem ist um MCP als primäre Interoperabilitätsschicht organisiert.
- **Offene Standards**: MIT-Lizenz. Vollständig engagiert für MCP, Open-Source-Interoperabilität und Datensouveränität der Nutzer.

---

## Beitragen & Lizenz

LibrAgent ist MIT-lizenziert und wird offen entwickelt. Beiträge sind willkommen — ob neue gebündelte Skills, MCP-Integrationen, Bug-Fixes oder architektonische Verbesserungen.

- 📖 [Beitragsleitfaden](CONTRIBUTING.md)
- 🐛 [Issue-Tracker](https://github.com/fritzprix/libr-agent/issues)
- 💬 [Diskussionen](https://github.com/fritzprix/libr-agent/discussions)

**Lizenz**: MIT
