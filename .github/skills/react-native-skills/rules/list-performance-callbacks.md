---
title: Hoist callbacks to the root of lists
impact: MEDIUM
impactDescription: Fewer re-renders and faster lists
tags: list-performance, callbacks, memoization, legendlist
---

## List performance callbacks

**Impact: HIGH (Fewer re-renders and faster lists)**

When passing callback functions to list items, create a single instance of the
callback at the root of the list. Items should then call it with a unique
identifier.

**Incorrect (creates a new callback on each render):**

```typescript
return (
  <LegendList
    renderItem={({ item }) => {
      // bad: creates a new callback on each render
      const onPress = () => handlePress(item.id)
      return <Item key={item.id} item={item} onPress={onPress} />
    }}
  />
)
```

**Correct (a single function instance shared by all items):**

```typescript
const onPress = useCallback((id: string) => handlePress(id), [handlePress])

return (
  <LegendList
    renderItem={({ item }) => (
      <Item key={item.id} item={item} onPress={() => onPress(item.id)} />
    )}
  />
)
```

Reference: [Legend List](https://legendapp.com/open-source/legend-list)
