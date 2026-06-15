## 2024-05-24 - Avoid GC Pressure in Startup Metrics
**Learning:** In non-React data processing code like startup metrics, using `.reduce()` when the callback allocates a new object on every iteration creates severe garbage collection pressure.
**Action:** Refactor these reducers to use primitive variables during a standard `for` loop and allocate a single object at the end to improve startup performance.
## 2024-06-15 - Optimize Collection Processing
**Learning:** When mapping values from a large Map (e.g., OpenRouter's 400+ models), chaining `Array.from(map.values()).map(...)` allocates an unnecessary intermediate array, which increases garbage collection pressure.
**Action:** Use a `for...of` loop directly on the map iterator (`map.values()`) to iterate and map the collection without the intermediate array allocation.
