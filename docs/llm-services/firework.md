# OpenAI compatibility

You can use the [OpenAI Python client library](https://github.com/openai/openai-python) to interact with Fireworks. This makes migration of existing applications already using OpenAI particularly easy.

## Specify endpoint and API key

### Using the OpenAI client

You can use the OpenAI client by initializing it with your Fireworks configuration:

```python
from openai import OpenAI

# Initialize with Fireworks parameters
client = OpenAI(
    base_url="https://api.fireworks.ai/inference/v1",
    api_key="<YOUR_FIREWORKS_API_KEY>",
)
```

You can also use environment variables with the client:

```python
import os
from openai import OpenAI

# Initialize using environment variables
client = OpenAI(
    base_url=os.environ.get("OPENAI_API_BASE", "https://api.fireworks.ai/inference/v1"),
    api_key=os.environ.get("OPENAI_API_KEY"),  # Set to your Fireworks API key
)
```

### Using environment variables

```shell
export OPENAI_API_BASE="https://api.fireworks.ai/inference/v1"
export OPENAI_API_KEY="<YOUR_FIREWORKS_API_KEY>"
```

### Alternative approach

```python
import openai

# warning: it has a process-wide effect
openai.api_base = "https://api.fireworks.ai/inference/v1"
openai.api_key = "<YOUR_FIREWORKS_API_KEY>"
```

## Usage

Use OpenAI's SDK how you'd normally would. Just ensure that the `model` parameter refers to one of [Fireworks models](https://fireworks.ai/models).

### Completion

Simple completion API that doesn't modify provided prompt in any way:

```python
from openai import OpenAI

client = OpenAI(
    base_url="https://api.fireworks.ai/inference/v1",
    api_key="<YOUR_FIREWORKS_API_KEY>",
)

completion = client.completions.create(
    model="accounts/fireworks/models/llama-v3p1-8b-instruct",
    prompt="The quick brown fox",
)
print(completion.choices[0].text)
```

### Chat Completion

Works best for models fine-tuned for conversation (e.g. llama\*-chat variants):

```python
from openai import OpenAI

client = OpenAI(
    base_url="https://api.fireworks.ai/inference/v1",
    api_key="<YOUR_FIREWORKS_API_KEY>",
)

chat_completion = client.chat.completions.create(
    model="accounts/fireworks/models/llama-v3p1-8b-instruct",
    messages=[
        {
            "role": "system",
            "content": "You are a helpful assistant.",
        },
        {
            "role": "user",
            "content": "Say this is a test",
        },
    ],
)
print(chat_completion.choices[0].message.content)
```

## API compatibility

### Differences

The following options have minor differences:

- `stop`: the returned string includes the stop word for Fireworks while it's omitted for OpenAI (it can be easily truncated on client side)
- `max_tokens`: behaves differently if the model context length is exceeded. If the length of `prompt` or `messages` plus `max_tokens` is higher than the model's context window, `max_tokens` will be adjusted lower accordingly. OpenAI returns invalid request error in this situation. This behavior can be adjusted by `context_length_exceeded_behavior` parameter.

### Token usage for streaming responses

OpenAI API returns usage stats (number of tokens in prompt and completion) for non-streaming responses but doesn't for the streaming ones (see [forum post](https://community.openai.com/t/chat-completion-stream-api-token-usage/352964)).

Fireworks.ai returns usage stats in both cases. For streaming responses, the `usage` field is returned in the very last chunk on the response (i.e. the one having `finish_reason` set). For example:

```bash cURL
curl --request POST \
     --url https://api.fireworks.ai/inference/v1/completions \
     --header "accept: application/json" \
     --header "authorization: Bearer $API_KEY" \
     --header "content-type: application/json" \
     --data '{"model": "accounts/fireworks/models/starcoder-16b-w8a16", "prompt": "def say_hello_world():", "max_tokens": 100, "stream": true}'
```

```
data: {..., "choices":[{"text":"\n  print('Hello,","index":0,"finish_reason":null,"logprobs":null}],"usage":null}

data: {..., "choices":[{"text":" World!')\n\n\n","index":0,"finish_reason":null,"logprobs":null}],"usage":null}

data: {..., "choices":[{"text":"say_hello_","index":0,"finish_reason":null,"logprobs":null}],"usage":null}

data: {..., "choices":[{"text":"world()\n","index":0,"finish_reason":"stop","logprobs":null}],"usage":{"prompt_tokens":7,"total_tokens":24,"completion_tokens":17}}

data: [DONE]
```

<Note>
  Note, that if you're using OpenAI SDK, they `usage` field won't be listed in the SDK's structure definition. But it can be accessed directly. For example:

- In Python SDK, you can access the attribute directly, e.g. `for chunk in openai.ChatCompletion.create(...): print(chunk["usage"])`.
- In TypeScript SDK, you need to cast away the typing, e.g. `for await (const chunk of await openai.chat.completions.create(...)) { console.log((chunk as any).usage); }`.
  </Note>

### Not supported options

The following options are not yet supported:

- `presence_penalty`
- `frequency_penalty`
- `best_of`: you can use `n` instead
- `logit_bias`
- `functions`: you can use our [LangChain integration](https://python.langchain.com/docs/integrations/providers/fireworks) to achieve similar functionality client-side

Please reach out to us on [Discord](https://discord.gg/fireworks-ai) if you have a use case requiring one of these.
