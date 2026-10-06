<p align="center">
  <img src="./src-tauri/icons/128x128.png" width="96" alt="LibrAgent" />
</p>

# LibrAgent

> **Um ambiente desktop para agentes de IA executado localmente sob seu controle total.**
> Conecte o modelo de sua escolha, instale ferramentas com um clique e automatize tarefas com padrões comprovados de coordenação multiagente.
> _Sem bloqueio de fornecedor e sem necessidade de configurar JSON manualmente. Todo o trabalho é salvo diretamente em arquivos locais no seu computador._

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

![Demonstração principal — Instale ferramentas como aplicativos, escolha seu modelo e mantenha seus arquivos locais.](./assets/hero-demo-60s.gif)

[Ver vídeo HD em WebM](./assets/hero-demo-60s.webm) · _Instale ferramentas como aplicativos, escolha seu modelo e mantenha seus arquivos locais._

---

## O que torna o LibrAgent diferente

Muitos frameworks de agentes assumem que você domina a linha de comando, edita manualmente arquivos de configuração JSON complexos e programa a orquestração diretamente em código. Outras ferramentas prendem você ao ecossistema fechado de um único provedor.

O LibrAgent resolve isso como um **aplicativo desktop completo e pronto para uso**:

| Abordagem tradicional | Com o LibrAgent |
| --------------------- | --------------- |
| Editar manualmente configurações JSON de MCP | **Extensões com um clique** — Predefinições prontas (GitHub, Brave Search, sistema de arquivos) e importação direta do Cursor, VS Code e Claude Code |
| Programar sistemas multiagente em código | **Padrões de coordenação integrados** — Habilidades empacotadas prontas para uso (`pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`) |
| Vinculação obrigatória aos modelos de um só provedor | **Liberdade tecnológica total** — Use chaves de API na nuvem ou modelos locais com [Ollama](https://ollama.com) sob licença aberta MIT |

[Baixar a versão mais recente](https://github.com/fritzprix/libr-agent/releases/latest) · [Guia de início em 5 minutos](#guia-de-início-em-5-minutos) · [Especificações da demo](docs/contributing/hero-demo-spec.md)

---

## O que você pode fazer nos primeiros 10 minutos

### 1. Instalar ferramentas com um clique e gerar um relatório

- Abra **Extensions** e instale uma predefinição (como GitHub) sem precisar editar arquivos JSON.
- Associe o seu **Workspace** a uma pasta de projeto local.
- Envie a instrução: _"Analise este repositório para identificar os principais riscos para um novo colaborador e salve o relatório em `DELIVERABLE.md`"_

### 2. Executar uma receita de fluxo de trabalho automatizada

- Inicie a receita **Morning Briefing** a partir da tela inicial do chat ou em [Tarefas agendadas](docs/user/guides/scheduled-tasks.md).
- As ferramentas do Hacker News e Yahoo Finance são configuradas automaticamente para execução diária às 09:00.
- Receba automaticamente a cada manhã um resumo consolidado sobre inovações tecnológicas e mercados financeiros.

### 3. Escolher um padrão de coordenação sem programar

- Defina o fluxo de trabalho desejado diretamente no prompt:
  - _"@skill:pipeline — pesquise, redija um rascunho e revise; produza um único relatório final"_
  - _"@skill:divide-conquer — divida esta tarefa em etapas independentes em paralelo e unifique os resultados"_
- Os padrões de coordenação são habilidades integradas que não exigem a montagem de SDKs externos. Consulte o guia de [Subagentes e orquestração](docs/user/guides/sub-agents.md).

### 4. Manter controle absoluto dos seus modelos

- **Modelos na nuvem**: Cole uma chave de API para OpenAI, Anthropic, Gemini ou Groq.
- **Modelos locais**: Execute `ollama pull qwen3:14b` e selecione Ollama nas configurações para uma execução 100 % local e privada.

---

## Três compromissos fundamentais

1. **Interface gráfica sem burocracia de configuração** — Interface intuitiva, extensões com um clique, receitas automatizadas e aprovações no próprio app eliminam a necessidade de abrir terminais ou editar arquivos de configuração.
2. **Orquestração como recurso nativo** — Escolha fluxos sequenciais, em estrela (Hub-and-spoke) ou distribuídos (Swarm) por meio de habilidades e expanda com facilidade para equipes duradouras (`teamwork`) ou rotinas periódicas (`schedule`).
3. **Autonomia completa de modelos e dados** — Desenvolvido sobre o padrão aberto MCP (Model Context Protocol). Todos os espaços de trabalho, históricos de sessão e estados de navegação permanecem estritamente salvos no seu computador.

**Público-alvo:** Profissionais que buscam agentes robustos sem complicações técnicas; desenvolvedores que evitam o aprisionamento tecnológico; pesquisadores que necessitam de automação de navegador, bases de conhecimento e rotinas agendadas em um único aplicativo.

---

## Padrões de coordenação integrados (Habilidades empacotadas)

Selecione o padrão adequado para a estrutura da sua tarefa diretamente no painel de chat:

| Nome da habilidade | Padrão de coordenação | Caso de uso recomendado |
| ------------------ | --------------------- | ----------------------- |
| `pipeline` | Etapas sequenciais | O resultado de uma fase alimenta a seguinte (Pesquisa → Redação → Revisão) |
| `hub-spoke` | Hub-and-Spoke | Um agente coordenador central gerencia e integra múltiplos agentes especialistas |
| `divide-conquer` | Divisão paralela | Dividir o trabalho em subtarefas independentes em paralelo e consolidar |
| `consensus-delegation` | Multiperspectiva | Enviar a mesma questão para múltiplos agentes especialistas e reconciliar análises |
| `gatekeeper` / `pair-programming` | Ciclos de revisão | Revisão rigorosa de código ou programação colaborativa em dupla |
| `delegate` | Delegação pontual | Delegar uma tarefa específica para uma subsessão com rastreamento completo |
| `teamwork` → `org` | Equipe duradoura | Equipes colaborativas estáveis com diretrizes compartilhadas e organograma interativo |
| `schedule` / `loop` / `call-me-back` | Tempo e eventos | Agendamentos periódicos Cron, intervalos na sessão ou retomada por eventos externos |

Critérios de seleção: [Seleção de framework](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · Guia completo: [Subagentes](docs/user/guides/sub-agents.md)

Outras habilidades prontas para uso: `setup-wizard`, `tool-installer`, `playbook-creator` e mais — consulte **[Habilidades integradas](docs/user/guides/skills.md)**

---

## Infraestrutura central de execução MCP

- **Suporte amplo a protocolos**: stdio, HTTP, SSE e OAuth 2.1
- **Mais de 15 servidores integrados**: Workspace, Shell, Browser, Knowledge, Planning, Scheduled Tasks, entre outros
- **Predefinições com um clique e instalação guiada**: Configuração interativa de ferramentas assistida por agente via `tool-installer`
- **Ambiente seguro e isolado**: Isolamento de ferramentas por sessão, restrições de caminhos de arquivos e comandos, além de modos seguros para automação

### Capacidades centrais de execução

| Área de execução | Recursos e funcionalidades |
| ---------------- | -------------------------- |
| **Workspace** | Edição precisa de código linha por linha, operações em múltiplos arquivos, injeção de contexto via `@file` / `@skill` / `@playbook` |
| **Shell** | Ambientes de terminal isolados e persistentes com monitoramento assíncrono de processos |
| **Browser** | Navegador em sandbox dedicado e ponte de extensão para interagir com suas sessões habituais do Chrome |
| **Knowledge** | Grafo de conhecimento e motor de busca híbrida BM25 de alta performance |
| **Export** | Relatórios estruturados em Markdown e exportação padronizada de trajetórias ATIF ([Exportação de sessões](docs/user/guides/session-export.md)) |

A compactação automática de contexto, a prevenção de loops infinitos e a validação de respostas asseguram um desempenho estável mesmo em sessões prolongadas.

---

## Casos de uso práticos

### Operador de negócios — Relatório matinal automatizado

1. Execute a receita **Morning Briefing** (configuração de ferramentas, assistente e agendamento diário às 09:00).
2. Clique em **Run now** para validar o fluxo imediatamente.
3. Acompanhe a cada manhã relatórios consolidados sobre tecnologia e mercado sem precisar abrir um terminal.

### Desenvolvedor autônomo — Integração imediata de ferramentas

1. Instale o preset de GitHub MCP no menu Extensions.
2. Associe um repositório local de código por meio do Workspace.
3. Solicite ao agente um relatório completo de revisão de segurança e arquitetura salvo localmente.

### Usuário avançado — Delegação com modelos padronizados

1. Digite `@skill:pipeline` (ou `hub-spoke`, `divide-conquer`) no chat.
2. Os agentes colaboram de forma coordenada conforme o padrão escolhido.
3. Obtenha um resultado integrado no seu espaço de trabalho sem precisar manter bibliotecas externas.

### Equipes com foco em privacidade — Operação 100 % local

1. Execute `ollama pull qwen3:14b`.
2. Todas as tarefas em Workspace e Shell são processadas exclusivamente na sua máquina.
3. Alterne para modelos na nuvem quando desejar, mantendo exatamente o mesmo fluxo de uso.

---

## Documentação oficial

- **[Guia do Usuário (User Guide)](docs/user/README.md)** — Instalação, primeiros passos, modelos e habilidades ([Site de documentação](https://fritzprix.github.io/libr-agent/))
- **[Especificações da demo (Hero Demo Spec)](docs/contributing/hero-demo-spec.md)** — Especificações do vídeo de demonstração (legendas em EN/KO/ZH)
- **[Guia de comunicação do produto](docs/contributing/product-messaging-guide.md)** — Diretrizes de posicionamento e terminologia
- **[Receitas](docs/user/guides/recipes.md)** · **[Tarefas agendadas](docs/user/guides/scheduled-tasks.md)** · **[Subagentes](docs/user/guides/sub-agents.md)** · **[Habilidades](docs/user/guides/skills.md)**
- **[API HTTP](docs/api/http_api.md)** — Controle remoto e interfaces programáticas de aprovação
- **[Arquitetura do sistema](docs/architecture/agent-workflow-architecture.md)** — Isolamento de sessão e ciclo Think-Act-Observe

---

## Primeiros passos

Baixe o instalador mais recente na **[Página de versões (Releases)](https://github.com/fritzprix/libr-agent/releases/latest)**.

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.23_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.23/LibrAgent_0.9.23_x64-setup.exe) · [`LibrAgent_0.9.23_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.23/LibrAgent_0.9.23_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.23_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.23/LibrAgent_0.9.23_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.23_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.23/LibrAgent_0.9.23_amd64.AppImage) · [`LibrAgent_0.9.23_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.23/LibrAgent_0.9.23_amd64.deb) · [`LibrAgent-0.9.23-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.23/LibrAgent-0.9.23-1.x86_64.rpm)
- **Todos os artefatos da release:** [página de Releases](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.23)
<!-- RELEASE_DOWNLOADS_END -->

### Guia de início em 5 minutos

**Etapa 1 — Conectar um modelo** (Settings → LLM Providers)

- Modelos em nuvem: Cole sua chave de API para OpenAI, Anthropic, Gemini ou Groq.
- Modelos locais: Execute `ollama pull qwen3:14b` e selecione Ollama nas configurações.

**Etapa 2 — Adicionar ferramentas sem configurar JSON**

- Instale uma predefinição (ex. GitHub) no menu Extensions, ou
- Diga ao agente: _"Importe meus servidores MCP do Cursor"_

**Etapa 3 — Vincular um espaço de trabalho e gerar um arquivo**

- Selecione a pasta do seu projeto local no Workspace.
- Peça ao agente: _"Analise este espaço de trabalho e registre os resultados em `DELIVERABLE.md`"_

**Próximos passos — Coordenação avançada e automação**

- _"@skill:pipeline — pesquise este tópico, elabore um resumo, revise-o e salve o relatório final"_
- _"Configure um espaço de trabalho teamwork para este repositório"_
- _"Crie uma tarefa agendada diária de análise de concorrentes às 07:00"_ (ou utilize a receita Morning Briefing)

### Exemplos de comandos prontos para uso

- _"Importe as configurações dos meus servidores MCP do Cursor e mostre quais ferramentas foram adicionadas."_
- _"Instale o preset do GitHub MCP e vincule-o a um agente de desenvolvimento."_
- _"Revise este espaço de trabalho e escreva recomendações práticas de melhoria em `DELIVERABLE.md`."_
- _"@skill:pipeline — aprofunde o estudo deste tema técnico, redija um resumo estruturado e produza um relatório final revisado."_
- _"Agende uma rotina diária para compilar um panorama do mercado todas as manhãs às 07:00."_

### Configuração para desenvolvedores

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## Filosofia de design

- **Produto completo em vez de kit para montar**: Pronto para utilização imediata sem necessidade de programar fluxos complexos.
- **Orquestração como habilidades bem estruturadas**: Padrões de colaboração documentados e padronizados como competências claras.
- **Liberdade absoluta de componentes**: Você decide quais modelos e ferramentas utilizar sem imposição de fornecedores.
- **Prioridade ao local (Local First)**: Espaços de trabalho, históricos de sessão e dados de navegação continuam sob sua gestão local.
- **O ambiente de execução acima do modelo**: Uma integração robusta de ferramentas, controle de estado e governança entregam resultados mais consistentes do que qualquer modelo isolado.
- **Estabilidade acima do acúmulo de recursos**: O isolamento de contexto, a compressão de memória e a prevenção de loops são prioridades validadas.
- **Adesão firme a padrões abertos**: Licença permissiva MIT adotando o protocolo MCP (Model Context Protocol) como alicerce de interoperabilidade.

---

## Contribuição e licença

O LibrAgent é um projeto de código aberto sob licença MIT. Toda contribuição (habilidades, integrações MCP, correções de bugs ou melhorias de arquitetura) é muito bem-vinda.

- [Guia de contribuição (Contributing Guide)](CONTRIBUTING.md)
- [Rastreador de problemas (Issue Tracker)](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- [Fórum de discussões (Discussions)](https://github.com/fritzprix/libr-agent/discussions)
- Testes de desempenho (Harbor / Terminal-Bench): consulte o [guia do Harbor](benchmarks/harbor/README.md) (`pnpm bench:diverse`, `pnpm bench:terminal`, …)

**Licença**: MIT
