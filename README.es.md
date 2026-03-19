# 🤖 LibrAgent

> **Una plataforma de agentes de IA autónomos, ligera y con estado.**

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent es un ejecutor de agentes local-first diseñado para mantener el contexto entre interacciones. A diferencia de los clientes sin estado, mantiene activas las pestañas del navegador y las sesiones de terminal entre turnos, permitiendo que los agentes trabajen de forma más fluida en un espacio de trabajo persistente.

Implementa estándares abiertos como **MCP (Model Context Protocol)** y **Skills** para seguir siendo modular y extensible.

---

## ¿Por qué LibrAgent?

El objetivo de este proyecto es hacer accesibles los agentes autónomos. Muchas herramientas existentes quedan atrapadas detrás de comandos de terminal y configuraciones JSON manuales, creando una brecha que excluye a muchos usuarios potenciales. LibrAgent tiene como objetivo cerrar esta brecha proporcionando un entorno local-first donde cualquiera pueda desplegar y gestionar agentes sin necesidad de ser un desarrollador.

---

## 🎬 Demostración

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

_Automatización del navegador y ejecución de shell en un único flujo de trabajo con estado._

---

## Funciones Principales

### 1. Espacio de Trabajo Persistente

Los agentes operan dentro de un entorno de larga duración en lugar de lanzar procesos nuevos en cada turno.

- **Webview en Vivo**: Automatización del navegador en tiempo real mediante Tauri webviews. Las sesiones y las cookies persisten entre turnos.
- **Terminal Unificada**: Una shell persistente y aislada (soporta Python/Node.js) que comparte el estado con el espacio de trabajo.

### 2. Orquestación Multi-Agente

LibrAgent permite que los agentes deleguen tareas a sub-agentes especializados.

- **Asistentes**: Gestiona perfiles de agentes con prompts de sistema y configuraciones de herramientas únicos.
- **Inteligencia de Enjambre (Swarm)**: Los agentes padre pueden generar, enviar mensajes y esperar resultados de los sub-agentes para resolver tareas complejas.

### 3. Extensibilidad

La plataforma está diseñada para ser expandida a través de estándares comunitarios.

- **Extensiones (MCP)**: Soporte completo para el protocolo MCP. Conéctate a cualquier servidor MCP al instante.
- **Presets de un Clic**: Catálogo seleccionado para GitHub, Brave Search, etc., disponible directamente en la interfaz.
- **Skills & Playbooks**: Snippets de comportamiento reutilizables y plantillas de flujo de trabajo estructuradas.

### 4. Autonomía y Programación

- **Modo YOLO**: Ejecución autónoma opcional para herramientas sensibles sin aprobación manual.
- **Tareas Programadas**: Automatización basada en Cron con recuperación automática tras reinicios y soporte para espacios de trabajo específicos.

### 5. Contexto y Métricas

- **@menciones**: Inyección directa de archivos, habilidades o playbooks en el chat.
- **Multimodal**: Maneja imágenes y audio para modelos de OpenAI, Anthropic y Gemini.
- **Observabilidad**: Métricas de TPS en tiempo real y aciertos de caché de prompts (para Anthropic/Gemini).

---

## 📦 Instalación

Descarga los últimos binarios para Windows, macOS o Linux desde la [página de Lanzamientos](https://github.com/fritzprix/libr-agent/releases/latest).

**Construir desde la fuente:**

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## Decisiones de Diseño

- **Local First**: Tus datos y claves API permanecen en tu máquina.
- **Tauri + Rust**: Elegido por seguridad (seguridad de memoria), rendimiento y tamaño reducido del binario.
- **SQLite (SeaORM)**: Utilizado para una persistencia local robusta de sesiones y configuraciones.

---

## Contribución y Licencia

Las contribuciones son bienvenidas. Por favor, consulta `CONTRIBUTING.md`.

**Licencia**: MIT
