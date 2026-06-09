## 2025-06-07 - Refactoring .map/.filter in AppSidebar to standard loops
**Learning:** React component useMemos with multiple array method chains (`.map`, `.filter`, `.forEach`) create intermediate array allocations on every recalculation.
**Action:** Replace `Array.prototype.map` and `Array.prototype.filter` chains with standard `for` loops in components with frequent re-renders or large object collections to minimize GC pressure and improve render performance.

## 2025-06-09 - Replaced Array.from().map() with for...of on Maps
**Learning:** Using `Array.from(map.values()).map(...)` to map a large Map allocates an intermediate array and iterates twice, which creates unnecessary memory pressure on startup (especially for providers with hundreds of models like OpenRouter).
**Action:** Iterate directly over `map.values()` using a `for...of` loop and `result.push(...)` to avoid intermediate array allocation and callback overhead.
