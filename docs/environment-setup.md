# 🔧 Environment Setup Guide

## 🔑 API Keys Setup

TauriAgent supports multiple AI providers. You need at least one API key to get started.

### 1. Copy Environment Template

```bash
cp .env.example .env
```

### 2. Get API Keys

#### 🟢 Groq (Recommended - Free & Fast)

1. Visit [Groq Console](https://console.groq.com/keys)
2. Sign up for free account
3. Create new API key
4. Copy key to `VITE_GROQ_API_KEY` in `.env`

**Why Groq?**

- ⚡ Extremely fast inference (10x faster than OpenAI)
- 🆓 Generous free tier
- 🦙 Latest Llama models
- 🔥 Perfect for development

#### 🔵 OpenAI (Premium)

1. Visit [OpenAI Platform](https://platform.openai.com/api-keys)
2. Create API key
3. Add to `VITE_OPENAI_API_KEY` in `.env`

#### 🟠 Anthropic Claude (Premium)

1. Visit [Anthropic Console](https://console.anthropic.com/)
2. Create API key
3. Add to `VITE_ANTHROPIC_API_KEY` in `.env`

### 3. Configure Your .env File

```bash
# Required: At least one AI API key
VITE_GROQ_API_KEY=gsk_your_actual_groq_key_here

# Optional: Additional providers
VITE_OPENAI_API_KEY=sk-your_openai_key_here
VITE_ANTHROPIC_API_KEY=sk-ant-your_claude_key_here

# Optional: Model settings
VITE_DEFAULT_MODEL=llama-3.1-8b-instant
VITE_AI_TEMPERATURE=0.7
VITE_AI_MAX_TOKENS=4096
```

## 🚀 Quick Start

1. **Setup environment:**

   ```bash
   cp .env.example .env
   # Edit .env with your API keys
   ```

2. **Install dependencies:**

   ```bash
   pnpm install
   ```

3. **Run development:**
   ```bash
   pnpm tauri dev
   ```

## 🎯 Model Recommendations

### For Development & Testing

- **Groq + Llama 3.1 8B**: Ultra-fast, free, great for testing
- **Model**: `llama-3.1-8b-instant`

### For Production

- **Groq + Llama 3.1 70B**: Balanced performance and cost
- **Model**: `llama-3.1-70b-versatile`

### For Complex Tasks

- **OpenAI GPT-4**: Best reasoning, higher cost
- **Model**: `gpt-4-turbo-preview`

### For Long Conversations

- **Anthropic Claude**: Excellent context handling
- **Model**: `claude-3-sonnet-20240229`

## 🔒 Security Notes

- ⚠️ **Never commit `.env` files to git**
- 🔐 Keep API keys private
- 🔄 Rotate keys regularly
- 📝 Use different keys for dev/prod

## 🐛 Troubleshooting

### "No API key found"

- Check `.env` file exists
- Verify `VITE_` prefix on all variables
- Restart development server after adding keys

### "Model not available"

- Check API key has access to selected model
- Try default model first: `llama-3.1-8b-instant`
- Verify API key permissions

### "Network errors"

- Check internet connection
- Verify API quotas/billing
- Try different AI provider

## 💡 Pro Tips

1. **Start with Groq**: Free, fast, reliable
2. **Use multiple providers**: Fallback for reliability
3. **Monitor usage**: Track API costs
4. **Experiment with models**: Different models for different tasks

---

**Need help?** Check our [Issues](https://github.com/fritzprix/tauri-agent/issues) or create a new one!
