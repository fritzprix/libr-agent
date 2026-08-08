---
title: Connecting models
---

# Connecting models

> Set API keys and default models under **Settings → AI & Models**.

---

## What you will learn

1. How to open Settings  
2. Paste keys under **Provider API Keys**  
3. Choose **Default LLM** / **Fallback LLM**  
4. Add custom OpenAI-compatible providers  
5. What to check when requests fail  

---

## 1. Open Settings

**Sidebar → Settings**. Full page `/settings` stays open until you close it.

Top actions: **Discard** · **Close** · **Save Changes**

### Real tab names

| Tab | Purpose |
|-----|---------|
| **General** | Language, skills directory, … |
| **AI & Models** | API keys, default/fallback model, temperature |
| **Chat Interface** | Chat UI / context options |
| **System** | System options |
| **Advanced** | Advanced (shell runtime PATH, …) |
| **Experimental** | Experimental features |

> Names that **do not** exist: 「LLM Provider」 section, standalone 「AI Models」, 「Preferred Model」, Settings only as a top-right gear.

![Settings → AI & Models](../../assets/screenshots/getting-started/settings-ai-models.png)

---

## 2. Provider API Keys

1. Open **AI & Models**.  
2. Under **Provider API Keys**, pick a card (OpenAI, Anthropic, Google Gemini, Ollama, Groq, Fireworks AI, Cerebras, OpenRouter, …).  
3. Paste **API Key** (some cards also have **Base URL**).  
4. Click **Save Changes**.

Keys are stored locally. Never paste keys into chat.

### Where to get keys

| Provider | Portal |
|----------|--------|
| Anthropic | [console.anthropic.com](https://console.anthropic.com/) |
| OpenAI | [platform.openai.com/api-keys](https://platform.openai.com/api-keys) |
| Google Gemini | [aistudio.google.com](https://aistudio.google.com/) |
| Groq | [console.groq.com/keys](https://console.groq.com/keys) |

---

## 3. Model Preferences

Same **AI & Models** tab:

| Field | Meaning |
|-------|---------|
| **Default LLM** | Default provider/model for new sessions |
| **Fallback LLM** | Used when the default fails |
| **Override temperature** | When on, set **Temperature** yourself |

You can also change **Provider** / **Model** in Chat for the **current session only**.

---

## 4. Custom OpenAI providers

For OpenRouter, Ollama, etc., use **Custom OpenAI Providers → Add Custom OpenAI Provider**.

OpenRouter example:

| Field | Example |
|-------|---------|
| Display name | OpenRouter |
| Base URL | `https://openrouter.ai/api/v1` |
| API Key | your OpenRouter key |

Ollama example:

| Field | Example |
|-------|---------|
| Base URL | `http://localhost:11434/v1` |
| API Key | empty or `ollama` |

Local servers must already be running.

---

## 5. Troubleshooting

| Symptom | Check |
|---------|--------|
| Requests fail | Key / Base URL under **Provider API Keys**, and **Save Changes** |
| Empty model list | Save key, then **Refresh models** in the picker |
| Switch provider | Add key on that card, change **Default LLM** |
| Python/Node runtime | Not Settings — **Chat → App Wizard** + **setup-wizard** |

There is no Settings “connected / verifying” badge. Send a Chat message to verify.

More: [Troubleshooting](../guides/troubleshooting.md)

---

## Next

- [5-minute tutorial](5-minute-tutorial.md)  
- [First agent chat](first-agent.md)
