---
title: Use useTransition Over Manual Loading States
impact: LOW
impactDescription: reduces re-renders and improves code clarity
tags: rendering, transitions, useTransition, loading, state
---

## Use useTransition Over Manual Loading States

Use `useTransition` to mark non-urgent UI updates and show pending state without wiring a separate loading flag. It helps keep urgent interactions responsive, but it does **not** cancel in-flight async work for you.

**Incorrect (manual loading state):**

```tsx
function SearchResults() {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState([]);
  const [isLoading, setIsLoading] = useState(false);

  const handleSearch = async (value: string) => {
    setIsLoading(true);
    setQuery(value);
    const data = await fetchResults(value);
    setResults(data);
    setIsLoading(false);
  };

  return (
    <>
      <input onChange={(e) => handleSearch(e.target.value)} />
      {isLoading && <Spinner />}
      <ResultsList results={results} />
    </>
  );
}
```

**Correct (useTransition for non-urgent async result updates):**

```tsx
import { useRef, useState, useTransition } from 'react';

function SearchResults() {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState([]);
  const [isPending, startTransition] = useTransition();
  const latestRequestId = useRef(0);

  const handleSearch = (value: string) => {
    setQuery(value); // Update input immediately
    const requestId = ++latestRequestId.current;

    startTransition(async () => {
      const data = await fetchResults(value);

      if (requestId !== latestRequestId.current) {
        return; // Ignore stale responses
      }

      startTransition(() => {
        setResults(data);
      });
    });
  };

  return (
    <>
      <input onChange={(e) => handleSearch(e.target.value)} />
      {isPending && <Spinner />}
      <ResultsList results={results} />
    </>
  );
}
```

**Benefits:**

- **Built-in pending state**: No need to manually toggle `setIsLoading(true/false)`
- **Better responsiveness**: Keeps urgent updates like typing responsive
- **Lower-priority result rendering**: Marks the results update as non-urgent work
- **Async safety still matters**: Add ordering or cancellation logic for requests because transitions do not cancel network work for you

Reference: [useTransition](https://react.dev/reference/react/useTransition)
