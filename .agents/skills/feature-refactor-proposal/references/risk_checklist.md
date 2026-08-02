# Architecture & Refactoring Risk Checklist

Use this checklist during Step 3 (Impact & Risk Assessment) to systematically evaluate technical risks before proposing refactoring.

## 1. Type Safety & Contracts

- [ ] Are type definitions shared across frontend/backend boundaries?
- [ ] Will changing props/types cause cascading type errors across other components?
- [ ] Are there `any` types or unsafe assertions (`as ...`) introduced?

## 2. State & Context Management

- [ ] Does the new feature create duplicate or fragmented state?
- [ ] Is state placed at the right level (local state vs React Context vs global state)?
- [ ] Will state changes cause excessive re-renders in parent/sibling components?

## 3. Component Architecture & Separation of Concerns

- [ ] Does a single file exceed 300-500 lines or handle multiple unrelated responsibilities?
- [ ] Can sub-components or custom hooks be extracted to keep component logic clean?
- [ ] Is business logic mixed directly into UI rendering code?

## 4. Backward Compatibility & Breaking Changes

- [ ] Does the refactoring break existing component APIs or prop contracts?
- [ ] Are there other features/pages relying on the components being refactored?
- [ ] Is database/API migration required for persistent data?

## 5. UI/UX & Layout Resilience

- [ ] Will layout changes break scroll pinning, virtualized lists, or autosizing inputs?
- [ ] Are loading, empty, and error states handled gracefully?
- [ ] Are keyboard navigation and accessibility maintained?

## 6. Verification & Test Coverage

- [ ] Is there an existing validation pipeline (e.g. `pnpm refactor:validate`, `pnpm test`)?
- [ ] How will the refactored code be verified without manual edge-case testing?
- [ ] Are unit/integration tests updated alongside the code changes?
