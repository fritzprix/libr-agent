# 🤖 LibrAgent

> **Una app de escritorio local-first para agentes de IA que usan herramientas reales, trabajan en paralelo y siguen bajo tu control.**
> _Conecta cualquier LLM, añade cualquier servidor MCP y deja que los agentes lean archivos, ejecuten shells, naveguen por la web y terminen automatizaciones de verdad._

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent es un **espacio de trabajo de agentes local-first** construido con Tauri + Rust + React. No es otra interfaz de chat más: está pensado para acceso real a archivos, ejecución de shell, automatización del navegador, extensibilidad MCP y flujos multi-agente que aguantan horas de trabajo en vez de romperse tras una demo bonita.

Puedes conectar modelos cloud o runtimes locales como Ollama, importar servidores MCP desde herramientas que ya usas y luego dejar que los agentes inspeccionen código, editen archivos, ejecuten comandos, naveguen por la web, capturen conocimiento y deleguen subtareas sin mandar todo tu flujo de trabajo a la VM cloud de otra persona.

**Empieza aquí:** [Descargar la última release](https://github.com/fritzprix/libr-agent/releases/latest) · [Ir a la ruta de incorporación de 5 minutos](#la-ruta-de-incorporación-de-5-minutos) · [Ver escenarios reales](#-escenarios-reales)

---

## ¿Por qué LibrAgent?

La mayoría de los productos de agentes siguen forzando un compromiso bastante molesto:

- **UI fácil, pero ejecución floja**
- **Automatización potente, pero experiencia de producto pobre**
- **Comodidad cloud, pero poco control sobre la privacidad**
- **Framework flexible, pero te toca montar toda la stack**

LibrAgent apunta justo al punto medio que la gente de verdad quiere:

- **Control local-first** sobre archivos, workspaces, sesiones y estado del navegador
- **Extensibilidad abierta con MCP** en vez de una historia cerrada de plugins
- **Capacidad real de ejecución** en shell, browser, workspace y knowledge
- **Una GUI usable por humanos normales** sin perder profundidad para power users
- **Un camino natural de un agente a varios** cuando un solo asistente ya no basta

### Para quién es LibrAgent

- **Desarrolladores solo** que quieren agentes que puedan realmente leer, editar, ejecutar, navegar y persistir contexto localmente
- **Usuarios avanzados y operadores** que quieren componer su propio stack desde modelos locales, proveedores API, servidores MCP y flujos de trabajo programados
- **Investigadores y analistas** que necesitan automatización del navegador, captura de conocimiento, playbooks repetibles y sesiones de larga duración
- **Equipos sensibles a la privacidad** que quieren ejecución local, gobernanza explícita y un camino de un agente único a una organización coordinada

---

## 🎬 La plataforma en acción

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

_De un solo agente a un enjambre coordinado — delegación recursiva, herramientas MCP y espacio de trabajo persistente en un substrato unificado._

---

## Lo que puedes hacer en los primeros 10 minutos

### 1. Revisar un repositorio con herramientas reales

- Conecta un repo local con la herramienta Workspace
- Añade el preset GitHub MCP
- Pide: _"Encuentra problemas de seguridad en la PR #42 y guarda el informe"_

### 2. Montar una stack de agentes totalmente local

- Ejecuta `ollama pull qwen3:14b`
- Conecta Workspace + Shell
- Deja que un agente lea, modifique, pruebe e itere sin enviar tu código a una VM cloud

### 3. Convertir investigación en un flujo repetible

- Añade Browser + Knowledge
- Pide: _"Sigue estos 5 blogs de competidores y resúmelo cada mañana"_
- Convierte una tarea puntual en una pipeline programada

### 4. Pasar de un asistente a un equipo de verdad

- Crea especialistas con `specialist-creator`
- Divide el trabajo con `delegate`
- Convierte la colaboración recurrente en un workspace compartido `teamwork` u `org`

---

## Por qué aguanta más allá de la demo

### 1. 🔐 Seguridad local-first — Tus datos permanecen en tu máquina

LibrAgent trata la seguridad como una preocupación arquitectónica de primer orden:

- **Aislamiento de sesión**: Cada sesión de agente recibe su propia instancia dedicada `MCPServiceProxy` — cero filtraciones de datos entre sesiones
- **SecurityValidator integrado**: Ataques de traversal de rutas e inyección de comandos bloqueados a nivel del sistema
- **No se requiere substrato cloud**: La ejecución principal ocurre localmente; las conexiones externas se limitan sobre todo a los proveedores LLM cloud y servicios remotos MCP/HTTP que decidas usar, además de las comprobaciones de actualización en builds de producción
- **Soporte offline completo**: Combina con [Ollama](https://ollama.ai) para un stack de agentes completamente aislado

#### Lo que permanece local vs lo que sale de tu máquina

- **Siempre local**: espacios de trabajo, archivos locales, habilidades agrupadas, estado de sesión, configs de servidores MCP, estado del navegador y ejecución de herramientas locales
- **Sale de tu máquina cuando hace falta**: solicitudes a proveedores LLM cloud o servicios MCP/HTTP remotos que configuras explícitamente, además de las comprobaciones de actualización en builds de producción
- **Modo offline completo**: usa Ollama u otro runtime local con servidores MCP locales para un flujo de trabajo aislado

### 2. 🧩 Ecosistema nativo MCP — Extensibilidad infinita por diseño

MCP (Model Context Protocol) es el estándar abierto detrás del modelo de extensibilidad de LibrAgent. LibrAgent lo trata no como una característica — sino como la columna vertebral arquitectónica:

- **Soporte completo de transportes**: stdio, HTTP, SSE y OAuth 2.1 — la especificación completa
- **12+ servidores integrados**: Planning, Knowledge (RAG), Browser Automation, Workspace, Shell Execution, Content Store, y más
- **Catálogo de presets**: Instala GitHub, Brave Search, Filesystem y otros servidores populares con un clic
- **Instancias aisladas por sesión**: Cada sesión de agente tiene estado de servidor MCP independiente — sin interferencia entre agentes paralelos
- **Importa desde cualquier lugar**: Migra automáticamente configs MCP desde Cursor, VS Code, Claude Code o Windsurf

### 3. 🦾 Substrato de ejecución de nivel producción

La mayoría de las herramientas de IA son impresionantes en demos y frágiles en producción. LibrAgent está meticulosamente diseñado para trabajo real y duradero:

| Substrato     | Capacidades                                                                                                                |
| ------------- | -------------------------------------------------------------------------------------------------------------------------- |
| **Workspace** | Edición precisa a línea, operaciones multi-archivo, búsqueda unificada, inyección de contexto `@file`/`@skill`/`@playbook` |
| **Shell**     | Ejecución aislada Y shells persistentes — monitoreo de procesos asíncrono (`poll`, `read output`, `list`)                  |
| **Browser**   | Automatización de navegador headless con un modelo de interacción similar a Playwright y garantías de coherencia del caché |
| **Knowledge** | Gestión de conocimiento basada en grafos con extracción entidad/relación (v2), búsqueda de texto completo BM25             |

**Ingeniería de confiabilidad incluida**: Compactación de contexto, prevención de bucles, disyuntores y guardas contra respuestas obsoletas mantienen a los agentes productivos en sesiones que duran horas.

### 4. 🤝 Enjambre → Equipo → Organización: Multi-agente a toda escala

LibrAgent tiene una historia multi-agente coherente desde la ejecución solo hasta la coordinación organizacional explícita:

- **`delegate`**: Los agentes padres generan, informan y monitorizan sesiones hijas con seguimiento de linaje explícito
- **`teamwork`**: Construye un espacio de trabajo de task-force completo (agents.md, MISSION.md, KANBAN.md) con un solo comando
- **`org`**: Formaliza equipos con identidad de organización duradera, reanudación de sesión raíz y jerarquía de miembros visible
- **`schedule`**: Automatización basada en CRON — los agentes se ejecutan sin supervisión, según un calendario, con constitución de espacio de trabajo
- **Concurrency Gate**: Límites estrictos en sesiones paralelas y procesos shell para prevenir deadlocks y costos desbocados

### 5. ⚡ Habilidades agrupadas — La forma más rápida de ir de una instalación vacía a un enjambre operativo

LibrAgent viene con una biblioteca creciente de **Habilidades agrupadas**. No son prompts aleatorios — son procedimientos operativos reutilizables que cualquier agente puede invocar por nombre.

Las habilidades más importantes para el primer día:

| Habilidad            | Qué hace                                                                                                           |
| -------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `system-setup`       | Detecta e instala runtimes faltantes (Python, Node.js, uv) en todas las plataformas                                |
| `mcp-installer`      | Registra servidores MCP desde paquetes npm, URLs de GitHub o bloques de config JSON                                |
| `mcp-importer`       | Importa configs MCP existentes desde Cursor, VS Code, Windsurf y similares                                         |
| `specialist-creator` | Diseña una config de agente completa (prompt del sistema, modelo, herramientas) desde una descripción de rol       |
| `crew-constructor`   | Escanea herramientas disponibles y crea automáticamente un equipo de especialistas adaptado                        |
| `agent-tooling`      | Audita agentes, detecta inadecuaciones de capacidades y reequilibra dinámicamente las asignaciones de herramientas |
| `delegate`           | Guía el traspaso de sesión padre→hijo con transferencia de contexto explícita y seguimiento de linaje              |
| `teamwork`           | Construye la constitución de espacio de trabajo compartido para el trabajo multi-agente coordinado                 |
| `org`                | Formaliza la identidad de organización duradera y la jerarquía de miembros visible                                 |
| `schedule`           | Crea y gestiona grupos de tareas programadas recurrentes para automatización sin supervisión                       |
| `soul-awakening`     | Ancla un agente a un persona `SOUL.md` — tono, postura, identidad                                                  |

Y eso es solo la capa de operador. LibrAgent también incluye habilidades de dominio para:

- **Conocimiento e investigación**: `deep-research-report`, `knowledge-distiller`
- **Flujos de trabajo documentales**: `document-to-markdown`, `docx`, `pptx`
- **Creación de habilidades y workflows**: `skill-creator`, `skill-deployer`, `playbook-creator`, `mcp-builder`
- **Operaciones especializadas**: `computer-diagnosis` y otros asistentes especializados

_Importante: `bootstrap` es una capacidad integrada que se usa frecuentemente junto con estas habilidades. Las Habilidades agrupadas son los procedimientos reutilizables; los integrados y las herramientas MCP son el substrato de ejecución subyacente._

---

## 🌍 Escenarios del mundo real

### Desarrollador solo — Revisión de código automatizada

1. Conecta tu repositorio local mediante la herramienta Workspace
2. Instala el preset GitHub MCP (un clic)
3. Solicita: _"Encuentra problemas de seguridad en el PR #42 y produce un informe en Markdown"_
4. El agente lee el código, ejecuta el análisis, guarda los hallazgos en el servidor Knowledge

### Marketero — Inteligencia competitiva en piloto automático

1. Configura 5 blogs de competidores mediante la herramienta Browser
2. Dile a un agente: _"Crea un brief de competidores programado cada mañana a las 7am"_ — el agente puede usar la habilidad `schedule` para configurar el grupo de tareas recurrente
3. El agente navega, resume y añade al Knowledge store
4. Pregunta en cualquier momento: _"Resume los movimientos de los competidores de la semana pasada"_

### Equipo de ingeniería — Stack de agentes offline

1. `ollama pull qwen3:14b` — sin claves API, sin nube
2. Conecta las herramientas Workspace + Shell a tu codebase
3. La propiedad intelectual sensible nunca sale de la máquina
4. Los agentes leen, modifican, prueban y hacen commit — completamente local

### Usuario avanzado — Pipeline de investigación multi-agente

1. Usa `crew-constructor` para generar automáticamente: Researcher×3, Analyst×1, Writer×1
2. El orquestador delega en paralelo mediante la habilidad `delegate`
3. Los resultados se fusionan en un único informe estructurado en Content Store
4. Programa todo el workflow semanalmente mediante `schedule`

---

## 📖 Documentación y guías

- **[Guía de navegación](docs/guides/navigation-guide.md)**: El hub Command & Control — `/assistants` (Definiciones de roles) y `/playbooks` (Blueprints de workflow).
- **[Guía de arquitectura](docs/architecture/agent-workflow-architecture.md)**: Aislamiento de sesión, motor de orquestación y el bucle Think-Act-Observe impulsado por Rust.
- **[Guía de herramientas integradas](docs/guides/builtin_tool_bp.md)**: Estándares de diseño de herramientas y patrones de respuesta MCP.

---

## 📦 Comenzando

Descarga el último instalador para tu plataforma desde la **[página de Releases](https://github.com/fritzprix/libr-agent/releases/latest)**.

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.8.6_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.6/LibrAgent_0.8.6_x64-setup.exe) · [`LibrAgent_0.8.6_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.6/LibrAgent_0.8.6_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.8.6_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.6/LibrAgent_0.8.6_aarch64.dmg)
- **Linux:** [`LibrAgent_0.8.6_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.6/LibrAgent_0.8.6_amd64.AppImage) · [`LibrAgent_0.8.6_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.6/LibrAgent_0.8.6_amd64.deb) · [`LibrAgent-0.8.6-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.6/LibrAgent-0.8.6-1.x86_64.rpm)
- **Todos los archivos de la release:** [página de Releases](https://github.com/fritzprix/libr-agent/releases/tag/v0.8.6)
<!-- RELEASE_DOWNLOADS_END -->

**Configuración para desarrolladores:**

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

### La ruta de incorporación de 5 minutos

**Paso 1 — Conecta un modelo** (Settings → LLM Providers)

- Nube: pega una clave API de OpenAI / Anthropic / Gemini / Groq
- Local: `ollama pull qwen3:14b` y luego selecciona Ollama en Settings
- ¿Ya usas Cursor o VS Code? Dile a cualquier agente: _"Importa mis servidores MCP desde Cursor"_ → `mcp-importer` lo maneja

**Paso 2 — Añade herramientas MCP** (barra lateral Extensions)

- Explora el catálogo de presets y haz clic en Instalar, o
- Dile a un agente: _"Install @modelcontextprotocol/server-everything"_ → `mcp-installer` lo registra automáticamente

**Paso 3 — Crea tu primer agente**

- _"Crea un agente investigador para inteligencia competitiva"_ → `specialist-creator` diseña la config completa
- _"Construye un equipo de investigación con mis herramientas actuales"_ → `crew-constructor` crea los especialistas en lote
- _"Optimiza las asignaciones de herramientas para todos mis agentes"_ → `agent-tooling` audita y reequilibra automáticamente

**Paso 4 — Ve en paralelo con `delegate`**

- Pide a cualquier agente que delegue subtareas a sesiones hijas
- La habilidad `delegate` gestiona el traspaso de contexto, el seguimiento de linaje y la fusión de resultados

**Paso 5 — Construye un equipo persistente**

- `teamwork` → construye el espacio de trabajo compartido con `agents.md`, `MISSION.md`, `KANBAN.md`
- `org` → formaliza el equipo con identidad duradera y gestión de sesión raíz
- `schedule` → deja que un agente cree y gestione la automatización CRON para ti, sin supervisión

### Primeros prompts para copiar y pegar

- _"Importa mis servidores MCP desde Cursor y muéstrame qué se añadió."_
- _"Crea un agente investigador para inteligencia competitiva con mis herramientas actuales."_
- _"Instala el preset GitHub MCP y adjúntalo a un agente de codificación."_
- _"Delega el análisis del repositorio a una sesión hija y tráeme un resumen."_
- _"Prepara un espacio de trabajo teamwork para este repositorio y crea un equipo de especialistas listo para org."_
- _"Configura un brief diario de competidores programado a las 7am y mantenlo todo en el espacio de trabajo teamwork compartido."_

---

## Dónde encaja mejor LibrAgent

| Si quieres...                                                     | LibrAgent destaca porque...                                                                 |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| **Una estación de trabajo IA local**                              | archivos, sesiones, workspaces y estado del navegador se quedan en tu máquina por defecto   |
| **Un producto desktop realmente nativo MCP**                      | puedes instalar, importar y gestionar servidores MCP sin tratar la app como un wrapper fino |
| **Agentes que hagan trabajo real**                                | Workspace, Shell, Browser y Knowledge están diseñados para ejecución prolongada             |
| **Workflows multi-agente sin construir antes un framework**       | `delegate`, `teamwork`, `org` y `schedule` ya vienen dentro del producto                    |
| **Un equilibrio entre GUI usable y profundidad para power users** | consigues una interfaz desktop sin perder extensibilidad ni control                         |

---

## Filosofía de diseño

- **Local First**: Tus datos, claves y "almas" de agentes permanecen bajo tu control exclusivo. No se requiere substrato cloud.
- **Harnés sobre Modelo**: El entorno de ejecución — herramientas, estado de sesión, delegación, gobernanza — importa más que cualquier modelo individual. LibrAgent está diseñado para maximizar lo que cualquier modelo puede hacer.
- **Estabilidad sobre Características**: El CHANGELOG refleja un enfoque obsesivo en la corrección del runtime — aislamiento de sesión, compactación, prevención de bucles, guardas contra respuestas obsoletas.
- **MCP como Infraestructura**: No un sistema de plugins. Todo el ecosistema de herramientas está organizado alrededor de MCP como la capa de interoperabilidad principal.
- **Estándares abiertos**: Licencia MIT. Completamente comprometido con MCP, la interoperabilidad open source y la soberanía de los datos del usuario.

---

## Contribución y Licencia

LibrAgent tiene licencia MIT y se desarrolla en abierto. Las contribuciones son bienvenidas — ya sean nuevas habilidades agrupadas, integraciones MCP, correcciones de errores o mejoras de arquitectura.

- 📖 [Guía de contribución](CONTRIBUTING.md)
- 🐛 [Rastreador de problemas](https://github.com/fritzprix/libr-agent/issues)
- 💬 [Discusiones](https://github.com/fritzprix/libr-agent/discussions)

**Licencia**: MIT
