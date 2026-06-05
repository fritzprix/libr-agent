# 🤖 LibrAgent

> **Uma app de desktop local-first para agentes de IA que usam ferramentas reais, trabalham em paralelo e ficam sob o teu controlo.**
> _Liga qualquer LLM, adiciona qualquer servidor MCP e deixa os agentes ler ficheiros, executar shells, navegar na web e concluir automatizações a sério._

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent é um **workspace de agentes local-first** construído com Tauri + Rust + React. Não é só mais uma interface de chat: foi pensado para acesso real a ficheiros, execução de shell, automatização do navegador, extensibilidade MCP e workflows multi-agente que aguentam horas de trabalho em vez de cair depois de uma demo bonita.

Podes ligar modelos cloud ou runtimes locais como Ollama, importar servidores MCP das ferramentas que já usas e depois deixar os agentes inspecionar código, editar ficheiros, executar comandos, navegar na web, capturar conhecimento e delegar subtarefas — sem mandar o teu workflow inteiro para a VM cloud de outra pessoa.

**Começa aqui:** [Descarregar a release mais recente](https://github.com/fritzprix/libr-agent/releases/latest) · [Ir para o percurso de integração de 5 minutos](#o-percurso-de-integração-de-5-minutos) · [Ver cenários reais](#-cenários-reais)

---

## Porquê LibrAgent?

A maioria dos produtos de agentes continua a impor um compromisso irritante:

- **UI fácil, mas execução fraca**
- **Automatização forte, mas experiência de produto pobre**
- **Conforto cloud, mas pouco controlo sobre a privacidade**
- **Framework flexível, mas a stack toda és tu que a montas**

O LibrAgent aponta ao meio-termo que as pessoas realmente querem:

- **Controlo local-first** sobre ficheiros, workspaces, sessões e estado do navegador
- **Extensibilidade aberta via MCP** em vez de uma história fechada de plugins
- **Capacidade real de execução** em shell, browser, workspace e knowledge
- **Uma GUI que pessoas normais conseguem usar** sem perder profundidade de power user
- **Um caminho natural de um agente para vários** quando um único assistente já não chega

### Para quem é o LibrAgent

- **Programadores solo** que querem agentes capazes de realmente ler, editar, executar, navegar e persistir contexto localmente
- **Utilizadores avançados e operadores** que querem compor o seu próprio stack a partir de modelos locais, fornecedores de API, servidores MCP e fluxos de trabalho agendados
- **Investigadores e analistas** que precisam de automatização do navegador, captura de conhecimento, playbooks repetíveis e sessões de longa duração
- **Equipas preocupadas com privacidade** que querem execução local, governança explícita e um caminho de um único agente para uma organização coordenada

---

## 🎬 A plataforma em ação

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

_De um único agente a um enxame coordenado — delegação recursiva, ferramentas MCP e espaço de trabalho persistente num substrato unificado._

---

## O que podes fazer nos primeiros 10 minutos

### 1. Rever um repositório com ferramentas reais

- Liga um repositório local com a ferramenta Workspace
- Adiciona o preset GitHub MCP
- Pede: _"Encontra problemas de segurança na PR #42 e guarda o relatório"_

### 2. Montar uma stack de agentes totalmente local

- Corre `ollama pull qwen3:14b`
- Liga Workspace + Shell
- Deixa um agente ler, alterar, testar e iterar sem enviar o teu código para uma VM cloud

### 3. Transformar pesquisa num workflow repetível

- Adiciona Browser + Knowledge
- Pede: _"Segue estes 5 blogs de concorrentes e resume-me isto todas as manhãs"_
- Converte uma tarefa pontual numa pipeline agendada

### 4. Passar de um assistente para uma equipa a sério

- Cria especialistas com `specialist-creator`
- Divide o trabalho com `delegate`
- Transforma colaboração recorrente num workspace partilhado `teamwork` ou `org`

---

## Porque aguenta depois da demo

### 1. 🔐 Segurança local-first — Os teus dados ficam na tua máquina

O LibrAgent trata a segurança como uma preocupação arquitetónica de primeira classe:

- **Isolamento de sessão**: Cada sessão de agente recebe a sua própria instância dedicada `MCPServiceProxy` — zero fugas de dados entre sessões
- **SecurityValidator integrado**: Ataques de traversal de caminhos e injeção de comandos bloqueados ao nível do sistema
- **Nenhum substrato cloud necessário**: A execução principal acontece localmente; as ligações externas ficam sobretudo limitadas aos fornecedores LLM cloud e aos serviços remotos MCP/HTTP que escolhas usar, além das verificações de atualização em builds de produção
- **Suporte offline completo**: Combina com [Ollama](https://ollama.ai) para um stack de agentes completamente isolado

#### O que fica local vs o que sai da tua máquina

- **Sempre local**: espaços de trabalho, ficheiros locais, competências agrupadas, estado de sessão, configs de servidores MCP, estado do navegador e execução de ferramentas locais
- **Sai da tua máquina quando necessário**: pedidos a fornecedores LLM cloud ou serviços MCP/HTTP remotos que configuras explicitamente, além das verificações de atualização em builds de produção
- **Modo offline completo**: usa Ollama ou outro runtime local com servidores MCP locais para um fluxo de trabalho isolado

### 2. 🧩 Ecossistema nativo MCP — Extensibilidade infinita por design

MCP (Model Context Protocol) é o padrão aberto por trás do modelo de extensibilidade do LibrAgent. O LibrAgent trata-o não como uma funcionalidade — mas como a espinha dorsal arquitetónica:

- **Suporte completo de transportes**: stdio, HTTP, SSE e OAuth 2.1 — a especificação completa
- **12+ servidores integrados**: Planning, Knowledge (RAG), Browser Automation, Workspace, Shell Execution, Content Store, e mais
- **Catálogo de presets**: Instala GitHub, Brave Search, Filesystem e outros servidores populares com um clique
- **Instâncias isoladas por sessão**: Cada sessão de agente tem estado de servidor MCP independente — sem interferência entre agentes paralelos
- **Importar de qualquer lugar**: Migra configs MCP automaticamente do Cursor, VS Code, Claude Code ou Windsurf

### 3. 🦾 Substrato de execução de nível produção

A maioria das ferramentas de IA é impressionante em demos e frágil em produção. O LibrAgent é obsessivamente desenhado para trabalho real e duradouro:

| Substrato     | Capacidades                                                                                                                 |
| ------------- | --------------------------------------------------------------------------------------------------------------------------- |
| **Workspace** | Edição precisa à linha, operações multi-ficheiro, pesquisa unificada, injeção de contexto `@file`/`@skill`/`@playbook`      |
| **Shell**     | Execução isolada E shells persistentes — monitorização de processos assíncrona (`poll`, `read output`, `list`)              |
| **Browser**   | Automatização de navegador headless com um modelo de interação ao estilo do Playwright e garantias de consistência de cache |
| **Knowledge** | Gestão de conhecimento baseada em grafos com extração entidade/relação (v2), pesquisa de texto completo BM25                |

**Engenharia de fiabilidade incluída**: Compactação de contexto, prevenção de ciclos, disjuntores e proteções contra respostas obsoletas mantêm os agentes produtivos em sessões que duram horas.

### 4. 🤝 Enxame → Equipa → Organização: Multi-agente em todas as escalas

O LibrAgent tem uma história multi-agente coerente desde a execução solo até à coordenação organizacional explícita:

- **`delegate`**: Agentes pai geram, informam e monitorizam sessões filhas com rastreamento de linhagem explícito
- **`teamwork`**: Constrói um espaço de trabalho completo de task-force (agents.md, MISSION.md, KANBAN.md) com um único comando
- **`org`**: Formaliza equipas com identidade de organização duradoura, retoma de sessão raiz e hierarquia de membros visível
- **`schedule`**: Automatização baseada em CRON — agentes executam sem supervisão, segundo um calendário, com constituição de espaço de trabalho
- **Concurrency Gate**: Limites rígidos em sessões paralelas e processos shell para prevenir deadlocks e custos descontrolados

### 5. ⚡ Competências agrupadas — A forma mais rápida de ir de uma instalação vazia a um enxame operacional

O LibrAgent vem com uma biblioteca crescente de **Competências agrupadas**. Não são prompts aleatórios — são procedimentos operativos reutilizáveis que qualquer agente pode invocar por nome.

As competências mais importantes para o primeiro dia:

| Competência          | O que faz                                                                                                         |
| -------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `system-setup`       | Deteta e instala runtimes em falta (Python, Node.js, uv) em todas as plataformas                                  |
| `mcp-installer`      | Regista servidores MCP a partir de pacotes npm, URLs do GitHub ou blocos de config JSON                           |
| `mcp-importer`       | Importa configs MCP existentes do Cursor, VS Code, Windsurf e similares                                           |
| `specialist-creator` | Desenha uma config de agente completa (prompt de sistema, modelo, ferramentas) a partir de uma descrição de papel |
| `crew-constructor`   | Analisa ferramentas disponíveis e cria automaticamente uma equipa de especialistas adequada                       |
| `agent-tooling`      | Audita agentes, deteta desajustes de capacidades e reequilibra dinamicamente as atribuições de ferramentas        |
| `delegate`           | Guia a transferência de sessão pai→filho com transferência de contexto explícita e rastreamento de linhagem       |
| `teamwork`           | Constrói a constituição do espaço de trabalho partilhado para trabalho multi-agente coordenado                    |
| `org`                | Formaliza identidade de organização duradoura e hierarquia de membros visível                                     |
| `schedule`           | Cria e gere grupos de tarefas agendadas recorrentes para automatização sem supervisão                             |
| `soul-awakening`     | Ancora um agente a uma persona `SOUL.md` — tom, postura, identidade                                               |

E isso é apenas a camada de operador. O LibrAgent também fornece competências de domínio para:

- **Conhecimento e investigação**: `deep-research-report`, `knowledge-distiller`
- **Fluxos de trabalho documentais**: `document-to-markdown`, `docx`, `pptx`
- **Criação de competências e workflows**: `skill-creator`, `skill-deployer`, `playbook-creator`, `mcp-builder`
- **Operações especializadas**: `computer-diagnosis` e outros assistentes especializados

_Importante: `bootstrap` é uma capacidade integrada frequentemente usada com estas competências. As Competências agrupadas são os procedimentos reutilizáveis; os integrados e as ferramentas MCP são o substrato de execução subjacente._

---

## 🌍 Cenários do mundo real

### Programador solo — Revisão de código automatizada

1. Conecta o teu repositório local através da ferramenta Workspace
2. Instala o preset GitHub MCP (um clique)
3. Pede: _"Encontra problemas de segurança no PR #42 e produz um relatório em Markdown"_
4. O agente lê o código, executa a análise, guarda os resultados no servidor Knowledge

### Profissional de marketing — Inteligência competitiva em piloto automático

1. Configura 5 blogs de concorrentes através da ferramenta Browser
2. Diz a um agente: _"Cria um briefing competitivo agendado todas as manhãs às 7h"_ — o agente pode usar a competência `schedule` para configurar o grupo de tarefas recorrente
3. O agente navega, resume e adiciona ao Knowledge store
4. Pergunta a qualquer momento: _"Resume os movimentos dos concorrentes da semana passada"_

### Equipa de engenharia — Stack de agentes offline

1. `ollama pull qwen3:14b` — sem chaves de API, sem nuvem
2. Conecta ferramentas Workspace + Shell à tua base de código
3. Propriedade intelectual sensível nunca sai da máquina
4. Agentes leem, modificam, testam e fazem commit — completamente local

### Utilizador avançado — Pipeline de investigação multi-agente

1. Usa `crew-constructor` para gerar automaticamente: Researcher×3, Analyst×1, Writer×1
2. O orquestrador delega em paralelo através da competência `delegate`
3. Os resultados fundem-se num único relatório estruturado no Content Store
4. Agenda todo o workflow semanalmente via `schedule`

---

## 📖 Documentação e guias

- **[Guia de navegação](docs/guides/navigation-guide.md)**: O hub Command & Control — `/assistants` (Definições de papéis) e `/playbooks` (Blueprints de workflow).
- **[Guia de arquitetura](docs/architecture/agent-workflow-architecture.md)**: Isolamento de sessão, motor de orquestração e o ciclo Think-Act-Observe orientado por Rust.
- **[Guia de ferramentas integradas](docs/guides/builtin_tool_bp.md)**: Padrões de design de ferramentas e padrões de resposta MCP.

---

## 📦 Começar

Descarrega o instalador mais recente para a tua plataforma na **[página de Releases](https://github.com/fritzprix/libr-agent/releases/latest)**.

<!-- RELEASE_DOWNLOADS_START -->

- **Windows:** [`LibrAgent_0.8.4_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.4/LibrAgent_0.8.4_x64-setup.exe) · [`LibrAgent_0.8.4_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.4/LibrAgent_0.8.4_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.8.4_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.4/LibrAgent_0.8.4_aarch64.dmg)
- **Linux:** [`LibrAgent_0.8.4_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.4/LibrAgent_0.8.4_amd64.AppImage) · [`LibrAgent_0.8.4_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.4/LibrAgent_0.8.4_amd64.deb) · [`LibrAgent-0.8.4-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.4/LibrAgent-0.8.4-1.x86_64.rpm)
- **Todos os artefatos da release:** [página de Releases](https://github.com/fritzprix/libr-agent/releases/tag/v0.8.4)
<!-- RELEASE_DOWNLOADS_END -->

**Configuração para programadores:**

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

### O percurso de integração de 5 minutos

**Passo 1 — Conecta um modelo** (Settings → LLM Providers)

- Nuvem: cola uma chave de API OpenAI / Anthropic / Gemini / Groq
- Local: `ollama pull qwen3:14b` e depois seleciona Ollama nas Settings
- Já usas Cursor ou VS Code? Diz a qualquer agente: _"Importa os meus servidores MCP do Cursor"_ → `mcp-importer` trata disso

**Passo 2 — Adiciona ferramentas MCP** (barra lateral Extensions)

- Navega no catálogo de presets e clica em Instalar, ou
- Diz a um agente: _"Install @modelcontextprotocol/server-everything"_ → `mcp-installer` regista-o automaticamente

**Passo 3 — Cria o teu primeiro agente**

- _"Cria um agente investigador para inteligência competitiva"_ → `specialist-creator` desenha a config completa
- _"Constrói uma equipa de investigação com as minhas ferramentas atuais"_ → `crew-constructor` cria os especialistas em lote
- _"Otimiza as atribuições de ferramentas para todos os meus agentes"_ → `agent-tooling` audita e reequilibra automaticamente

**Passo 4 — Vai em paralelo com `delegate`**

- Pede a qualquer agente que delegue subtarefas a sessões filhas
- A competência `delegate` gere a transferência de contexto, o rastreamento de linhagem e a fusão de resultados

**Passo 5 — Constrói uma equipa persistente**

- `teamwork` → constrói o espaço de trabalho partilhado com `agents.md`, `MISSION.md`, `KANBAN.md`
- `org` → formaliza a equipa com identidade duradoura e gestão de sessão raiz
- `schedule` → deixa um agente criar e gerir a automatização CRON para ti, sem supervisão

### Primeiros prompts para copiar e colar

- _"Importa os meus servidores MCP do Cursor e mostra-me o que foi adicionado."_
- _"Cria um agente investigador para inteligência competitiva com as minhas ferramentas atuais."_
- _"Instala o preset GitHub MCP e associa-o a um agente de codificação."_
- _"Delega a análise do repositório a uma sessão filha e traz-me um resumo."_
- _"Prepara um espaço de trabalho teamwork para este repositório e cria uma equipa de especialistas pronta para org."_
- _"Configura um briefing diário de concorrentes agendado para as 7h e mantém tudo no espaço de trabalho teamwork partilhado."_

---

## Onde o LibrAgent encaixa melhor

| Se queres...                                                     | O LibrAgent é forte porque...                                                         |
| ---------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| **uma workstation local de IA**                                  | ficheiros, sessões, workspaces e estado do navegador ficam na tua máquina por defeito |
| **um produto desktop realmente nativo MCP**                      | podes instalar, importar e gerir servidores MCP sem tratar a app como um wrapper fino |
| **agentes que fazem trabalho real**                              | Workspace, Shell, Browser e Knowledge foram pensados para execução prolongada         |
| **workflows multi-agente sem construir primeiro um framework**   | `delegate`, `teamwork`, `org` e `schedule` já fazem parte do produto                  |
| **equilíbrio entre GUI utilizável e profundidade de power user** | tens uma interface desktop sem perder extensibilidade nem controlo                    |

---

## Filosofia de design

- **Local First**: Os teus dados, chaves e "almas" de agentes ficam sob o teu controlo exclusivo. Nenhum substrato cloud necessário.
- **Arnês sobre Modelo**: O ambiente de execução — ferramentas, estado de sessão, delegação, governança — importa mais do que qualquer modelo individual. O LibrAgent é desenhado para maximizar o que qualquer modelo pode fazer.
- **Estabilidade sobre Funcionalidades**: O CHANGELOG reflete um foco obsessivo na correção do runtime — isolamento de sessão, compactação, prevenção de ciclos, proteções contra respostas obsoletas.
- **MCP como Infraestrutura**: Não um sistema de plugins. Todo o ecossistema de ferramentas está organizado em torno do MCP como a camada de interoperabilidade principal.
- **Padrões abertos**: Licença MIT. Totalmente comprometido com MCP, interoperabilidade open source e soberania dos dados dos utilizadores.

---

## Contribuição e Licença

O LibrAgent tem licença MIT e é desenvolvido em aberto. As contribuições são bem-vindas — sejam novas competências agrupadas, integrações MCP, correções de bugs ou melhorias arquitetónicas.

- 📖 [Guia de contribuição](CONTRIBUTING.md)
- 🐛 [Rastreador de problemas](https://github.com/fritzprix/libr-agent/issues)
- 💬 [Discussões](https://github.com/fritzprix/libr-agent/discussions)

**Licença**: MIT
