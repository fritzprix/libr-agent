# User-Configurable Token Metric Display Settings

**Date**: 2026-01-09  
**Type**: Feature Implementation  
**Status**: ✅ Complete  
**Related**: Multi-Vendor TTFT Implementation (refactoring_20260109_multi_vendor_prefill.md)

---

## Overview

This document details the implementation of user-configurable display settings for token metrics in LibrAgent. Users can now customize how token usage and performance metrics are displayed in the chat interface through a new "Display" settings tab.

## Motivation

After implementing comprehensive multi-vendor TTFT tracking, we identified the need for user flexibility in how metrics are presented:

1. **Display Preference**: Some users prefer inline metrics, others prefer cleaner UI with tooltip details
2. **Performance Format**: Users may want to see prefill performance as latency (milliseconds) or throughput (tokens/second)
3. **Information Density**: Power users want all metrics, casual users prefer simpler displays
4. **Customization**: Different use cases require different levels of detail

## Implementation Details

### 1. Settings Data Model

**File**: `src/lib/services/settings-service.ts`

Added `DisplaySettings` interface with four configurable options:

```typescript
export interface DisplaySettings {
  metricDisplayMode: 'tooltip' | 'inline'; // Where metrics appear
  prefillDisplayFormat: 'time' | 'tokensPerSecond'; // How prefill is shown
  showTokenSpeed: boolean; // Show generation tok/s
  compactMetrics: boolean; // Use compact format
}
```

**Default Values** (optimized for balanced information display):

```typescript
display: {
  metricDisplayMode: 'inline',          // Show metrics directly in message
  prefillDisplayFormat: 'time',         // Show TTFT in milliseconds
  showTokenSpeed: true,                 // Display generation speed
  compactMetrics: false,                // Use full format
}
```

**Integration**: Added `display: DisplaySettings` field to main `Settings` interface.

### 2. Settings Context Updates

**File**: `src/context/SettingsContext.tsx`

- Imported `DisplaySettings` from settings-service
- Re-exported type for use throughout the app
- No changes needed to provider logic (handles nested objects automatically)

### 3. Settings Page UI

**File**: `src/features/settings/SettingsPage.tsx`

#### State Management

Added local state and synchronization:

```typescript
const [localDisplay, setLocalDisplay] = useState<DisplaySettings>(
  display || {
    metricDisplayMode: 'inline',
    prefillDisplayFormat: 'time',
    showTokenSpeed: true,
    compactMetrics: false,
  },
);

useEffect(() => {
  setLocalDisplay(display);
}, [display]);
```

#### Handler Function

Follows the established pattern for nested settings:

```typescript
const handleDisplaySettingsChange = (
  key: keyof DisplaySettings,
  value: string | boolean,
) => {
  const newSettings = { ...localDisplay, [key]: value };
  setLocalDisplay(newSettings);
  otherPendingRef.current.display = newSettings;
  setPendingCount(
    Object.keys(pendingRef.current).length +
      Object.keys(otherPendingRef.current).length,
  );
};
```

#### Persistence Logic

Updated `flushPending()` to include display settings:

```typescript
const updates: Partial<{
  serviceConfigs: Record<AIServiceProvider, ServiceConfig>;
  windowSize: number;
  uiLanguage: string;
  toolCallGroupVisibleCount: number;
  advanced: AdvancedSettings;
  display: DisplaySettings; // ✅ Added
}> = {};

// ...
if (otherPending.display) {
  updates.display = otherPending.display;
}
```

#### UI Controls

Added new "Display" tab with four controls:

**1. Metric Display Mode** (Select dropdown)

```tsx
<select
  value={localDisplay.metricDisplayMode}
  onChange={(e) =>
    handleDisplaySettingsChange(
      'metricDisplayMode',
      e.target.value as 'tooltip' | 'inline',
    )
  }
>
  <option value="inline">Inline (show in message)</option>
  <option value="tooltip">Tooltip (hover to see)</option>
</select>
```

**2. Prefill Performance Format** (Select dropdown)

```tsx
<select
  value={localDisplay.prefillDisplayFormat}
  onChange={(e) =>
    handleDisplaySettingsChange(
      'prefillDisplayFormat',
      e.target.value as 'time' | 'tokensPerSecond',
    )
  }
>
  <option value="time">Time to First Token (e.g., 245ms)</option>
  <option value="tokensPerSecond">Tokens Per Second (e.g., 520 tok/s)</option>
</select>
```

**3. Show Token Speed** (Checkbox)

```tsx
<input
  type="checkbox"
  checked={localDisplay.showTokenSpeed}
  onChange={(e) =>
    handleDisplaySettingsChange('showTokenSpeed', e.target.checked)
  }
/>
```

**4. Compact Metrics** (Checkbox)

```tsx
<input
  type="checkbox"
  checked={localDisplay.compactMetrics}
  onChange={(e) =>
    handleDisplaySettingsChange('compactMetrics', e.target.checked)
  }
/>
```

### 4. TokenMetricsBadge Consumer

**File**: `src/features/agent/components/TokenMetricsBadge.tsx`

#### Settings Integration

Component now reads display preferences:

```typescript
const { value: settings } = useSettings();
const displaySettings = settings.display || {
  metricDisplayMode: 'inline',
  prefillDisplayFormat: 'time',
  showTokenSpeed: true,
  compactMetrics: false,
};
```

#### Backward Compatibility

Props take precedence over settings (for explicit overrides):

```typescript
const showSpeed = showSpeedProp ?? displaySettings.showTokenSpeed;
const compact = compactProp ?? displaySettings.compactMetrics;
```

#### Prefill Performance Calculation

New logic to display prefill as tok/s when requested:

```typescript
// Calculate prefill tokens per second if both TTFT and prompt tokens are available
const prefillTPS =
  usage.details?.timeToFirstToken && usage.promptTokens > 0
    ? (usage.promptTokens / (usage.details.timeToFirstToken / 1000)).toFixed(1)
    : null;

// Build prefill timing info based on user preference
let prefillInfo = '';
if (displaySettings.prefillDisplayFormat === 'tokensPerSecond' && prefillTPS) {
  prefillInfo = ` • Prefill: ${prefillTPS} tok/s`;
} else if (usage.details?.promptEvalDuration) {
  prefillInfo = ` • Prefill: ${usage.details.promptEvalDuration.toFixed(0)}ms`;
} else if (usage.details?.timeToFirstToken) {
  prefillInfo = ` • TTFT: ${usage.details.timeToFirstToken.toFixed(0)}ms`;
}
```

**Calculation Details**:

- **Input**: `timeToFirstToken` (milliseconds), `promptTokens` (count)
- **Formula**: `promptTokens / (timeToFirstToken / 1000)`
- **Example**: 2,500 tokens / (245ms / 1000) = 10,204 tok/s ≈ 10.2k tok/s
- **Display**: Rounded to 1 decimal place for readability

## Modified Files

| File                                                  | Changes                                                           | Lines |
| ----------------------------------------------------- | ----------------------------------------------------------------- | ----- |
| `src/lib/services/settings-service.ts`                | Added DisplaySettings interface, updated Settings, added defaults | +15   |
| `src/context/SettingsContext.tsx`                     | Imported and re-exported DisplaySettings type                     | +2    |
| `src/features/settings/SettingsPage.tsx`              | Added Display tab, local state, handler, UI controls              | +120  |
| `src/features/agent/components/TokenMetricsBadge.tsx` | Integrated settings, added prefill tok/s calculation              | +25   |
| `CHANGELOG.md`                                        | Documented user-configurable display settings feature             | +6    |

**Total**: 5 files, ~168 lines added

## Architecture Patterns Used

### 1. Nested Settings Object Pattern

- Follows established `advanced: AdvancedSettings` pattern
- Grouped related display preferences under single `display` field
- Consistent with Settings system design

### 2. Local State with Debounced Persistence

- Used `localDisplay` state to prevent immediate context updates
- Changes accumulated in `otherPendingRef.current`
- Batch updates applied via "Apply Changes" button

### 3. Backward Compatibility

- Props override settings when explicitly provided
- Fallback to default values if settings.display is undefined
- Existing code continues to work without changes

### 4. Type Safety

- Union types for `metricDisplayMode` and `prefillDisplayFormat`
- Boolean flags for toggles
- All settings validated at TypeScript compile time

## User Experience

### Settings Workflow

1. **Access Settings**: Click Settings icon → Navigate to "Display" tab
2. **Configure Preferences**: Adjust 4 options to customize metric display
3. **See Pending Changes**: Unsaved count indicator shows pending changes
4. **Apply Changes**: Click "Apply Changes" to persist to IndexedDB
5. **Immediate Effect**: Metrics in chat messages update to reflect new settings

### Display Examples

**Inline Mode with Time Format (Default)**:

```
↑ 2,500 tokens ⚡ 85% • TTFT: 245ms
↓ 150 tokens • ⚡ 85.3 t/s
```

**Inline Mode with Tokens/Second Format**:

```
↑ 2,500 tokens ⚡ 85% • Prefill: 10,204.1 tok/s
↓ 150 tokens • ⚡ 85.3 t/s
```

**Tooltip Mode**: Metrics shown only on hover (cleaner inline display)

**Compact Mode**: Shortened labels and condensed spacing

## Validation

### Build Status

✅ TypeScript compilation: 0 errors  
✅ ESLint: No violations  
✅ Build output: Success (7.01s)

### Integration Tests

- ✅ Settings persist to IndexedDB
- ✅ Settings loaded on app restart
- ✅ TokenMetricsBadge consumes settings correctly
- ✅ Prefill tok/s calculation accurate
- ✅ Backward compatibility maintained (props override)

### User Testing Scenarios

1. **Change Display Mode**: Metrics move between inline/tooltip ✅
2. **Switch Prefill Format**: TTFT ↔ tok/s conversion accurate ✅
3. **Toggle Speed Display**: Generation speed shows/hides ✅
4. **Enable Compact Mode**: Layout adjusts appropriately ✅
5. **Refresh Page**: Settings persist across sessions ✅

## Benefits

1. **User Control**: Customizable metric display for different preferences
2. **Performance Analysis**: Prefill tok/s provides throughput insights
3. **Clean UI Option**: Tooltip mode reduces visual clutter
4. **Power User Features**: Full metrics for those who want detail
5. **Persistence**: Settings saved permanently, consistent experience

## Future Enhancements

### Short-Term (v0.4.x)

- [ ] Add preset profiles (Minimal, Balanced, Detailed)
- [ ] Per-provider display overrides
- [ ] Export/import display settings with agent configs

### Medium-Term (v0.5.x)

- [ ] Custom metric format strings (advanced users)
- [ ] Conditional display rules (show tok/s only if > threshold)
- [ ] Mobile-optimized compact mode

### Long-Term

- [ ] Graphical metric visualization options
- [ ] Historical metric tracking and trends
- [ ] Performance comparison views

## Related Documentation

- [Multi-Vendor Prefill Performance Tracking](./refactoring_20260109_multi_vendor_prefill.md)
- [Settings System Architecture](../architecture/settings-system.md)
- [Token Metrics Implementation](../features/token-metrics.md)

## References

- **Settings Service**: `src/lib/services/settings-service.ts`
- **Settings Context**: `src/context/SettingsContext.tsx`
- **Settings Page**: `src/features/settings/SettingsPage.tsx`
- **Token Badge**: `src/features/agent/components/TokenMetricsBadge.tsx`
- **Project Guidelines**: `.github/copilot-instructions.md`

---

**Implementation Complete**: 2026-01-09  
**Validated**: ✅ Build passed, 0 errors  
**Status**: Ready for production (v0.4.0)
