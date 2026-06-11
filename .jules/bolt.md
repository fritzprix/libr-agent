## 2024-06-11 - Avoid intermediate array allocations in map transforms
**Learning:** Using `Array.from(map.values()).map(...)` allocates an unnecessary intermediate array, which increases garbage collection pressure, especially for larger collections like the OpenRouter model list (~400 items).
**Action:** Replace `Array.from(map.values()).map(...)` with a `for...of` loop directly iterating over the map iterator (`for (const item of map.values()) { ... }`) to build the final array in a single pass.
