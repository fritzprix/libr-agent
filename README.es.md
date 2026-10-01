# 🤖 LibrAgent

> **Un entorno operativo de agentes que tú ejecutas — elige el modelo, herramientas en un clic, elige el patrón de coordinación.**
> _Sin arnés de proveedor. Sin deberes de JSON. El trabajo termina como archivos en tu máquina._

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

---

## Cómo LibrAgent es distinto

La mayoría de arneses de agentes asumen que editas JSON de MCP, vives en una terminal y armas la orquestación en código (o te encierran en la pila de un solo proveedor).

LibrAgent es un **producto de escritorio** para el mismo trabajo:

| En lugar de… | Obtienes… |
| ------------ | --------- |
| Editar a mano configs MCP | **Extensions** — presets de un clic (GitHub, Brave Search, Filesystem, …) e importación desde Cursor / VS Code / Claude Code / Windsurf |
| «Tenemos multiagente» | **Patrones de coordinación con nombre** como skills empaquetados — `pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, … |
| El modelo + herramientas de un proveedor | **Tu** LLM (clave API o [Ollama](https://ollama.com)) y **tu** pila MCP — MIT, local primero |

[Descargar la última versión](https://github.com/fritzprix/libr-agent/releases/latest) · [Incorporación en 5 minutos](#incorporación-en-5-minutos) · [Especificación de demo hero](docs/contributing/hero-demo-spec.md)

---

## Qué puedes hacer en los primeros 10 minutos

### 1. Herramientas en un clic, luego un entregable

- Abre **Extensions** e instala un preset (p. ej. GitHub) — sin JSON
- Apunta **Workspace** a una carpeta real
- Pide: _"Revisa este repo para el mayor riesgo para un nuevo colaborador y guarda `DELIVERABLE.md`"_

### 2. Desplegar una receta de workflow de un clic

- Lanza la receta **Morning Briefing** desde el inicio de Chat o [Scheduled Tasks](docs/user/guides/scheduled-tasks.md)
- Instala presets de Hacker News + Yahoo Finance, configura un asistente y programa una ejecución diaria a las 9 AM
- Despierta con un briefing sintetizado de tech y mercados — sin supervisión

### 3. Elegir un patrón de coordinación (sin armar un framework)

- Di cómo es el trabajo, o adjunta un skill por nombre:
  - _"@skill:pipeline — investiga, luego redacta, luego revisa; deja un informe final"_
  - _"@skill:divide-conquer — divide esto en piezas independientes y fusiona los resultados"_
- Los patrones son skills productizados — no un SDK que cablees tú. Ver [Sub-agents & orchestration](docs/user/guides/sub-agents.md).

### 4. Mantener la libertad del modelo

- Nube: pega una clave API de OpenAI / Anthropic / Gemini / Groq
- Local: `ollama pull qwen3:14b` y selecciona Ollama — el mismo arnés en ambos casos

---

## Tres promesas de producto

1. **Superficie sin deberes de arnés** — GUI, Extensions de un clic, recetas, aprobaciones in-app, `@skill:` — no «abre primero una config y un shell».
2. **Orquestación como producto** — elige flujos Sequential / Hub-and-spoke / estilo Swarm vía skills; crece a `teamwork` → `org` y `schedule` cuando necesites equipos duraderos o cron — aún sin armar LangGraph/CrewAI tú mismo.
3. **Libertad de proveedor y pila** — cualquier LLM soportado, MCP como infraestructura, importar configs MCP de IDE existentes, licencia MIT, workspaces y estado del navegador locales por defecto.

**Mejor encaje:** operadores y power users que quieren profundidad de arnés sin vivir en JSON; desarrolladores que rechazan una pila de agentes de un solo proveedor; investigadores que necesitan navegador + conocimiento + horarios en un solo producto.

---

## Patrones de coordinación (skills empaquetados)

Elige el modelo según la **forma del trabajo**, luego ejecútalo desde el chat:

| Skill | Patrón | Cuándo usarlo |
| ----- | ------ | ------------- |
| `pipeline` | Etapas secuenciales | Las salidas alimentan el siguiente paso (investigar → redactar → revisar) |
| `hub-spoke` | Hub-and-spoke | Un coordinador integra a muchos workers |
| `divide-conquer` | División paralela | Piezas independientes, luego fusionar |
| `consensus-delegation` | Multiperspectiva | Misma pregunta a varios especialistas, luego reconciliar |
| `gatekeeper` / `pair-programming` | Bucles de revisión | Revisión estricta o codificación con dos agentes |
| `delegate` | Traspaso ligero | Una sesión hija, linaje rastreado |
| `teamwork` → `org` | Equipo duradero | Constitución compartida + Org UI |
| `schedule` / `loop` / `call-me-back` | Tiempo y eventos | Cron, demoras en sesión, o reanudar por proceso/webhook |

Heurísticas de selección: [framework-selection](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · guía completa: [Sub-agents](docs/user/guides/sub-agents.md).

Otros skills del primer día: `setup-wizard`, `tool-installer`, `playbook-creator`, y más — **[Bundled Skills](docs/user/guides/skills.md)**.

---

## Plataforma MCP (sigue siendo capaz para power users)

- Transportes: stdio, HTTP, SSE, OAuth 2.1
- 15+ servidores integrados (Workspace, Shell, Browser, Knowledge, Planning, Scheduled Tasks, …)
- Presets de un clic + instalación asistida por agente (`tool-installer`)
- Aislamiento de herramientas por sesión; guardas de ruta/comando; modos YOLO / unsafe opcionales para automatización

### Sustrato de ejecución

| Sustrato | Capacidades |
| -------- | ----------- |
| **Workspace** | Edición precisa por línea, ops multiarchivo, contexto `@file` / `@skill` / `@playbook` |
| **Shell** | Shells aislados y persistentes con monitorización de procesos async |
| **Browser** | Sidecar de navegador aislado; perfiles de login guardados opcionales |
| **Knowledge** | Conocimiento en grafo + búsqueda BM25 |
| **Export** | Informes Markdown y exportaciones de trayectoria ATIF ([session export](docs/user/guides/session-export.md)) |

Las sesiones largas siguen productivas mediante compactación de contexto, prevención de bucles, cortacircuitos y guardas de respuestas obsoletas.

---

## Escenarios del mundo real

### Operador — de app vacía a briefing diario

1. Ejecuta la receta **Morning Briefing** (presets + asistente + horario 9 AM)
2. Haz clic en **Run now** una vez para verificar
3. Déjalo — el informe llega sin abrir una terminal

### Desarrollador en solitario — preset, no archivos de config

1. Extensions → instala el preset GitHub MCP
2. Adjunta un repo local vía Workspace
3. Pide un informe Markdown de seguridad/revisión que quedes en disco

### Power user sin framework — orquestación con nombre

1. `@skill:pipeline` (o `hub-spoke` / `divide-conquer`) según la forma del trabajo
2. Los agentes se coordinan bajo ese patrón
3. Un entregable fusionado en el workspace — sin biblioteca de orquestación que mantener

### Equipo sensible a la privacidad — mismo producto, modelo local

1. `ollama pull qwen3:14b`
2. Workspace + Shell se quedan en la máquina
3. Cambia a claves cloud más tarde si quieres — el arnés no te cambia de proveedor

---

## Documentación

- **[User Guide](docs/user/README.md)** — instalación, primer chat, modelos, skills ([docs site](https://fritzprix.github.io/libr-agent/))
- **[Hero Demo Spec](docs/contributing/hero-demo-spec.md)** — demo de producto canónica (subtítulos EN/KO/ZH)
- **[Product Messaging Guide](docs/contributing/product-messaging-guide.md)** — posicionamiento y copy
- **[Recipes](docs/user/guides/recipes.md)** · **[Scheduled Tasks](docs/user/guides/scheduled-tasks.md)** · **[Sub-agents](docs/user/guides/sub-agents.md)** · **[Skills](docs/user/guides/skills.md)**
- **[HTTP API](docs/api/http_api.md)** — control remoto y aprobaciones programáticas
- **[Architecture](docs/architecture/agent-workflow-architecture.md)** — aislamiento de sesión y Think-Act-Observe

---

## Empezar

Descarga el último instalador desde la **[página de Releases](https://github.com/fritzprix/libr-agent/releases/latest)**.

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.21_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_x64-setup.exe) · [`LibrAgent_0.9.21_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.21_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.21_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_amd64.AppImage) · [`LibrAgent_0.9.21_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_amd64.deb) · [`LibrAgent-0.9.21-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent-0.9.21-1.x86_64.rpm)
- **All release assets:** [Releases page](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.21)
<!-- RELEASE_DOWNLOADS_END -->

### Incorporación en 5 minutos

**Paso 1 — Conectar un modelo** (Settings → LLM Providers)

- Nube: pega una clave API de OpenAI / Anthropic / Gemini / Groq
- Local: `ollama pull qwen3:14b`, luego selecciona Ollama en Settings

**Paso 2 — Añadir herramientas sin JSON**

- Extensions → instala un preset (p. ej. GitHub), **o**
- Dile a un agente: _"Importa mis servidores MCP desde Cursor"_

**Paso 3 — Adjuntar un workspace y pedir un archivo que guardes**

- Apunta Workspace a una carpeta de proyecto real
- _"Revisa este workspace y escribe los hallazgos en `DELIVERABLE.md`."_

**Siguiente — coordinación y automatización**

- _"@skill:pipeline — investiga, redacta, luego revisa; un informe final."_
- _"Prepara un workspace teamwork para este repo."_
- _"Configura un brief diario de competidores a las 7am."_ (o ejecuta la receta Morning Briefing)

### Primeros prompts para copiar y pegar

- _"Importa mis servidores MCP desde Cursor y muéstrame qué se añadió."_
- _"Instala el preset GitHub MCP y adjúntalo a un agente de código."_
- _"Revisa este workspace y escribe los hallazgos en `DELIVERABLE.md`."_
- _"@skill:pipeline — investiga este tema, redacta un resumen, luego revisa; guarda el informe final."_
- _"Configura un brief diario de competidores a las 7am."_

### Configuración de desarrollador

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## Dónde encaja mejor LibrAgent

| Si quieres… | LibrAgent es fuerte porque… |
| ----------- | --------------------------- |
| **Profundidad de arnés sin deberes de arnés** | Presets de Extensions, recetas, patrones `@skill:` y aprobaciones — no onboarding JSON primero |
| **Orquestación sin construir un framework** | `pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, `teamwork` / `org`, `schedule` llegan como producto |
| **Libertad frente a una pila de agentes de proveedor** | Trae tu modelo y herramientas MCP; MIT; local primero por defecto |
| **Un sustrato de ejecución real** | Workspace, shell, browser, knowledge, playbooks y guardas de sesiones largas |
| **Un producto de escritorio nativo MCP** | Presets, importación y 15+ builtins — no un envoltorio fino de chat |

---

## Filosofía de diseño

- **Producto sobre kit**: el arnés es usable sin armarlo.
- **Orquestación como skills**: los patrones de coordinación tienen nombre, son seleccionables y están documentados — no enterrados en repos de ejemplo.
- **Libertad de pila**: modelos y herramientas son elecciones del usuario; el producto no exige un proveedor de IA.
- **Local First**: workspaces, sesiones, skills y estado del navegador quedan bajo tu control. Cloud LLM / MCP remoto solo cuando optas por ello.
- **Arnés sobre modelo**: herramientas, estado de sesión, delegación y gobernanza importan más que cualquier modelo único.
- **Estabilidad sobre features**: aislamiento, compactación, prevención de bucles — antes de perseguir funciones.
- **Estándares abiertos**: MIT. MCP como capa de interoperabilidad.

---

## Contribución y licencia

LibrAgent tiene licencia MIT y se construye en abierto. Las contribuciones son bienvenidas — skills empaquetados, integraciones MCP, correcciones de bugs o mejoras de arquitectura.

- 📖 [Contributing Guide](CONTRIBUTING.md)
- 🐛 [Issue Tracker](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- 💬 [Discussions](https://github.com/fritzprix/libr-agent/discussions)
- 🧪 Benchmarks (Harbor / Terminal-Bench): ver [Harbor guide](benchmarks/harbor/README.md) (`pnpm bench:diverse`, `pnpm bench:terminal`, …)

**License**: MIT
