## 2024-05-24 - Avoid GC Pressure in Startup Metrics
**Learning:** In non-React data processing code like startup metrics, using `.reduce()` when the callback allocates a new object on every iteration creates severe garbage collection pressure.
**Action:** Refactor these reducers to use primitive variables during a standard `for` loop and allocate a single object at the end to improve startup performance.
## 2025-06-13 - [Optimize OpenRouter listModels Iteration]
**Learning:** [Replacing `Array.from(map.values()).map(...)` with a `for...of` loop directly on the map iterator avoids intermediate array allocations and reduces garbage collection pressure.]
**Action:** [When converting iterables or map values into an array, iterate directly with `for...of` instead of allocating intermediate arrays via `Array.from()` followed by `.map()`.]
