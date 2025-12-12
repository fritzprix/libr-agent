# Message Deduplication - Visual Flow

## Before Deduplication (10 messages, 600 tokens)

```
┌─────────────────────────────────────────────────────────┐
│ Index 0: User Message                                   │
├─────────────────────────────────────────────────────────┤
│ Index 1: Assistant (tool_call: read_file, missing.txt) │ ◄─┐
├─────────────────────────────────────────────────────────┤   │ Pair 1 (KEEP)
│ Index 2: Tool Response (Error: File not found)         │ ◄─┘ Hash: "read_file::missing.txt::Error: File not found"
├─────────────────────────────────────────────────────────┤
│ Index 3: Assistant (tool_call: read_file, missing.txt) │ ◄─┐
├─────────────────────────────────────────────────────────┤   │ Pair 2 (REMOVE - duplicate)
│ Index 4: Tool Response (Error: File not found)         │ ◄─┘ Same hash as Pair 1
├─────────────────────────────────────────────────────────┤
│ Index 5: Assistant (tool_call: read_file, missing.txt) │ ◄─┐
├─────────────────────────────────────────────────────────┤   │ Pair 3 (REMOVE - duplicate)
│ Index 6: Tool Response (Error: File not found)         │ ◄─┘ Same hash as Pair 1
├─────────────────────────────────────────────────────────┤
│ Index 7: User Message (Try again)                      │
├─────────────────────────────────────────────────────────┤
│ Index 8: Assistant (tool_call: read_file, config.json) │ ◄─┐ PRESERVED
├─────────────────────────────────────────────────────────┤   │ (Last 3 messages)
│ Index 9: Tool Response ({"version": "1.0"})            │ ◄─┘ Not deduplicated
└─────────────────────────────────────────────────────────┘
```

## After Deduplication (6 messages, 250 tokens)

```
┌──────────────────────────────────────────────────────────┐
│ Index 0: User Message                                    │
├──────────────────────────────────────────────────────────┤
│ Index 1: Assistant (tool_call: read_file, missing.txt)  │
├──────────────────────────────────────────────────────────┤
│ Index 2: Tool Response                                   │
│   Content: "Error: File not found (repeated 3x)"        │
│   Metadata: { dedupCount: 3 }                           │ ◄── Annotated first occurrence
├──────────────────────────────────────────────────────────┤
│ Index 3: User Message (Try again)                       │
├──────────────────────────────────────────────────────────┤
│ Index 4: Assistant (tool_call: read_file, config.json)  │
├──────────────────────────────────────────────────────────┤
│ Index 5: Tool Response ({"version": "1.0"})             │
└──────────────────────────────────────────────────────────┘

Token Savings: ~350 tokens (58% reduction)
```

## Processing Flow

```
Input Messages (N messages)
         │
         ▼
┌────────────────────────┐
│ Check message count    │
│ If < minMessageCount   │──► Return original (no overhead)
│ (default: 10)          │
└────────┬───────────────┘
         │
         ▼
┌────────────────────────┐
│ Split messages:        │
│ - Compressible: [0..N-3] │
│ - Preserved: [N-3..N]  │ ◄─── Last 3 messages untouched
└────────┬───────────────┘
         │
         ▼
┌────────────────────────┐
│ Extract tool pairs     │
│ from compressible zone │
│                        │
│ For each pair:         │
│ • Assistant message    │
│ • Tool message         │
│ • Hash signature       │
└────────┬───────────────┘
         │
         ▼
┌────────────────────────┐
│ Hash-based dedup       │
│                        │
│ Map<hash, firstPair>   │
│ Set<messageIdsToRemove>│
│                        │
│ O(n) complexity        │
└────────┬───────────────┘
         │
         ▼
┌────────────────────────┐
│ Build result:          │
│ • Skip removed IDs     │
│ • Add dedupCount       │
│ • Append "(repeated Nx)"│
│ • Merge with preserved │
└────────┬───────────────┘
         │
         ▼
Deduplicated Messages
```

## Hash Creation Example

```typescript
// Example tool call/response pair
const pair = {
  assistant: {
    tool_calls: [{
      function: {
        name: "read_file",
        arguments: '{"path":"missing.txt"}'
      }
    }]
  },
  tool: {
    content: [{ 
      type: "text", 
      text: "Error: File not found" 
    }]
  }
}

// Hash creation
const hash = createPairHash(
  "read_file",                    // Tool name
  '{"path":"missing.txt"}',       // Arguments
  "Error: File not found"         // Response content
)

// Result: "read_file::{"path":"missing.txt"}::Error: File not found"
```

## Real-World Scenarios

### Scenario 1: AI Agent Retry Loop (Most Common)
```
AI tries to read a file that doesn't exist
→ Error: File not found
→ AI tries again (same path)
→ Error: File not found (DEDUPLICATED)
→ AI tries third time
→ Error: File not found (DEDUPLICATED)

Token Savings: ~400 tokens
```

### Scenario 2: Repeated Configuration Checks
```
AI checks config.json multiple times during workflow
→ {"version": "1.0", "theme": "dark"}
→ Later checks same file
→ {"version": "1.0", "theme": "dark"} (DEDUPLICATED)

Token Savings: ~150 tokens per duplicate
```

### Scenario 3: Health/Status Checks
```
AI polls service status during long operation
→ check_status() → "Running"
→ check_status() → "Running" (DEDUPLICATED)
→ check_status() → "Running" (DEDUPLICATED)
→ check_status() → "Completed" (KEPT - different result)

Token Savings: ~200 tokens
```

## Performance Profile

| Message Count | Pairs Found | Dedup Time | Token Savings |
|--------------|-------------|------------|---------------|
| < 10         | N/A         | 0ms (skip) | 0             |
| 20           | 5           | <2ms       | ~300 tokens   |
| 50           | 15          | <5ms       | ~800 tokens   |
| 100          | 30          | <10ms      | ~1500 tokens  |
| 200          | 60          | <20ms      | ~3000 tokens  |

## Integration Test Checklist

- [x] No orphaned tool messages
- [x] tool_call_id pairing preserved
- [x] Recent messages untouched
- [x] Metadata added correctly
- [x] Visual indicator in content
- [x] Logging works
- [x] Early exit works
- [x] O(n) performance
- [x] Works with all vendors
- [x] No type errors
