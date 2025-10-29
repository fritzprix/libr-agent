# Ollama Integration

This document describes how to integrate and use Ollama with LibrAgent.

## Overview

LibrAgent now supports Ollama as an AI service provider, allowing you to run local language models on your machine. Ollama provides a simple way to run large language models locally, offering privacy and control over your AI interactions.

## Prerequisites

1. **Install Ollama**: Download and install Ollama from [https://ollama.ai](https://ollama.ai)
2. **Pull a Model**: Download at least one model using Ollama CLI
3. **Start Ollama Server**: Ensure the Ollama server is running

### Quick Setup

```bash
# Install a model (example with Llama 3.1)
ollama pull llama3.1

# Start the Ollama server (if not running)
ollama serve

# Verify installation
ollama list
```

## Configuration

### Default Settings

- **Host**: `http://127.0.0.1:11434` (Ollama's default)
- **API Key**: Not required for local Ollama instances
- **Default Model**: `llama3.1`

### Custom Configuration

You can configure Ollama settings in the LibrAgent settings:

1. Open Settings → AI Services
2. Select Ollama as your provider
3. Configure the following options:
   - **Host**: Custom Ollama server URL (if running remotely)
   - **Model**: Choose from available models
   - **Temperature**: Control response creativity (0.0 - 1.0)
   - **Max Tokens**: Maximum response length

## Supported Models

The following models are preconfigured in LibrAgent:

| Model            | Context Window | Description                        |
| ---------------- | -------------- | ---------------------------------- |
| `llama3.1`       | 128,000        | Meta's latest large language model |
| `llama3.2`       | 128,000        | Meta's multimodal model            |
| `llama3.3`       | 128,000        | Meta's latest optimized model      |
| `codellama`      | 100,000        | Specialized for code generation    |
| `mistral`        | 32,000         | High-performance open model        |
| `qwen2.5`        | 128,000        | Alibaba's multilingual model       |
| `gemma2`         | 8,192          | Google's open model                |
| `deepseek-coder` | 16,384         | Specialized coding model           |

### Adding Custom Models

To use models not listed above:

1. Pull the model with Ollama: `ollama pull model-name`
2. In LibrAgent settings, enter the exact model name as it appears in `ollama list`

## Features

### ✅ Supported

- **Streaming Chat**: Real-time response streaming
- **Model Selection**: Choose from any locally available model
- **Custom Host**: Connect to remote Ollama instances
- **Temperature Control**: Adjust response creativity
- **Token Limits**: Control response length
- **System Prompts**: Set custom system instructions

### 🚧 Coming Soon

- **Tool Calling**: MCP tool integration (planned for future release)
- **Image Input**: Support for multimodal models
- **Custom Parameters**: Advanced model parameters

## Usage Examples

### Basic Chat

```typescript
import { AIServiceFactory, AIServiceProvider } from '@/lib/ai-service';

const service = AIServiceFactory.getService(
  AIServiceProvider.Ollama,
  '', // No API key needed for local Ollama
  {
    defaultModel: 'llama3.1',
    temperature: 0.7,
    maxTokens: 4096,
  },
);

const messages = [
  {
    id: '1',
    sessionId: 'session-1',
    role: 'user',
    content: 'Hello, how are you?',
  },
];

for await (const chunk of service.streamChat(messages)) {
  const data = JSON.parse(chunk);
  if (data.content) {
    console.log(data.content);
  }
}
```

### Custom System Prompt

```typescript
const messages = [
  {
    id: '1',
    sessionId: 'session-1',
    role: 'user',
    content: 'Write a Python function to calculate fibonacci numbers',
  },
];

for await (const chunk of service.streamChat(messages, {
  systemPrompt:
    'You are an expert Python developer. Provide clean, well-documented code.',
  modelName: 'codellama',
})) {
  const data = JSON.parse(chunk);
  if (data.content) {
    console.log(data.content);
  }
}
```

## Troubleshooting

### Common Issues

1. **Connection Failed**
   - Ensure Ollama is running: `ollama serve`
   - Check if the host URL is correct
   - Verify firewall settings allow connections to port 11434

2. **Model Not Found**
   - List available models: `ollama list`
   - Pull the model if not available: `ollama pull model-name`
   - Ensure the model name matches exactly

3. **Slow Responses**
   - Consider using smaller models (e.g., `llama3.1:8b` instead of `llama3.1:70b`)
   - Increase system resources allocated to Ollama
   - Check CPU/GPU utilization

4. **Memory Issues**
   - Monitor system memory usage
   - Use quantized models for lower memory requirements
   - Close other applications to free up resources

### Performance Tips

1. **Model Selection**
   - Use 8B parameter models for faster responses
   - Use 70B+ parameter models for higher quality responses
   - Choose specialized models for specific tasks (e.g., `codellama` for coding)

2. **Hardware Optimization**
   - Enable GPU acceleration if available
   - Allocate sufficient RAM for model loading
   - Use SSD storage for faster model loading

3. **Configuration Tuning**
   - Lower temperature for more consistent responses
   - Adjust max tokens based on your needs
   - Use keep_alive to keep models loaded between requests

## Remote Ollama Setup

To connect to a remote Ollama instance:

1. Start Ollama with external access:

   ```bash
   OLLAMA_HOST=0.0.0.0:11434 ollama serve
   ```

2. Configure LibrAgent with the remote host:

   ```typescript
   const service = AIServiceFactory.getService(AIServiceProvider.Ollama, '', {
     host: 'http://remote-server:11434',
     defaultModel: 'llama3.1',
   });
   ```

## Security Considerations

- **Local Only**: Keep Ollama local for maximum privacy
- **Network Access**: Be cautious when exposing Ollama to network access
- **Model Verification**: Only use trusted model sources
- **Data Privacy**: Local processing ensures your data stays on your machine

## API Reference

### OllamaService Class

```typescript
class OllamaService extends BaseAIService {
  constructor(apiKey: string, config?: AIServiceConfig & { host?: string });

  async *streamChat(
    messages: Message[],
    options?: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
    },
  ): AsyncGenerator<string, void, void>;
}
```

### Configuration Options

```typescript
interface AIServiceConfig {
  timeout?: number; // Request timeout in ms
  maxRetries?: number; // Maximum retry attempts
  retryDelay?: number; // Delay between retries in ms
  defaultModel?: string; // Default model name
  maxTokens?: number; // Maximum response tokens
  temperature?: number; // Response creativity (0.0-1.0)
  host?: string; // Ollama server host URL
}
```

## Contributing

To contribute to the Ollama integration:

1. Follow the existing code patterns in `src/lib/ai-service/ollama.ts`
2. Add comprehensive error handling
3. Include appropriate logging
4. Update this documentation for any new features
5. Test with multiple models and configurations

## Resources

- [Ollama Documentation](https://github.com/jmorganca/ollama)
- [Ollama Model Library](https://ollama.ai/library)
- [LibrAgent AI Service Architecture](./AI_SERVICE_ARCHITECTURE.md)
