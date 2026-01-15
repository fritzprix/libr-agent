## 2024-05-24 - [Duplicate Throttle Implementation]
**Pattern:** Found a `useThrottleHook` implementation embedded inside `useDebounce.ts` that was completely unused and duplicated the intent of `useThrottle.ts`.
**Action:** Removed the unused `useThrottleHook` from `useDebounce.ts` to enforce Single Source of Truth (`src/hooks/useThrottle.ts`).
