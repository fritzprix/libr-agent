<p align="center">
  <img src="./src-tauri/icons/128x128.png" width="96" alt="LibrAgent" />
</p>

# LibrAgent

> **Un entorno de escritorio para agentes de IA que se ejecuta de forma local bajo tu control total.**
> Conecta el modelo que prefieras, instala herramientas con un solo clic y automatiza tus flujos de trabajo con patrones de coordinación multiagente comprobados.
> _Sin dependencia de proveedores cerrados y sin configuración manual de JSON. Todo el trabajo se guarda directamente en archivos locales en tu equipo._

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

![Demostración principal — Instala herramientas como aplicaciones, elige tu modelo y conserva tus archivos locales.](./assets/hero-demo-60s.gif)

[Ver video HD en WebM](./assets/hero-demo-60s.webm) · _Instala herramientas como aplicaciones, elige tu modelo y conserva tus archivos locales._

---

## En qué se diferencia LibrAgent

Muchos frameworks para agentes asumen que dominas la línea de comandos, editas manualmente complejos archivos de configuración JSON y programas la orquestación desde cero en código. Otras herramientas te atan al ecosistema cerrado de un único proveedor.

LibrAgent resuelve esto como una **aplicación de escritorio completa y lista para usar**:

| Enfoque tradicional | Con LibrAgent |
| ------------------- | ------------- |
| Editar manualmente configuraciones JSON de MCP | **Extensiones con un clic** — Ajustes preestablecidos listos (GitHub, Brave Search, sistema de archivos) e importación directa desde Cursor, VS Code y Claude Code |
| Programar sistemas multiagente desde código | **Patrones de coordinación integrados** — Habilidades estándar preconfiguradas (`pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`) |
| Atarse a las herramientas y modelos de un solo proveedor | **Libertad tecnológica absoluta** — Conecta tus claves de API en la nube o modelos locales con [Ollama](https://ollama.com) bajo licencia abierta MIT |

[Descargar la última versión](https://github.com/fritzprix/libr-agent/releases/latest) · [Guía de inicio en 5 minutos](#guía-de-inicio-en-5-minutos) · [Especificaciones de la demo](docs/contributing/hero-demo-spec.md)

---

## Qué puedes lograr en los primeros 10 minutos

### 1. Instalar herramientas con un clic y generar un entregable

- Abre **Extensions** e instala un ajuste preestablecido (como GitHub) sin editar JSON.
- Asocia tu **Workspace** a una carpeta de proyecto local.
- Envía la instrucción: _"Analiza este repositorio para identificar los riesgos principales para un nuevo colaborador y guarda el informe en `DELIVERABLE.md`"_

### 2. Ejecutar una receta de flujo de trabajo automatizada

- Inicia la receta **Morning Briefing** desde el inicio del chat o desde [Tareas programadas](docs/user/guides/scheduled-tasks.md).
- Se configurarán automáticamente las herramientas de Hacker News y Yahoo Finance, programando su ejecución diaria a las 09:00.
- Recibe automáticamente cada mañana un resumen actualizado sobre tecnología y mercados financieros.

### 3. Elegir un patrón de coordinación sin escribir código

- Define el flujo de trabajo deseado directamente en el prompt:
  - _"@skill:pipeline — investiga, redacta un borrador y luego revisa; entrega un único informe final"_
  - _"@skill:divide-conquer — divide esta tarea en partes independientes ejecutadas en paralelo y combina los resultados"_
- Los patrones de coordinación son habilidades integradas que no requieren ensamblar SDKs externos. Consulta la guía de [Subagentes y orquestación](docs/user/guides/sub-agents.md).

### 4. Mantener el control absoluto de tus modelos

- **Modelos en la nube**: Pega una clave de API para OpenAI, Anthropic, Gemini o Groq.
- **Modelos locales**: Ejecuta `ollama pull qwen3:14b` y selecciona Ollama en la configuración para trabajar de forma 100 % local y privada.

---

## Tres compromisos esenciales

1. **Interfaz gráfica sin complicaciones de configuración** — Entorno visual intuitivo, extensiones con un clic, recetas automáticas y aprobaciones en la app que eliminan la necesidad de abrir terminales y archivos de configuración.
2. **Orquestación como funcionalidad nativa** — Elige flujos secuenciales, en estrella (Hub-and-spoke) o distribuidos (Swarm) mediante habilidades, con escalado sencillo a equipos permanentes (`teamwork`) o tareas periódicas (`schedule`).
3. **Autonomía total de modelos y datos** — Diseñado sobre el estándar abierto MCP (Model Context Protocol). Todos los espacios de trabajo, historiales de sesión y datos de navegación permanecen estrictamente guardados en tu equipo.

**Usuarios ideales:** Profesionales que buscan agentes potentes sin complicaciones técnicas; desarrolladores que evitan el bloqueo de proveedores; investigadores que necesitan navegación web automatizada, bases de conocimiento y tareas programadas en una única aplicación.

---

## Patrones de coordinación integrados (Habilidades empaquetadas)

Selecciona el patrón adecuado para la estructura de tu tarea directamente desde el panel de chat:

| Nombre de habilidad | Patrón de coordinación | Caso de uso recomendado |
| ------------------- | ---------------------- | ----------------------- |
| `pipeline` | Fases secuenciales | El resultado de cada fase alimenta a la siguiente (Investigación → Borrador → Revisión) |
| `hub-spoke` | Hub-and-Spoke | Un agente coordinador central gestiona e integra múltiples agentes especialistas |
| `divide-conquer` | División paralela | Dividir el trabajo en subtareas independientes en paralelo y luego unificar |
| `consensus-delegation` | Multiperspectiva | Plantear la misma cuestión a varios agentes expertos para conciliar criterios |
| `gatekeeper` / `pair-programming` | Ciclos de revisión | Revisión estricta de código o programación colaborativa en pareja |
| `delegate` | Delegación puntual | Delegar una tarea específica a una subsesión con trazabilidad completa |
| `teamwork` → `org` | Equipo duradero | Equipos estables con estatutos compartidos y organigrama interactivo |
| `schedule` / `loop` / `call-me-back` | Tiempo y eventos | Tareas programadas con Cron, demoras controladas o reactivación por eventos externos |

Criterios de elección: [Selección de frameworks](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · Guía completa: [Subagentes](docs/user/guides/sub-agents.md)

Otras habilidades listas para usar: `setup-wizard`, `tool-installer`, `playbook-creator` y más — consulta **[Habilidades integradas](docs/user/guides/skills.md)**

---

## Infraestructura central de ejecución MCP

- **Compatibilidad total de protocolos**: stdio, HTTP, SSE y OAuth 2.1
- **Más de 15 servidores integrados**: Workspace, Shell, Browser, Knowledge, Planning, Scheduled Tasks, entre otros
- **Ajustes preestablecidos e instalación guiada**: Configuración interactiva asistida por agente con `tool-installer`
- **Aislamiento y seguridad sólidos**: Aislamiento de herramientas por sesión, restricciones de rutas y comandos, y modos de automatización seguros

### Capacidades centrales de ejecución

| Área de ejecución | Funcionalidades |
| ----------------- | --------------- |
| **Workspace** | Edición precisa de código línea por línea, operaciones multifichero, inyección de contexto con `@file` / `@skill` / `@playbook` |
| **Shell** | Terminales aisladas y persistentes con monitorización asíncrona de procesos |
| **Browser** | Navegador en entorno seguro independiente y puente de extensión para sesiones activas de Chrome |
| **Knowledge** | Grafo de conocimiento y motor de búsqueda híbrida BM25 de alto rendimiento |
| **Export** | Informes estructurados en Markdown y exportación estandarizada de trayectorias ATIF ([Exportación de sesiones](docs/user/guides/session-export.md)) |

La compactación automática de contexto, la prevención de bucles infinitos y la validación de respuestas aseguran un rendimiento estable incluso en sesiones de larga duración.

---

## Casos de uso prácticos

### Operador de negocio — Resumen matutino automatizado

1. Ejecuta la receta **Morning Briefing** (instalación de herramientas, configuración del asistente, programación diaria a las 09:00).
2. Haz clic en **Run now** para verificar el funcionamiento al instante.
3. Consulta cada mañana un informe consolidado de tecnología y mercados sin necesidad de abrir un terminal.

### Desarrollador independiente — Integración directa de herramientas

1. Instala el preset de GitHub MCP desde el menú Extensions.
2. Vincula un repositorio local de código mediante el Workspace.
3. Solicita al agente un informe de revisión de seguridad y arquitectura guardado en tu disco local.

### Usuario avanzado — Delegación con plantillas estándar

1. Escribe `@skill:pipeline` (o `hub-spoke`, `divide-conquer`) en el chat.
2. Los agentes coordinan sus tareas según el patrón seleccionado.
3. Obtén un resultado consolidado en tu espacio de trabajo sin tener que mantener librerías externas.

### Equipos con altos requisitos de privacidad — Funcionamiento 100 % local

1. Ejecuta `ollama pull qwen3:14b`.
2. Todas las operaciones en Workspace y Shell se procesan exclusivamente en tu ordenador.
3. Cambia a modelos en la nube cuando lo desees manteniendo exactamente el mismo flujo de trabajo.

---

## Documentación oficial

- **[Guía de usuario (User Guide)](docs/user/README.md)** — Instalación, primeros pasos, modelos y habilidades ([Sitio web de documentación](https://fritzprix.github.io/libr-agent/))
- **[Especificaciones de la demo (Hero Demo Spec)](docs/contributing/hero-demo-spec.md)** — Especificaciones del video demostrativo (subtítulos en EN/KO/ZH)
- **[Guía de posicionamiento y estilo](docs/contributing/product-messaging-guide.md)** — Posicionamiento del producto y terminología oficial
- **[Recetas](docs/user/guides/recipes.md)** · **[Tareas programadas](docs/user/guides/scheduled-tasks.md)** · **[Subagentes](docs/user/guides/sub-agents.md)** · **[Habilidades](docs/user/guides/skills.md)**
- **[API HTTP](docs/api/http_api.md)** — Control remoto e interfaces programáticas de aprobación
- **[Arquitectura](docs/architecture/agent-workflow-architecture.md)** — Aislamiento de sesiones y ciclo Think-Act-Observe

---

## Primeros pasos

Descarga el instalador más reciente desde la **[Página de versiones (Releases)](https://github.com/fritzprix/libr-agent/releases/latest)**.

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.22_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_x64-setup.exe) · [`LibrAgent_0.9.22_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.22_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.22_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_amd64.AppImage) · [`LibrAgent_0.9.22_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_amd64.deb) · [`LibrAgent-0.9.22-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent-0.9.22-1.x86_64.rpm)
- **Todos los paquetes:** [Página de versiones](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.22)
<!-- RELEASE_DOWNLOADS_END -->

### Guía de inicio en 5 minutos

**Paso 1 — Conectar un modelo** (Settings → LLM Providers)

- Modelos en la nube: Introduce tu clave de API para OpenAI, Anthropic, Gemini o Groq.
- Modelos locales: Ejecuta `ollama pull qwen3:14b` y selecciona Ollama en la configuración.

**Paso 2 — Añadir herramientas sin configurar JSON**

- Instala un ajuste preestablecido (ej. GitHub) desde el menú Extensions, o
- Indica al agente: _"Importa mis servidores MCP desde Cursor"_

**Paso 3 — Vincular un espacio de trabajo y generar un entregable**

- Selecciona la carpeta de tu proyecto local en Workspace.
- Solicita al agente: _"Revisa este espacio de trabajo y escribe los resultados en `DELIVERABLE.md`"_

**Siguientes pasos — Coordinación avanzada y automatización**

- _"@skill:pipeline — investiga este tema, redacta un borrador, revísalo y guarda el informe final"_
- _"Prepara un espacio de trabajo teamwork para este repositorio"_
- _"Configura una tarea programada diaria de análisis de competencia a las 07:00"_ (o ejecuta la receta Morning Briefing)

### Ejemplos de instrucciones listos para usar

- _"Importa la configuración de mis servidores MCP desde Cursor y muéstrame qué herramientas se agregaron."_
- _"Instala el ajuste preestablecido de GitHub MCP y conéctalo a un agente de programación."_
- _"Revisa este espacio de trabajo y redacta sugerencias de mejora concretas en `DELIVERABLE.md`."_
- _"@skill:pipeline — investiga a fondo esta propuesta técnica, redacta un borrador y realiza una revisión cruzada para generar el informe final."_
- _"Crea una tarea programada para generar un resumen del mercado cada mañana a las 07:00."_

### Configuración para desarrolladores

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## Filosofía de diseño

- **Producto terminado antes que kit de montaje**: Listo para trabajar de inmediato sin necesidad de ensamblar código.
- **Orquestación mediante habilidades bien definidas**: Los patrones de colaboración están estructurados y documentados como capacidades con nombre propio.
- **Libertad absoluta de componentes**: El usuario mantiene el control total sobre los modelos y herramientas sin dependencias impuestas.
- **Prioridad local (Local First)**: Tus espacios de trabajo, historiales de sesión y datos de navegación se mantienen bajo tu control directo en tu equipo.
- **El entorno de ejecución por encima del modelo**: Una integración sólida de herramientas, control de estados y gobernanza garantizan mejores resultados que cualquier modelo individual.
- **Estabilidad antes que saturación de funciones**: El aislamiento de contexto, la compresión de memoria y la prevención de bucles son prioridades verificadas.
- **Compromiso firme con estándares abiertos**: Licencia abierta MIT adoptando MCP (Model Context Protocol) como eje fundamental de interoperabilidad.

---

## Contribuciones y licencia

LibrAgent es un proyecto de código abierto bajo licencia MIT. Agradecemos cualquier contribución, como nuevas habilidades, integraciones MCP, corrección de errores o mejoras arquitectónicas.

- [Guía de contribución (Contributing Guide)](CONTRIBUTING.md)
- [Seguimiento de problemas (Issue Tracker)](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- [Foro de discusión (Discussions)](https://github.com/fritzprix/libr-agent/discussions)
- Pruebas de rendimiento (Harbor / Terminal-Bench): consulta la [guía de Harbor](benchmarks/harbor/README.md) (`pnpm bench:diverse`, `pnpm bench:terminal`, …)

**Licencia**: MIT
