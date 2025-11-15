# Model Capability Detection Architecture

## Overview

LibrAgent uses a dynamic, multi-tier approach to detect model capabilities (thinking/reasoning support, tool use, context windows) without maintaining static configuration files.

## Strategy

### 1. **API-First Approach** (Primary)

Query provider APIs for real-time model metadata:

- **Ollama**: `/api/show` endpoint provides model Modelfile and parameters
- **OpenAI**: `/v1/models/{model}` endpoint (limited metadata)
- **Anthropic**: Models API (limited metadata)
- **Gemini**: Models API with capability fields

### 2. **Minimal Fallback Patterns** (Secondary)

Only the most stable, popular model families:

- Ollama: `qwen`, `deepseek`
- OpenAI: `o1`, `o3`, `o4`
- Anthropic: `claude-3.5`
- Gemini: `gemini-2`

### 3. **Caching Layer** (Performance)

- **In-Memory Cache**: Map<string, CapabilityCache>
- **TTL**: 24 hours
- **Cache Key**: `${provider}:${modelName}`

### 4. **User Override** (Future)

Settings UI to manually configure model capabilities.

## Implementation Example

### Ollama /api/show Detection

```typescript
const response = await fetch('http://localhost:11434/api/show', {
  method: 'POST',
  body: JSON.stringify({ name: 'qwen2.5:latest' }),
});

const data = await response.json();

// Check modelfile for thinking parameter
const hasThinking =
  data.modelfile?.includes('think') || data.parameters?.think !== undefined;
```

### Response Example

```json
{
  "modelfile": "FROM qwen2.5:latest\nPARAMETER temperature 0.7\n...",
  "parameters": {
    "num_ctx": 32768,
    "think": true,
    "temperature": 0.7
  },
  "template": "{{ if .Thinking }}...",
  "details": {
    "format": "gguf",
    "family": "qwen2",
    "parameter_size": "7B"
  }
}
```

## Benefits

✅ **No Static Configuration**: No JSON files to maintain  
✅ **Auto-Discovery**: New models automatically detected  
✅ **Performance**: 24h cache avoids repeated API calls  
✅ **Reliability**: API-first with fallback patterns  
✅ **User Control**: Future override capability

## Limitations

⚠️ **Ollama Server Required**: For Ollama models, server must be running  
⚠️ **OpenAI Limited Metadata**: Must use pattern matching  
⚠️ **Initial Delay**: First check requires API call

## Future Enhancements

1. **Ollama Tag API Integration**: `https://ollama.com/search?c=thinking`
2. **Community Model Registry**: Crowdsourced capability database
3. **LiteLLM Integration**: Use their model registry as fallback
4. **Model Fingerprinting**: Detect capabilities via test prompts

## References

- [Ollama Thinking Documentation](https://docs.ollama.com/capabilities/thinking)
- [Ollama API Show Endpoint](https://docs.ollama.com/api-reference/show-model-details)
- [LiteLLM Ollama Provider](https://docs.litellm.ai/docs/providers/ollama)
- [OpenAI Models API](https://platform.openai.com/docs/api-reference/models)
