# 🤖 LibrAgent

> **El harnés de agentes para la era de la inteligencia autónoma.**
> _No solo una aplicación de chat. Un substrato de ejecución donde los agentes trabajan, colaboran y escalan._

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent es un **sistema operativo de agentes local-first** construido sobre Tauri + Rust + React. Va mucho más allá de las interfaces de chat — proporcionando un substrato de ejecución seguro, un ecosistema de herramientas nativo MCP y una arquitectura de delegación recursiva que escala un único agente a un enjambre coordinado.

Conecta cualquier LLM (nube o local vía Ollama), extiende con cualquier servidor MCP y deja que los agentes hagan trabajo real: editar archivos, ejecutar shells, navegar por la web, gestionar conocimiento — de forma autónoma, durante el tiempo que sea necesario.

---

## ¿Por qué LibrAgent?

El enfoque de la industria de la IA ha cambiado. En la práctica, **el mismo modelo puede mostrar diferencias grandes de éxito según el harness que lo rodea**. El modelo es el motor — pero el harness determina hasta dónde puede llegar.

Cada opción actual todavía impone un compromiso:

| Plataforma               | El problema                                                                                                                                                                                               |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **OpenClaw**             | Ecosistema abierto muy flexible, pero análisis de principios de 2026 destacaron instancias expuestas, manejo de secretos en texto plano y riesgos de inyección de prompts en habilidades de la comunidad. |
| **Claude Cowork**        | UX local sólido, pero aún limitado en tareas autónomas complejas. Ecosistema cerrado. No extensible.                                                                                                      |
| **Claude Code / Cursor** | Solo para desarrolladores. Requiere dominio del terminal. No es de propósito general.                                                                                                                     |
| **Google Mariner**       | Tu trabajo se ejecuta en las VMs cloud de Google. No controlas tus datos.                                                                                                                                 |
| **LangGraph / CrewAI**   | Frameworks potentes, pero tienes que ensamblarlo todo tú mismo. Sin experiencia de producto.                                                                                                              |

**LibrAgent está construido para eliminar ese compromiso.** Seguridad local-first. Extensibilidad nativa MCP. Coordinación multi-agente enjambre→organización. Una interfaz gráfica pulida que funciona para no desarrolladores. Todo en una aplicación de escritorio open source.

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

## Pilares fundamentales

### 1. 🔐 Seguridad local-first — Tus datos permanecen en tu máquina

LibrAgent trata la seguridad como una preocupación arquitectónica de primer orden:

- **Aislamiento de sesión**: Cada sesión de agente recibe su propia instancia dedicada `MCPServiceProxy` — cero filtraciones de datos entre sesiones
- **SecurityValidator integrado**: Ataques de traversal de rutas e inyección de comandos bloqueados a nivel del sistema
- **No se requiere substrato cloud**: La ejecución principal ocurre localmente; el tráfico de red externo se limita a los proveedores LLM y servicios remotos MCP/HTTP que decidas usar
- **Soporte offline completo**: Combina con [Ollama](https://ollama.ai) para un stack de agentes completamente aislado

#### Lo que permanece local vs lo que sale de tu máquina

- **Siempre local**: espacios de trabajo, archivos locales, habilidades agrupadas, estado de sesión, configs de servidores MCP, estado del navegador y ejecución de herramientas locales
- **Sale de tu máquina solo cuando lo eliges**: solicitudes a proveedores LLM cloud o servicios MCP/HTTP remotos que configuras explícitamente
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

```
Windows  →  LibrAgent_x.x.x_x64-setup.exe
macOS    →  LibrAgent_x.x.x_aarch64.dmg
Linux    →  libragent_x.x.x_amd64.AppImage
```

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

## Cómo se compara LibrAgent

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
