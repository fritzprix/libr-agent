# Models List API

## Groq

- Client Example

```ts
import Groq from 'groq-sdk';

const groq = new Groq({ apiKey: process.env.GROQ_API_KEY });

const getModels = async () => {
  return await groq.models.list();
};

getModels().then((models) => {
  // console.log(models);
});
```

## Gemini

- Client Example

```ts
import OpenAI from 'openai';

const openai = new OpenAI({
  apiKey: 'GEMINI_API_KEY',
  baseURL: 'https://generativelanguage.googleapis.com/v1beta/openai/',
});

async function main() {
  const list = await openai.models.list();

  for await (const model of list) {
    console.log(model);
  }
}
main();
```

## OpenAI

- Client Example

```js
import OpenAI from 'openai';

const openai = new OpenAI();

async function main() {
  const list = await openai.models.list();

  for await (const model of list) {
    console.log(model);
  }
}
main();
```

- Response Example

```json
{
  "object": "list",
  "data": [
    {
      "id": "model-id-0",
      "object": "model",
      "created": 1686935002,
      "owned_by": "organization-owner"
    },
    {
      "id": "model-id-1",
      "object": "model",
      "created": 1686935002,
      "owned_by": "organization-owner"
    },
    {
      "id": "model-id-2",
      "object": "model",
      "created": 1686935002,
      "owned_by": "openai"
    }
  ],
  "object": "list"
}
```

## Anthropic

- Client Example

```js
import Anthropic from '@anthropic-ai/sdk';

const anthropic = new Anthropic();

await anthropic.models.list({
  limit: 20,
});
```

- Response Example

```json
{
  "data": [
    {
      "created_at": "2025-02-19T00:00:00Z",
      "display_name": "Claude Sonnet 4",
      "id": "claude-sonnet-4-20250514",
      "type": "model"
    }
  ],
  "first_id": "<string>",
  "has_more": true,
  "last_id": "<string>"
}
```
