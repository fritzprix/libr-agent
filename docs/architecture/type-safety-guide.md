# Type Safety Guide

**Principle: Never use `any` in TypeScript.** The lint configuration is extremely strict — always use precise types and interfaces.

## Prohibited Patterns

1. **Blind Type Assertions** — Never use `as` or `<Type>` casting without runtime validation
2. **Unsafe unknown handling** — When using `unknown`, ALWAYS validate before casting
3. **Blind any conversion** — Never cast `any` directly to a specific type without validation
4. **JSON.parse without validation** — Never cast parsed JSON without runtime checks
5. **Backend response assumptions** — Never assume backend data structure without validation
6. **Generic type casts** — Never use `as T` in generic functions without validation

## Anti-Patterns (❌ Don't Do This)

```typescript
// ❌ Direct casting without validation
const data = response as MyInterface;
const result = <UserData>jsonData;

// ❌ Unknown to type without validation
function process(input: unknown) {
  const user = input as User; // Unsafe!
  return user.name;
}

// ❌ Any to specific type
function handle(data: any) {
  const config: Config = data; // Unsafe!
}

// ❌ JSON.parse without validation
const config = JSON.parse(jsonString) as AppConfig;

// ❌ Backend response without validation
const tools = (await getTools(sessionId)) as MCPTool[];

// ❌ Generic cast without validation
function getValue<T>(key: string, defaultValue: T): T {
  const value = storage.get(key);
  return (value ?? defaultValue) as T; // Unsafe!
}

// ❌ Double casting to bypass errors
const sdk = this.client as unknown as SpecificSDK;
```

## Correct Patterns (✅ Do This)

### Type Guard Validation

```typescript
interface User {
  name: string;
  age: number;
}

function isUser(obj: unknown): obj is User {
  return (
    typeof obj === 'object' &&
    obj !== null &&
    'name' in obj &&
    typeof obj.name === 'string' &&
    'age' in obj &&
    typeof obj.age === 'number'
  );
}

function process(input: unknown) {
  if (isUser(input)) {
    return input.name; // Type-safe!
  }
  throw new Error('Invalid user data');
}
```

### Zod Schema Validation

```typescript
import { z } from 'zod';

const UserSchema = z.object({
  name: z.string(),
  age: z.number(),
});

function processWithZod(input: unknown) {
  const user = UserSchema.parse(input); // Runtime validation + type inference
  return user.name;
}
```

### Backend Response Validation

```typescript
async function getValidatedTools(sessionId: string): Promise<MCPTool[]> {
  const response = await getTools(sessionId);

  if (!Array.isArray(response)) {
    throw new Error('Expected array of tools');
  }

  return response.filter((tool): tool is MCPTool => {
    return isMCPTool(tool); // Use type guard
  });
}
```

### Generic with Validator

```typescript
function getValue<T>(
  key: string,
  defaultValue: T,
  validator: (val: unknown) => val is T,
): T {
  const value = storage.get(key);
  if (value !== undefined && validator(value)) {
    return value;
  }
  return defaultValue;
}
```

### Progressive Type Narrowing

```typescript
function handleData(data: unknown) {
  if (typeof data !== 'object' || data === null) {
    throw new Error('Expected object');
  }
  if (!('type' in data) || typeof data.type !== 'string') {
    throw new Error('Missing type field');
  }
  // Now data is narrowed to { type: string } & object
  return data.type;
}
```

### SDK Cast (Documented Exception)

```typescript
interface OpenAIModelsAPI {
  models: { list: () => Promise<{ data: unknown[] }> };
}

// Define explicit interface and document why cast is needed
const openaiModels = this.openai as OpenAIModelsAPI;
```

## Acceptable `unknown` Usage

These patterns are acceptable:

- **Logger variadic arguments**: `...args: unknown[]` for flexible logging
- **Error catch blocks**: `catch (error: unknown)` per TypeScript best practice
- **Test environment mocking**: `(global as unknown as MockType)` for test setup
- **Protocol definitions**: JSON-RPC payloads where structure varies
- **Abstract base classes**: When subclasses define concrete types

**Key difference:** These use `unknown` as input that gets validated, not as output assumed to be valid.

## Type Safety Checklist

Before merging any PR, verify:

- [ ] No `as any` casts (use `grep "as any" src/`)
- [ ] No ESLint disable comments for type rules
- [ ] All `JSON.parse` operations have schema validation
- [ ] All backend responses validated with type guards
- [ ] All `unknown` types narrowed before use
- [ ] Generic functions include validator parameters
- [ ] Type assertions documented with rationale

## Refactoring Guidelines

When encountering type safety issues:

1. **Identify the root cause** — Why is the type unknown or any?
2. **Add runtime validation** — Use type guards or Zod schemas
3. **Update types at source** — Fix backend type definitions if possible
4. **Document exceptions** — If cast is truly necessary, document why
5. **Add tests** — Ensure validation catches invalid data

See [Type Safety Refactoring Plan](../../docs/refactoring/type-safety-refactoring-plan.md) for detailed migration guide.
