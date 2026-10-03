<p align="center">
  <img src="./src-tauri/icons/128x128.png" width="96" alt="LibrAgent" />
</p>

# LibrAgent

> **Eine lokal ausgeführte Desktop-Umgebung für KI-Agenten unter Ihrer vollständigen Kontrolle.**
> Verbinden Sie beliebige Sprachmodelle, installieren Sie Werkzeuge per Klick und automatisieren Sie Abläufe mit erprobten Multi-Agenten-Mustern.
> _Ohne Bindung an einzelne Cloud-Anbieter, ohne manuelle JSON-Konfiguration. Alle Arbeitsergebnisse verbleiben sicher als lokale Dateien auf Ihrem Rechner._

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

![Hero-Demo — Werkzeuge wie Apps installieren, Modelle frei wählen, Ergebnisse lokal behalten.](./assets/hero-demo-60s.gif)

[HD-WebM-Video ansehen](./assets/hero-demo-60s.webm) · _Werkzeuge wie Apps installieren, Modelle frei wählen, Ergebnisse lokal behalten._

---

## Was LibrAgent auszeichnet

Viele Agenten-Frameworks setzen voraus, dass Benutzer komplexe Terminalbefehle beherrschen, JSON-Dateien manuell editieren und Multi-Agenten-Orchestrierungen im Code selbst zusammenbauen. Zudem binden zahlreiche Lösungen den Anwender an ein geschlossenes Anbieter-Ökosystem.

LibrAgent löst diese Herausforderungen als **vollwertige, gebrauchsfertige Desktop-Anwendung**:

| Herkömmlicher Ansatz | Mit LibrAgent |
| -------------------- | ------------- |
| Manuelle Pflege komplexer MCP-JSON-Konfigurationen | **Ein-Klick-Erweiterungen** — Vorgefertigte Presets (GitHub, Brave Search, Dateisystem) und nahtloser Import aus Cursor, VS Code und Claude Code |
| Aufwendiges Selbstprogrammieren von Multi-Agenten | **Integrierte Koordinationsmuster** — Erprobte Standard-Skills wie `pipeline`, `hub-spoke`, `divide-conquer` und `consensus-delegation` |
| Feste Bindung an proprietäre Modelle und Tools | **Volle Technologiefreiheit** — Beliebige Cloud-APIs oder lokale Modelle ([Ollama](https://ollama.com)) unter einer quelloffenen MIT-Lizenz nutzen |

[Neueste Version herunterladen](https://github.com/fritzprix/libr-agent/releases/latest) · [5-Minuten-Schnellstart](#5-minuten-schnellstart) · [Demo-Spezifikation](docs/contributing/hero-demo-spec.md)

---

## In 10 Minuten startklar

### 1. Werkzeuge mit einem Klick installieren und Ergebnisse erzeugen

- Öffnen Sie **Extensions** und installieren Sie ein Preset (z. B. GitHub) ohne manuelle JSON-Anpassungen.
- Verknüpfen Sie Ihren **Workspace** mit einem lokalen Projektordner.
- Geben Sie die Anweisung: _"Analysiere dieses Repository hinsichtlich der größten Risiken für neue Mitwirkende und speichere den Bericht als `DELIVERABLE.md`"_

### 2. Automatisierte Workflow-Rezepte ausführen

- Starten Sie das Rezept **Morning Briefing** über die Chat-Startseite oder die [Geplante Aufgaben](docs/user/guides/scheduled-tasks.md).
- Werkzeuge für Hacker News und Yahoo Finance werden automatisch verknüpft und für die tägliche Ausführung um 09:00 Uhr eingeplant.
- Erhalten Sie jeden Morgen automatisch eine aktuelle Technologie- und Marktübersicht.

### 3. Koordinationsmuster ohne Programmierung auswählen

- Definieren Sie das gewünschte Arbeitsmuster direkt im Prompt:
  - _"@skill:pipeline — Recherche, Entwurf und Prüfung nacheinander ausführen; erstelle einen konsolidierten Abschlussbericht"_
  - _"@skill:divide-conquer — Teile diese Aufgabe in unabhängige Teilaufgaben auf, führe sie parallel aus und führe die Ergebnisse zusammen"_
- Bewährte Koordinationsmuster stehen direkt als fertige Skills bereit. Weitere Informationen finden Sie unter [Sub-Agenten und Orchestrierung](docs/user/guides/sub-agents.md).

### 4. Freie Modellwahl behalten

- **Cloud-Modelle**: Fügen Sie einfach API-Schlüssel für OpenAI, Anthropic, Gemini oder Groq ein.
- **Lokale Modelle**: Führen Sie `ollama pull qwen3:14b` aus und wählen Sie Ollama in den Einstellungen für einen rein lokalen Betrieb.

---

## Drei zentrale Produktversprechen

1. **Gebrauchsfertige Benutzeroberfläche ohne Einrichtungsaufwand** — Grafische Oberfläche, Ein-Klick-Erweiterungen, automatisierte Rezepte und integrierte Freigabeprozesse machen Terminalarbeit und Konfigurationsdateien überflüssig.
2. **Orchestrierung als integrierte Funktion** — Nutzen Sie sequentielle Abläufe, Hub-and-Spoke- oder Swarm-Strukturen über intuitive Skills und erweitern Sie diese bei Bedarf für dauerhafte Teams (`teamwork`) oder zeitgesteuerte Cron-Jobs (`schedule`).
3. **Vollständige Modell- und Datenautonomie** — Basiert auf dem offenen Standard MCP (Model Context Protocol). Sämtliche Arbeitsbereiche, Sitzungsverläufe und Browserdaten verbleiben standardmäßig lokal auf Ihrem Rechner.

**Zielgruppen:** Praktiker, die leistungsfähige Agenten ohne Konfigurationshürden einsetzen möchten; Entwickler, die Anbieter-Lock-in vermeiden wollen; Forscher, die Browser-Automatisierung, Wissensgraphen und Aufgabenplanung in einer integrierten Lösung benötigen.

---

## Integrierte Koordinationsmuster (Gebündelte Skills)

Wählen Sie das passende Muster basierend auf Ihrer Aufgabenstellung direkt im Chat:

| Skill-Name | Koordinationsmuster | Empfohlener Einsatzbereich |
| ---------- | ------------------- | -------------------------- |
| `pipeline` | Sequentielle Phasen | Die Ausgabe einer Phase dient als Eingabe der nächsten (Recherche → Entwurf → Prüfung) |
| `hub-spoke` | Hub-and-Spoke | Ein zentraler Koordinator steuert und integriert mehrere spezialisierte Agenten |
| `divide-conquer` | Parallele Teilung | Unabhängige Teilaufgaben parallel bearbeiten und anschließend zusammenführen |
| `consensus-delegation` | Multiperspektivisch | Dieselbe Fragestellung an mehrere Expertenagenten richten und abgleichen |
| `gatekeeper` / `pair-programming` | Prüfschleifen | Strikte Code-Reviews oder kollaboratives Programmieren zweier Agenten |
| `delegate` | Gezielte Delegation | Aufgaben an eine dedizierte Untersitzung mit lückenloser Verlaufskontrolle delegieren |
| `teamwork` → `org` | Dauerhafte Teams | Feste Multi-Agenten-Teams mit gemeinsamen Arbeitsrichtlinien und Organisationsübersicht |
| `schedule` / `loop` / `call-me-back` | Zeit- & Ereignissteuerung | Cron-Zeitpläne, Verzögerungen in Sitzungen oder Wiederaufnahme bei Prozessereignissen |

Auswahlhilfe: [Framework-Auswahl](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · Vollständiger Leitfaden: [Sub-Agenten](docs/user/guides/sub-agents.md)

Weitere Standard-Skills: `setup-wizard`, `tool-installer`, `playbook-creator` und mehr — siehe **[Gebündelte Skills](docs/user/guides/skills.md)**

---

## Leistungsfähige MCP-Laufzeitinfrastruktur

- **Umfassende Protokollunterstützung**: stdio, HTTP, SSE und OAuth 2.1
- **Über 15 integrierte Server**: Workspace, Shell, Browser, Knowledge, Planning, Scheduled Tasks u. v. m.
- **Ein-Klick-Presets und geführte Installation**: Interaktive Tool-Konfiguration mit `tool-installer`
- **Sichere Ausführungsumgebung**: Werkzeugisolation pro Sitzung, Pfad- und Befehlsbeschränkungen sowie sichere Automatisierungsmodi

### Kern-Laufzeitinfrastruktur

| Ausführungsbereich | Funktionsumfang |
| ------------------ | --------------- |
| **Workspace** | Zeilengenaue Codebearbeitung, dateiübergreifende Operationen, Kontextinjektion über `@file` / `@skill` / `@playbook` |
| **Shell** | Isolierte und persistente Terminalumgebungen mit asynchroner Prozessüberwachung |
| **Browser** | Isolierte Browser-Sidecars sowie Bridge-Unterstützung für reguläre Chrome-Sitzungen |
| **Knowledge** | Wissensgraphen und performante BM25-Hybridsuche |
| **Export** | Strukturierte Markdown-Berichte und standardisierter ATIF-Sitzungsexport ([Sitzungsexport](docs/user/guides/session-export.md)) |

Automatische Kontextkomprimierung, Schleifenerkennung, Schutzschalter und Antwortvalidierung gewährleisten auch bei langen Sitzungen einen zuverlässigen und stabilen Betrieb.

---

## Praktische Einsatzszenarien

### Business-Operator — Automatisiertes morgendliches Briefing

1. Starten Sie das Rezept **Morning Briefing** (Tools einbinden, Assistent konfigurieren, tägliche Ausführung um 09:00 Uhr).
2. Nutzen Sie **Run now**, um den Ablauf unmittelbar zu testen.
3. Überprüfen Sie morgens die automatisch zusammengestellten Technologie- und Marktberichte, ohne ein Terminal zu öffnen.

### Einzelentwickler — Schnelle Tool-Einbindung ohne Konfigurationsdateien

1. Installieren Sie das GitHub-MCP-Preset über das Extensions-Menü.
2. Binden Sie ein lokales Code-Repository über den Workspace ein.
3. Weisen Sie den Agenten an, Sicherheits- und Architekturanalysen als lokale Markdown-Datei zu speichern.

### Power-User — Aufgaben delegieren mit Standardvorlagen

1. Geben Sie `@skill:pipeline` (oder `hub-spoke`, `divide-conquer`) im Chat ein.
2. Die Agenten koordinieren sich nach dem festgelegten Muster.
3. Erhalten Sie ein konsolidiertes Gesamtergebnis im Workspace ohne Pflege externer Bibliotheken.

### Datenschutzorientierte Teams — Vollständig lokaler Betrieb

1. Führen Sie `ollama pull qwen3:14b` aus.
2. Alle Operationen in Workspace und Shell verbleiben zu 100 % auf dem lokalen Rechner.
3. Wechseln Sie bei Bedarf flexibel zu Cloud-Modellen; der Bedienablauf bleibt identisch.

---

## Dokumentation

- **[Benutzerhandbuch (User Guide)](docs/user/README.md)** — Installation, erste Schritte, Modelle und Skills ([Dokumentations-Website](https://fritzprix.github.io/libr-agent/))
- **[Hero-Demo-Spezifikation](docs/contributing/hero-demo-spec.md)** — Produktdemo-Spezifikation (Untertitel in EN/KO/ZH)
- **[Leitfaden zur Produktkommunikation](docs/contributing/product-messaging-guide.md)** — Positionierung und Terminologie
- **[Rezepte](docs/user/guides/recipes.md)** · **[Geplante Aufgaben](docs/user/guides/scheduled-tasks.md)** · **[Sub-Agenten](docs/user/guides/sub-agents.md)** · **[Skills](docs/user/guides/skills.md)**
- **[HTTP-API](docs/api/http_api.md)** — Fernsteuerung und programmatische Freigaben
- **[Architektur](docs/architecture/agent-workflow-architecture.md)** — Sitzungsisolation und Think-Act-Observe-Schleife

---

## Erste Schritte

Laden Sie das neueste Installationsprogramm von der **[Releases-Seite](https://github.com/fritzprix/libr-agent/releases/latest)** herunter.

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.22_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_x64-setup.exe) · [`LibrAgent_0.9.22_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.22_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.22_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_amd64.AppImage) · [`LibrAgent_0.9.22_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_amd64.deb) · [`LibrAgent-0.9.22-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent-0.9.22-1.x86_64.rpm)
- **Alle Release-Assets:** [Releases-Seite](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.22)
<!-- RELEASE_DOWNLOADS_END -->

### 5-Minuten-Schnellstart

**Schritt 1 — Modell verbinden** (Settings → LLM Providers)

- Cloud-Dienste: Fügen Sie Ihren API-Schlüssel für OpenAI, Anthropic, Gemini oder Groq ein.
- Lokale Ausführung: Führen Sie `ollama pull qwen3:14b` aus und wählen Sie Ollama in den Einstellungen.

**Schritt 2 — Werkzeuge ohne JSON-Konfiguration hinzufügen**

- Installieren Sie Presets (z. B. GitHub) im Extensions-Menü, oder
- Weisen Sie den Agenten an: _"Importiere meine MCP-Server aus Cursor"_

**Schritt 3 — Workspace verknüpfen und Ergebnisse erzeugen**

- Wählen Sie Ihren lokalen Projektordner im Workspace aus.
- Geben Sie die Anweisung: _"Prüfe diesen Arbeitsbereich und fasse die Ergebnisse in `DELIVERABLE.md` zusammen"_

**Nächste Schritte — Erweiterte Koordination und Automatisierung**

- _"@skill:pipeline — Recherchiere dieses Thema, erstelle einen Entwurf, prüfe ihn und speichere den finalen Bericht"_
- _"Richte einen Teamwork-Arbeitsbereich für dieses Repository ein"_
- _"Erstelle einen täglichen Zeitplan für einen Marktbericht um 07:00 Uhr"_ (oder nutzen Sie das Morning-Briefing-Rezept)

### Direkt verwendbare Prompt-Beispiele

- _"Importiere meine MCP-Server-Konfiguration aus Cursor und zeige mir, welche Tools hinzugefügt wurden."_
- _"Installiere das GitHub-MCP-Preset und verknüpfe es mit dem Coding-Agenten."_
- _"Prüfe diesen Workspace und schreibe konkrete Verbesserungsvorschläge in `DELIVERABLE.md`."_
- _"@skill:pipeline — Analysiere dieses Fachthema vertieft, erstelle eine strukturierte Zusammenfassung und speichere den Abschlussbericht."_
- _"Richte einen täglichen Zeitplan ein, der jeden Morgen um 07:00 Uhr eine Marktübersicht generiert."_

### Entwickler-Setup

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## Design-Philosophie

- **Gebrauchsfertiges Produkt statt Bausatz**: Ohne langwierige Montage oder Programmierung direkt einsatzbereit.
- **Orchestrierung als standardisierte Skills**: Koordinationsmuster sind dokumentierte, benannte Fähigkeiten statt unübersichtlicher Beispielcode.
- **Vollständige Technologiefreiheit**: Benutzer entscheiden frei über Modelle und Werkzeuge ohne Bindung an einen bestimmten KI-Konzern.
- **Lokale Datenhoheit (Local First)**: Arbeitsbereiche, Sitzungschroniken und Browserzustände verbleiben stets auf Ihrem Rechner.
- **Robuste Laufzeitumgebung vor Einzelmodellen**: Erstklassige Tool-Integration, stabiles Zustandsmanagement und verlässliche Governance wiegen schwerer als die Eigenschaften einzelner Modelle.
- **Stabilität vor Funktionsüberladung**: Isolation, automatische Komprimierung und Schleifenschutz haben stets Vorrang vor unüberlegter Feature-Ausweitung.
- **Konsequente offene Standards**: Veröffentlicht unter der MIT-Lizenz mit MCP (Model Context Protocol) als offenem Interoperabilitätsstandard.

---

## Mitwirken & Lizenz

LibrAgent steht unter der MIT-Lizenz und wird als Open-Source-Projekt entwickelt. Beiträge in Form von Skills, Tool-Integrationen, Fehlerbehebungen oder Architekturverbesserungen sind jederzeit willkommen.

- [Leitfaden für Beiträge (Contributing Guide)](CONTRIBUTING.md)
- [Issue-Tracker](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- [Diskussionen](https://github.com/fritzprix/libr-agent/discussions)
- Benchmarks (Harbor / Terminal-Bench): siehe [Harbor-Leitfaden](benchmarks/harbor/README.md) (`pnpm bench:diverse`, `pnpm bench:terminal` etc.)

**Lizenz**: MIT
