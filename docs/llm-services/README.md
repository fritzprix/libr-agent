# LLM Services: A Comprehensive Comparison

This document provides a detailed overview and comparison of the various Large Language Model (LLM) service providers integrated with SynapticFlow.

## Feature Matrix

This table offers a quick comparison of key features across the supported providers.

| Feature | OpenAI | Anthropic | Google Gemini | Groq | Cerebras | Fireworks AI | Ollama |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **SDK Package** | `openai` | `@anthropic-ai/sdk` | `@google/genai` | `groq-sdk` | `@cerebras/cerebras_cloud_sdk` | `openai` | (N/A) |
| **Streaming** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Tool Calling** | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | 🚧 |
| **Enforced Tool Calling**| ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | 🚧 |
| **Image Input** | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | 🚧 |
| **Batch API** | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Deployment** | Cloud | Cloud | Cloud | Cloud | Cloud | Cloud | Local |

**Legend:**
- ✅: Supported
- ❌: Not Supported
- 🚧: Planned / In Development

---

## OpenAI

### Overview
The OpenAI provider offers access to models like GPT-4o, known for their strong general reasoning, conversational, and multimodal capabilities. It serves as a baseline for many other API structures.

### Installation
```bash
npm install openai
```

### Configuration & Usage
```typescript
import OpenAI from 'openai';

const client = new OpenAI({
  apiKey: process.env.OPENAI_API_KEY,
});

const completion = await client.chat.completions.create({
  model: 'gpt-4o',
  messages: [{ role: 'user', content: 'What is the purpose of a rubber duck?' }],
});

console.log(completion.choices[0].message.content);
```

### Key Features
- **Streaming:** Fully supported. Set `stream: true` in the request and iterate over the response chunks.
- **Tool Calling:** Robust support for function calling. Use the `tools` parameter to define functions and `tool_choice` to force the model to use a specific function or any function.
- **Image Input:** Supported via the `image_url` content type in messages with models like `gpt-4o`. Images can be passed by URL or as a Base64-encoded string.
- **Batch API:** An asynchronous Batch API is available for processing large datasets at a 50% discount, with results delivered within 24 hours.

---

## Anthropic (Claude)

### Overview
Anthropic provides the Claude family of models, which are recognized for their large context windows, strong performance on complex reasoning tasks, and a focus on safety and reliability.

### Installation
```bash
npm install @anthropic-ai/sdk
```

### Configuration & Usage
```typescript
import Anthropic from '@anthropic-ai/sdk';

const client = new Anthropic({
  apiKey: process.env.ANTHROPIC_API_KEY,
});

const message = await client.messages.create({
  model: 'claude-3-opus-20240229',
  max_tokens: 1024,
  messages: [{ role: 'user', content: 'What is the history of the sonnet?' }],
});

console.log(message.content);
```

### Key Features
- **Streaming:** Fully supported. Set `stream: true` and handle the message stream events.
- **Tool Calling:** Supported. Define tools and use the `tool_choice` parameter to force the model to use a specific tool (`{"type": "tool", "name": "..."}`) or any available tool (`{"type": "any"}`).
- **Image Input:** Supported by Claude 3 models. Images can be passed as a Base64-encoded string in the message content.
- **Batch API:** The Message Batches API allows for asynchronous processing of a large number of requests.

---

## Google Gemini

### Overview
Google's Gemini models are built from the ground up to be multimodal, excelling at understanding and processing information across text, images, audio, and video. The API is accessible via Google AI Studio or Vertex AI for enterprise-grade features.

### Installation
```bash
npm install @google/genai
```

### Configuration & Usage
```typescript
import { GoogleGenerativeAI } from '@google/genai';

const genAI = new GoogleGenerativeAI(process.env.GEMINI_API_KEY);

const model = genAI.getGenerativeModel({ model: 'gemini-1.5-flash' });
const result = await model.generateContent('What are the main components of a transformer model?');
console.log(result.response.text());
```

### Key Features
- **Streaming:** Fully supported via the `generateContentStream` method.
- **Tool Calling:** Supported. Define functions in the `tools` parameter and force a call using `toolConfig` with `FunctionCallingConfigMode.ANY`.
- **Image Input:** Natively supported. Pass image data (as Base64) along with text in the `generateContent` call.
- **Batch API:** A dedicated Batch Mode is available for processing large volumes of prompts asynchronously at a reduced cost.
- **Advanced Features:** Includes unique capabilities like Caching (`ai.caches`) for prompt prefixes and a File API (`ai.files`) for managing uploads.

---

## Groq

### Overview
Groq is known for its extremely high-speed inference, powered by its custom LPU™ (Language Processing Unit) hardware. It provides an OpenAI-compatible API, making it easy to integrate as a drop-in replacement for speed-critical applications.

### Installation
```bash
npm install groq-sdk
```

### Configuration & Usage
```typescript
import Groq from 'groq-sdk';

const groq = new Groq({
    apiKey: process.env.GROQ_API_KEY,
});

const chatCompletion = await groq.chat.completions.create({
    messages: [{ role: 'user', content: 'Why is low latency important for user-facing AI applications?' }],
    model: 'llama3-8b-8192',
});
console.log(chatCompletion.choices[0].message.content);
```

### Key Features
- **Streaming:** Fully supported. Set `stream: true` in the request.
- **Tool Calling:** Supports OpenAI-compatible function calling, including the `tool_choice` parameter.
- **Image Input:** Supports vision-capable models like Llava, allowing for image inputs via URL or Base64 encoding.
- **Performance:** Its primary feature is exceptionally low latency, making it ideal for real-time conversational AI.

---

## Cerebras

### Overview
Cerebras provides access to its large-scale AI systems, powered by the Wafer-Scale Engine. It offers an OpenAI-compatible API, focusing on high-throughput and performance for large models.

### Installation
```bash
npm install @cerebras/cerebras_cloud_sdk
```

### Configuration & Usage
```typescript
import Cerebras from '@cerebras/cerebras_cloud_sdk';

const client = new Cerebras({
  apiKey: process.env.CEREBRAS_API_KEY,
});

const chatCompletion = await client.chat.completions.create({
    messages: [{ role: 'user', content: 'Explain the concept of wafer-scale computing.' }],
    model: 'llama3.1-8b',
});
console.log(chatCompletion?.choices[0]?.message);
```

### Key Features
- **Streaming:** Supported via the `stream: true` parameter.
- **Tool Calling:** Supports OpenAI-style tool calling.
- **Image Input:** The underlying Cerebras platform supports multimodal models, including those that take images as input.
- **Compatibility:** Aims for strong compatibility with the OpenAI API structure.

---

## Fireworks AI

### Overview
Fireworks AI is a platform focused on providing high-speed inference for a wide variety of open-source and custom models. It uses an OpenAI-compatible API, making it easy to switch providers.

### Installation
```bash
npm install openai
```

### Configuration & Usage
```typescript
import OpenAI from 'openai';

// Point the OpenAI client to the Fireworks AI endpoint
const client = new OpenAI({
    baseURL: 'https://api.fireworks.ai/inference/v1',
    apiKey: process.env.FIREWORKS_API_KEY,
});

const chatCompletion = await client.chat.completions.create({
    model: 'accounts/fireworks/models/llama-v3p1-8b-instruct',
    messages: [{ role: 'user', content: 'What is model cascading?' }],
});
console.log(chatCompletion.choices[0].message.content);
```

### Key Features
- **API Compatibility:** Fully compatible with the OpenAI SDK.
- **Streaming:** Supported.
- **Tool Calling:** **Not supported.** The `functions` parameter is explicitly listed as not yet supported.
- **Model Variety:** Offers access to a broad range of open-source models.

---

## Ollama

### Overview
Ollama allows you to run powerful open-source language models, such as Llama 3, locally on your own machine. It's ideal for privacy-focused applications, offline use, and development without API costs.

### Installation
Ollama is a desktop application that must be installed first. See the [Ollama website](https://ollama.ai). No `npm` package is required for basic integration.

### Configuration & Usage
```typescript
// SynapticFlow specific integration
import { AIServiceFactory, AIServiceProvider } from '@/lib/ai-service';

const service = AIServiceFactory.getService(
  AIServiceProvider.Ollama,
  '', // No API key needed for local Ollama
  { defaultModel: 'llama3.1' }
);

const messages = [{ role: 'user', content: 'How do I run a local LLM?' }];
const responseStream = service.streamChat(messages);
// ... process stream
```

### Key Features
- **Local First:** Runs entirely on your local machine, ensuring data privacy and offline capability.
- **Streaming:** Supported out-of-the-box.
- **Tool Calling:** **Not yet supported.** This feature is on the roadmap.
- **Image Input:** **Not yet supported.** This feature is on the roadmap.
- **Cost-Free:** No API fees, as you are using your own hardware.