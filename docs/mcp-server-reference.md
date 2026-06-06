# MCP Server Configuration Reference

> Auto-generated from `mcp-server.json` — 20 registered servers across 7 categories.

---

## Table of Contents

- [Search](#search)
- [DevTools](#devtools)
- [Data](#data)
- [AI](#ai)
- [Documents](#documents)
- [Creative](#creative)
- [HTTP-Only Servers](#http-only-servers)

---

## Search

### `hn` — Hacker News

| Field           | Value                                                                |
| --------------- | -------------------------------------------------------------------- |
| **Category**    | search                                                               |
| **Transport**   | stdio                                                                |
| **Command**     | `npx -y @fre4x/hn`                                                   |
| **Description** | Read Hacker News — top, new, best, Ask HN, Show HN, and job stories. |
| **Logo**        | https://news.ycombinator.com/favicon.ico                             |

---

### `ddg-search` — DuckDuckGo

| Field           | Value                                      |
| --------------- | ------------------------------------------ |
| **Category**    | search                                     |
| **Transport**   | stdio                                      |
| **Command**     | `uvx duckduckgo-mcp-server`                |
| **Description** | Search the web privately using DuckDuckGo. |
| **Logo**        | https://duckduckgo.com/favicon.ico         |

---

### `brave-search` — Brave Search

| Field           | Value                                                            |
| --------------- | ---------------------------------------------------------------- |
| **Category**    | search                                                           |
| **Transport**   | stdio (Docker)                                                   |
| **Command**     | `docker run -i --rm -e BRAVE_API_KEY docker.io/mcp/brave-search` |
| **Description** | Search the web with Brave Search API.                            |
| **Logo**        | https://brave.com/favicon.ico                                    |
| **Env**         | `BRAVE_API_KEY` (required, password)                             |

---

### `exa` — Exa Search

| Field           | Value                                                |
| --------------- | ---------------------------------------------------- |
| **Category**    | search                                               |
| **Transport**   | HTTP                                                 |
| **URL**         | https://mcp.exa.ai/mcp                               |
| **Description** | AI-powered web search and content retrieval via Exa. |
| **Logo**        | https://exa.ai/favicon.ico                           |
| **Auth**        | `exaApiKey` via URL param (required, password)       |

---

### `arxiv` — arXiv Papers

| Field           | Value                                                                                      |
| --------------- | ------------------------------------------------------------------------------------------ |
| **Category**    | search                                                                                     |
| **Transport**   | stdio                                                                                      |
| **Command**     | `npx -y @fre4x/arxiv`                                                                      |
| **Description** | Search and retrieve academic papers from arXiv — millions of papers across all categories. |
| **Logo**        | https://arxiv.org/favicon.ico                                                              |

---

## DevTools

### `fre4x-inspector-bridge` — MCP Inspector Bridge

| Field           | Value                                                               |
| --------------- | ------------------------------------------------------------------- |
| **Category**    | devtools                                                            |
| **Transport**   | stdio                                                               |
| **Command**     | `npx -y --package @fre4x/inspector-bridge fre4x-inspector-bridge`   |
| **Description** | MCP bridge — connect and use external MCP servers within LibrAgent. |

---

### `context7` — Context7 Documentation

| Field           | Value                                                         |
| --------------- | ------------------------------------------------------------- |
| **Category**    | devtools                                                      |
| **Transport**   | HTTP                                                          |
| **URL**         | https://mcp.context7.com/mcp                                  |
| **Description** | Access up-to-date documentation for any library via Context7. |
| **Logo**        | https://context7.com/favicon.ico                              |
| **Auth**        | `CONTEXT7_API_KEY` via HTTP header (required, password)       |

---

### `serena` — Semantic Coding Agent

| Field           | Value                                                                     |
| --------------- | ------------------------------------------------------------------------- |
| **Category**    | devtools                                                                  |
| **Transport**   | stdio                                                                     |
| **Command**     | `uvx --from git+https://github.com/oraios/serena serena start-mcp-server` |
| **Description** | Semantic coding agent with symbol-level code understanding and editing.   |
| **Logo**        | https://oraios.github.io/serena/_static/serena-icon-32.png                |

---

### `github` — GitHub API

| Field           | Value                                                            |
| --------------- | ---------------------------------------------------------------- |
| **Category**    | devtools                                                         |
| **Transport**   | HTTP                                                             |
| **URL**         | https://api.githubcopilot.com/mcp/                               |
| **Description** | Interact with GitHub repositories, issues, PRs, and code search. |
| **Logo**        | https://github.com/favicon.ico                                   |
| **Auth**        | `GITHUB_PAT` via Bearer token (required, password)               |

---

### `jules` — Google Jules

| Field           | Value                                                    |
| --------------- | -------------------------------------------------------- |
| **Category**    | devtools                                                 |
| **Transport**   | stdio                                                    |
| **Command**     | `npx -y @fre4x/jules`                                    |
| **Description** | Google Jules — autonomous coding agent for GitHub tasks. |
| **Logo**        | https://jules.google.com/favicon.ico                     |
| **Env**         | `JULES_API_KEY` (required, password)                     |

---

### `benchmark` — GAIA Benchmark

| Field           | Value                                                                   |
| --------------- | ----------------------------------------------------------------------- |
| **Category**    | devtools                                                                |
| **Transport**   | stdio                                                                   |
| **Command**     | `npx -y @fre4x/benchmark`                                               |
| **Description** | Run GAIA benchmark challenges to evaluate AI agent capabilities.        |
| **Env**         | `BENCHMARK_GAIA_DATA_FILE` (required, text, absolute path to GAIA JSON) |

---

## Data

### `yahoo-finance` — Yahoo Finance

| Field           | Value                                                                                          |
| --------------- | ---------------------------------------------------------------------------------------------- |
| **Category**    | data                                                                                           |
| **Transport**   | stdio                                                                                          |
| **Command**     | `npx -y @fre4x/yahoo-finance`                                                                  |
| **Description** | Get real-time stock prices, financials, options chains, and insider data. No API key required. |
| **Logo**        | https://s.yimg.com/rz/p/yahoo_finance_en-US_h_p_finance_2.png                                  |

---

### `fred` — Federal Reserve Economic Data

| Field           | Value                                                                                                  |
| --------------- | ------------------------------------------------------------------------------------------------------ |
| **Category**    | data                                                                                                   |
| **Transport**   | stdio                                                                                                  |
| **Command**     | `npx -y @fre4x/fred`                                                                                   |
| **Description** | Access Federal Reserve economic data — GDP, inflation, unemployment, interest rates, and 800K+ series. |
| **Logo**        | https://fred.stlouisfed.org/favicon.ico                                                                |
| **Env**         | `FRED_API_KEY` (required, password)                                                                    |

---

## AI

### `gemini` — Google Gemini

| Field           | Value                                                                                             |
| --------------- | ------------------------------------------------------------------------------------------------- |
| **Category**    | ai                                                                                                |
| **Transport**   | stdio                                                                                             |
| **Command**     | `npx -y @fre4x/gemini`                                                                            |
| **Description** | Google Gemini — multimodal text generation, image/video analysis, and image synthesis via Imagen. |
| **Logo**        | https://www.gstatic.com/lamda/images/gemini_favicon_f069958c85030456e93de685481c559f160ea06.svg   |
| **Env**         | `GEMINI_API_KEY` (required, password)                                                             |

---

### `grok` — xAI Grok

| Field           | Value                                             |
| --------------- | ------------------------------------------------- |
| **Category**    | ai                                                |
| **Transport**   | stdio                                             |
| **Command**     | `npx -y @fre4x/grok`                              |
| **Description** | Grok API access for text generation and analysis. |
| **Logo**        | https://grok.ai/favicon.ico                       |
| **Env**         | `XAI_API_KEY` (required, password)                |

---

### `openai` — OpenAI

| Field           | Value                                               |
| --------------- | --------------------------------------------------- |
| **Category**    | ai                                                  |
| **Transport**   | stdio                                               |
| **Command**     | `npx -y @fre4x/openai`                              |
| **Description** | OpenAI MCP server for text generation and analysis. |
| **Env**         | `OPENAI_API_KEY` (required, password)               |

---

### `huggingface` — Hugging Face

| Field           | Value                                                             |
| --------------- | ----------------------------------------------------------------- |
| **Category**    | ai                                                                |
| **Transport**   | stdio                                                             |
| **Command**     | `npx -y @fre4x/huggingface`                                       |
| **Description** | Interact with Hugging Face models and datasets.                   |
| **Logo**        | https://huggingface.co/front/assets/huggingface_logo-noborder.svg |
| **Env**         | `HF_TOKEN` (required, password)                                   |
| **Env**         | `HF_MODEL_CACHE_DIR` (optional, text)                             |
| **Env**         | `HF_LOCAL_MODEL_PATH` (optional, text)                            |
| **Env**         | `HF_ALLOW_REMOTE_MODELS` (optional, text, default: `true`)        |

---

## Documents

### `docx` — Word Documents

| Field           | Value                                                       |
| --------------- | ----------------------------------------------------------- |
| **Category**    | documents                                                   |
| **Transport**   | stdio                                                       |
| **Command**     | `npx -y @fre4x/docx`                                        |
| **Description** | Work with Word documents programmatically through MCP APIs. |
| **Logo**        | https://www.docx.com/favicon.ico                            |

---

### `jupyter` — Jupyter Notebooks

| Field           | Value                                         |
| --------------- | --------------------------------------------- |
| **Category**    | documents                                     |
| **Transport**   | stdio                                         |
| **Command**     | `npx -y @fre4x/jupyter`                       |
| **Description** | Execute and manage Jupyter notebooks via MCP. |
| **Logo**        | https://jupyter.org/favicon.ico               |

---

## Creative

### `comfyui` — ComfyUI

| Field           | Value                                                                   |
| --------------- | ----------------------------------------------------------------------- |
| **Category**    | creative                                                                |
| **Transport**   | stdio                                                                   |
| **Command**     | `npx -y @fre4x/comfyui`                                                 |
| **Description** | Interact with ComfyUI to generate and manage AI images.                 |
| **Env**         | `COMFYUI_SERVER_URL` (required, text, default: `http://localhost:8188`) |

---

## HTTP-Only Servers

These servers use HTTP transport instead of stdio. They require HTTPS endpoints and in-band authentication.

| Server     | URL                                | Auth Target                 |
| ---------- | ---------------------------------- | --------------------------- |
| `context7` | https://mcp.context7.com/mcp       | `CONTEXT7_API_KEY` (header) |
| `github`   | https://api.githubcopilot.com/mcp/ | `GITHUB_PAT` (Bearer)       |
| `exa`      | https://mcp.exa.ai/mcp             | `exaApiKey` (URL param)     |

---

## Transport Summary

| Transport          | Count | Servers                                                                                                                               |
| ------------------ | ----- | ------------------------------------------------------------------------------------------------------------------------------------- |
| **stdio** (npx)    | 13    | hn, fre4x-inspector-bridge, yahoo-finance, serena, jules, benchmark, arxiv, gemini, grok, docx, jupyter, comfyui, openai, huggingface |
| **stdio** (uvx)    | 2     | ddg-search, serena                                                                                                                    |
| **stdio** (Docker) | 1     | brave-search                                                                                                                          |
| **HTTP**           | 3     | context7, github, exa                                                                                                                 |

## Credential Summary

| Variable                   | Required | Type     | Target       | Servers      |
| -------------------------- | -------- | -------- | ------------ | ------------ |
| `BRAVE_API_KEY`            | ✅       | password | env          | brave-search |
| `CONTEXT7_API_KEY`         | ✅       | password | header       | context7     |
| `GITHUB_PAT`               | ✅       | password | bearer-token | github       |
| `JULES_API_KEY`            | ✅       | password | env          | jules        |
| `exaApiKey`                | ✅       | password | url-param    | exa          |
| `FRED_API_KEY`             | ✅       | password | env          | fred         |
| `GEMINI_API_KEY`           | ✅       | password | env          | gemini       |
| `XAI_API_KEY`              | ✅       | password | env          | grok         |
| `HF_TOKEN`                 | ✅       | password | env          | huggingface  |
| `BENCHMARK_GAIA_DATA_FILE` | ✅       | text     | env          | benchmark    |
| `COMFYUI_SERVER_URL`       | ✅       | text     | env          | comfyui      |
| `HF_MODEL_CACHE_DIR`       | —        | text     | env          | huggingface  |
| `HF_LOCAL_MODEL_PATH`      | —        | text     | env          | huggingface  |
| `HF_ALLOW_REMOTE_MODELS`   | —        | text     | env          | huggingface  |
