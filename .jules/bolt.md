## 2024-05-24 - Avoid Array.from().filter() on Set Iterators
**Learning:** Calling `Array.from(set).filter()` on hot paths like AI message formatting creates unnecessary intermediate array allocations, increasing garbage collection pressure.
**Action:** Use a `for...of` loop directly on the `Set` to filter elements into a new array, completely bypassing the intermediate array allocation.
