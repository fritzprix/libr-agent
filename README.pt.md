# 🤖 LibrAgent

> **Um ambiente operacional de agentes que você executa — escolha o modelo, ferramentas em um clique, escolha o padrão de coordenação.**
> _Sem harness de fornecedor. Sem dever de JSON. O trabalho termina como arquivos na sua máquina._

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

![Demo hero — Instale ferramentas como apps. Fique com seu modelo. Fique com o arquivo.](./assets/hero-demo-60s.gif)

[HD WebM](./assets/hero-demo-60s.webm) · _Instale ferramentas como apps. Fique com seu modelo. Fique com o arquivo._

---

## Como o LibrAgent é diferente

A maioria dos harnesses de agentes assume que você edita JSON de MCP, vive no terminal e monta orquestração em código (ou te prende à stack de um único fornecedor).

O LibrAgent é um **produto desktop** para o mesmo trabalho:

| Em vez de… | Você obtém… |
| ---------- | ----------- |
| Editar configs MCP à mão | **Extensions** — presets de um clique (GitHub, Brave Search, Filesystem, …) e importação do Cursor / VS Code / Claude Code / Windsurf |
| “Temos multiagente” | **Padrões de coordenação nomeados** como skills empacotados — `pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, … |
| Modelo + ferramentas de um provedor | **Seu** LLM (chave de API ou [Ollama](https://ollama.com)) e **sua** stack MCP — MIT, local-first |

[Baixar a versão mais recente](https://github.com/fritzprix/libr-agent/releases/latest) · [Onboarding em 5 minutos](#onboarding-em-5-minutos) · [Especificação da demo hero](docs/contributing/hero-demo-spec.md)

---

## O que você pode fazer nos primeiros 10 minutos

### 1. Ferramentas em um clique, depois um entregável

- Abra **Extensions** e instale um preset (ex.: GitHub) — sem JSON
- Aponte **Workspace** para uma pasta real
- Peça: _"Revise este repo quanto ao maior risco para um novo contribuinte e salve `DELIVERABLE.md`"_

### 2. Implantar uma receita de workflow em um clique

- Inicie a receita **Morning Briefing** na home do Chat ou em [Scheduled Tasks](docs/user/guides/scheduled-tasks.md)
- Instala presets Hacker News + Yahoo Finance, configura um assistente e agenda uma execução diária às 9h
- Acorde com um briefing sintetizado de tech e mercados — sem supervisão

### 3. Escolher um padrão de coordenação (sem montar framework)

- Diga a forma do trabalho, ou anexe um skill pelo nome:
  - _"@skill:pipeline — pesquisar, depois redigir, depois revisar; deixar um relatório final"_
  - _"@skill:divide-conquer — divida em peças independentes e una os resultados"_
- Os padrões são skills produtizados — não um SDK que você mesmo conecta. Veja [Sub-agents & orchestration](docs/user/guides/sub-agents.md).

### 4. Manter a liberdade do modelo

- Nuvem: cole uma chave de API OpenAI / Anthropic / Gemini / Groq
- Local: `ollama pull qwen3:14b` e selecione Ollama — o mesmo harness nos dois casos

---

## Três promessas do produto

1. **Superfície sem dever de casa do harness** — GUI, Extensions de um clique, receitas, aprovações in-app, `@skill:` — não “abra primeiro uma config e um shell”.
2. **Orquestração como produto** — escolha fluxos Sequential / Hub-and-spoke / estilo Swarm via skills; cresça para `teamwork` → `org` e `schedule` quando precisar de equipes duráveis ou cron — ainda sem montar LangGraph/CrewAI você mesmo.
3. **Liberdade de provedor e stack** — qualquer LLM suportado, MCP como infraestrutura, importar configs MCP de IDE existentes, licença MIT, workspaces e estado do navegador locais por padrão.

**Melhor encaixe:** operadores e power users que querem profundidade de harness sem viver em JSON; desenvolvedores que recusam uma stack de agentes de um único fornecedor; pesquisadores que precisam de navegador + conhecimento + agendas em um só produto.

---

## Padrões de coordenação (skills empacotados)

Escolha o modelo pela **forma do trabalho**, depois execute no chat:

| Skill | Padrão | Quando usar |
| ----- | ------ | ----------- |
| `pipeline` | Etapas sequenciais | Saídas alimentam o próximo passo (pesquisa → rascunho → revisão) |
| `hub-spoke` | Hub-and-spoke | Um coordenador integra muitos workers |
| `divide-conquer` | Divisão paralela | Peças independentes, depois merge |
| `consensus-delegation` | Multiperspectiva | Mesma pergunta a vários especialistas, depois reconciliar |
| `gatekeeper` / `pair-programming` | Loops de revisão | Revisão estrita ou coding com dois agentes |
| `delegate` | Handoff leve | Uma sessão filha, linhagem rastreada |
| `teamwork` → `org` | Equipe durável | Constituição compartilhada + Org UI |
| `schedule` / `loop` / `call-me-back` | Tempo e eventos | Cron, atrasos na sessão, ou retomar por processo/webhook |

Heurísticas de seleção: [framework-selection](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · guia completo: [Sub-agents](docs/user/guides/sub-agents.md).

Outros skills do primeiro dia: `setup-wizard`, `tool-installer`, `playbook-creator` e mais — **[Bundled Skills](docs/user/guides/skills.md)**.

---

## Plataforma MCP (ainda capaz para power users)

- Transportes: stdio, HTTP, SSE, OAuth 2.1
- 15+ servidores embutidos (Workspace, Shell, Browser, Knowledge, Planning, Scheduled Tasks, …)
- Presets de um clique + instalação assistida por agente (`tool-installer`)
- Isolamento de ferramentas por sessão; guards de caminho/comando; modos YOLO / unsafe opcionais para automação

### Substrato de execução

| Substrato | Capacidades |
| --------- | ----------- |
| **Workspace** | Edição linha a linha, ops multiarquivo, contexto `@file` / `@skill` / `@playbook` |
| **Shell** | Shells isolados e persistentes com monitoramento de processos async |
| **Browser** | Sidecar de navegador isolado; perfis de login salvos opcionais |
| **Knowledge** | Conhecimento em grafo + busca BM25 |
| **Export** | Relatórios Markdown e exports de trajetória ATIF ([session export](docs/user/guides/session-export.md)) |

Sessões longas permanecem produtivas via compactação de contexto, prevenção de loops, circuit breakers e guards de respostas obsoletas.

---

## Cenários do mundo real

### Operador — do app vazio ao briefing diário

1. Execute a receita **Morning Briefing** (presets + assistente + agenda 9h)
2. Clique em **Run now** uma vez para verificar
3. Deixe rodar — o relatório chega sem abrir um terminal

### Desenvolvedor solo — preset, não arquivos de config

1. Extensions → instale o preset GitHub MCP
2. Anexe um repo local via Workspace
3. Peça um relatório Markdown de segurança/revisão que você guarda no disco

### Power user sem framework — orquestração nomeada

1. `@skill:pipeline` (ou `hub-spoke` / `divide-conquer`) para a forma do trabalho
2. Agentes coordenam sob esse padrão
3. Um entregável mesclado no workspace — sem biblioteca de orquestração para manter

### Equipe sensível à privacidade — mesmo produto, modelo local

1. `ollama pull qwen3:14b`
2. Workspace + Shell ficam na máquina
3. Troque para chaves cloud depois se quiser — o harness não troca de fornecedor por você

---

## Documentação

- **[User Guide](docs/user/README.md)** — instalação, primeiro chat, modelos, skills ([docs site](https://fritzprix.github.io/libr-agent/))
- **[Hero Demo Spec](docs/contributing/hero-demo-spec.md)** — demo canônica do produto (legendas EN/KO/ZH)
- **[Product Messaging Guide](docs/contributing/product-messaging-guide.md)** — posicionamento e copy
- **[Recipes](docs/user/guides/recipes.md)** · **[Scheduled Tasks](docs/user/guides/scheduled-tasks.md)** · **[Sub-agents](docs/user/guides/sub-agents.md)** · **[Skills](docs/user/guides/skills.md)**
- **[HTTP API](docs/api/http_api.md)** — controle remoto e aprovações programáticas
- **[Architecture](docs/architecture/agent-workflow-architecture.md)** — isolamento de sessão e Think-Act-Observe

---

## Começando

Baixe o instalador mais recente na **[página de Releases](https://github.com/fritzprix/libr-agent/releases/latest)**.

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.21_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_x64-setup.exe) · [`LibrAgent_0.9.21_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.21_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.21_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_amd64.AppImage) · [`LibrAgent_0.9.21_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_amd64.deb) · [`LibrAgent-0.9.21-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent-0.9.21-1.x86_64.rpm)
- **All release assets:** [Releases page](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.21)
<!-- RELEASE_DOWNLOADS_END -->

### Onboarding em 5 minutos

**Passo 1 — Conectar um modelo** (Settings → LLM Providers)

- Nuvem: cole uma chave de API OpenAI / Anthropic / Gemini / Groq
- Local: `ollama pull qwen3:14b`, depois selecione Ollama em Settings

**Passo 2 — Adicionar ferramentas sem JSON**

- Extensions → instale um preset (ex.: GitHub), **ou**
- Diga a um agente: _"Importe meus servidores MCP do Cursor"_

**Passo 3 — Anexar um workspace e pedir um arquivo que você guarda**

- Aponte Workspace para uma pasta de projeto real
- _"Revise este workspace e escreva os achados em `DELIVERABLE.md`."_

**Em seguida — coordenação e automação**

- _"@skill:pipeline — pesquisar, redigir, depois revisar; um relatório final."_
- _"Prepare um workspace teamwork para este repo."_
- _"Configure um brief diário de concorrentes às 7h."_ (ou execute a receita Morning Briefing)

### Primeiros prompts para copiar e colar

- _"Importe meus servidores MCP do Cursor e mostre o que foi adicionado."_
- _"Instale o preset GitHub MCP e anexe a um agente de coding."_
- _"Revise este workspace e escreva os achados em `DELIVERABLE.md`."_
- _"@skill:pipeline — pesquise este tópico, redija um resumo, depois revise; salve o relatório final."_
- _"Configure um brief diário de concorrentes às 7h."_

### Setup de desenvolvedor

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## Onde o LibrAgent se encaixa melhor

| Se você quer… | O LibrAgent é forte porque… |
| ------------- | --------------------------- |
| **Profundidade de harness sem dever de casa** | Presets de Extensions, receitas, padrões `@skill:` e aprovações — não onboarding JSON-first |
| **Orquestração sem construir um framework** | `pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, `teamwork` / `org`, `schedule` chegam como produto |
| **Liberdade de uma stack de agentes de fornecedor** | Traga seu modelo e ferramentas MCP; MIT; local-first por padrão |
| **Um substrato de execução real** | Workspace, shell, browser, knowledge, playbooks e guards de sessões longas |
| **Um produto desktop nativo de MCP** | Presets, importação e 15+ builtins — não um wrapper fino de chat |

---

## Filosofia de design

- **Produto sobre kit**: o harness é utilizável sem montá-lo.
- **Orquestração como skills**: padrões de coordenação são nomeados, selecionáveis e documentados — não enterrados em repos de exemplo.
- **Liberdade de stack**: modelos e ferramentas são escolhas do usuário; o produto não exige um fornecedor de IA.
- **Local First**: workspaces, sessões, skills e estado do navegador ficam sob seu controle. Cloud LLM / MCP remoto só quando você opta.
- **Harness sobre modelo**: ferramentas, estado de sessão, delegação e governança importam mais do que qualquer modelo único.
- **Estabilidade sobre features**: isolamento, compactação, prevenção de loops — antes da corrida por recursos.
- **Padrões abertos**: MIT. MCP como camada de interoperabilidade.

---

## Contribuição e licença

O LibrAgent tem licença MIT e é construído em aberto. Contribuições são bem-vindas — skills empacotados, integrações MCP, correções de bugs ou melhorias de arquitetura.

- 📖 [Contributing Guide](CONTRIBUTING.md)
- 🐛 [Issue Tracker](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- 💬 [Discussions](https://github.com/fritzprix/libr-agent/discussions)
- 🧪 Benchmarks (Harbor / Terminal-Bench): veja [Harbor guide](benchmarks/harbor/README.md) (`pnpm bench:diverse`, `pnpm bench:terminal`, …)

**License**: MIT
