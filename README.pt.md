# 🤖 LibrAgent

> **Uma plataforma de agentes de IA autónomos, leve e com estado.**

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent é um executor de agentes local-first projetado para manter o contexto entre interações. Ao contrário de clientes sem estado, ele mantém as abas do navegador e as sessões de terminal ativas entre os turnos, permitindo que os agentes trabalhem de forma mais fluida num espaço de trabalho persistente.

Implementa padrões abertos como **MCP (Model Context Protocol)** e **Skills** para permanecer modular e extensível.

---

## Porquê LibrAgent?

O objetivo deste projeto é tornar os agentes autónomos acessíveis. Muitas ferramentas existentes permanecem presas atrás de comandos de terminal e configurações JSON manuais, criando uma lacuna que exclui muitos utilizadores potenciais. O LibrAgent visa colmatar essa lacuna, fornecendo um ambiente local-first onde qualquer pessoa pode implementar e gerir agentes sem precisar de ser um programador.

---

## 🎬 Demonstração

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

*Automatização do navegador e execução de shell num único fluxo de trabalho com estado.*

---

## Funcionalidades Principais

### 1. Espaço de Trabalho Persistente
Os agentes operam num ambiente de longa duração em vez de iniciar novos processos em cada turno.
- **Webview ao Vivo**: Automatização do navegador em tempo real usando Tauri webviews. As sessões e os cookies persistem entre turnos.
- **Terminal Unificado**: Uma shell persistente e protegida (suporta Python/Node.js) que partilha o estado com o espaço de trabalho.

### 2. Orquestração Multi-Agente
O LibrAgent permite que os agentes deleguem tarefas a sub-agentes especializados.
- **Assistentes**: Gerencie perfis de agentes com prompts de sistema e configurações de ferramentas exclusivos.
- **Inteligência de Enxame (Swarm)**: Os agentes pai podem criar, enviar mensagens e aguardar resultados de sub-agentes para resolver tarefas complexas.

### 3. Extensibilidade
A plataforma foi projetada para ser expandida através de padrões da comunidade.
- **Extensões (MCP)**: Suporte total para o protocolo MCP. Ligue-se a qualquer servidor MCP instantaneamente.
- **Presets de um Clique**: Catálogo selecionado para GitHub, Brave Search, etc., disponível diretamente na interface.
- **Skills & Playbooks**: Snippets de comportamento reutilizáveis e modelos de fluxo de trabalho estruturados.

### 4. Autonomia e Agendamento
- **Modo YOLO**: Execução autónoma opcional para ferramentas sensíveis sem aprovação manual.
- **Tarefas Agendadas**: Automatização baseada em Cron com recuperação automática após reinícios e suporte para áreas de trabalho específicas.

### 5. Contexto e Métricas
- **@menções**: Injeção direta de ficheiros, competências ou playbooks no chat.
- **Multimodal**: Lida com imagens e áudio para modelos OpenAI, Anthropic e Gemini.
- **Observabilidade**: Métricas de TPS em tempo real e hits de cache de prompt (para Anthropic/Gemini).

---

## 📦 Instalação

Descarregue os binários mais recentes para Windows, macOS ou Linux a partir da [página de Lançamentos](https://github.com/fritzprix/libr-agent/releases/latest).

**Compilar a partir do código-fonte:**
```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## Escolhas de Design

- **Local First**: Os seus dados e chaves de API permanecem na sua máquina.
- **Tauri + Rust**: Escolhido pela segurança (segurança de memória), desempenho e tamanho reduzido do binário.
- **SQLite (SeaORM)**: Usado para uma persistência local robusta de sessões e configurações.

---

## Contribuição e Licença

Contribuições são bem-vindas. Por favor, consulte `CONTRIBUTING.md`.

**Licença**: MIT
