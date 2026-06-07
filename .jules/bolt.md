## 2025-06-07 - Refactoring .map/.filter in AppSidebar to standard loops
**Learning:** React component useMemos with multiple array method chains (`.map`, `.filter`, `.forEach`) create intermediate array allocations on every recalculation.
**Action:** Replace `Array.prototype.map` and `Array.prototype.filter` chains with standard `for` loops in components with frequent re-renders or large object collections to minimize GC pressure and improve render performance.
