💡 What
Replaced `.map()` and `.filter().forEach()` array method chains with standard `for` loops inside the `recentSessions` useMemo hook in `src/components/layout/AppSidebar.tsx`.

🎯 Why
React component useMemos containing multiple array method chains create intermediate array allocations on every recalculation. By utilizing standard `for` loops, we eliminate the unnecessary allocations of temporary arrays, reducing garbage collection pressure during UI updates in the Sidebar.

📊 Impact
Slight reduction in memory allocation and GC overhead during AppSidebar re-renders.

🔬 Measurement
Verified via `pnpm test run` and linting to ensure functionality is preserved.
