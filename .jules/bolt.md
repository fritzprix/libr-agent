## 2024-05-24 - Avoid GC Pressure in Startup Metrics
**Learning:** In non-React data processing code like startup metrics, using `.reduce()` when the callback allocates a new object on every iteration creates severe garbage collection pressure.
**Action:** Refactor these reducers to use primitive variables during a standard `for` loop and allocate a single object at the end to improve startup performance.

## 2024-06-14 - Optimize Map iteration in openrouter.ts
**Learning:** Using `Array.from(map.values()).map(...)` creates unnecessary intermediate array allocations, increasing garbage collection pressure.
**Action:** Iterate directly over `map.values()` using a `for...of` loop and push to a result array to avoid intermediate allocations on hot paths or large collections.
