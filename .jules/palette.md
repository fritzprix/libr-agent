## 2025-05-18 - [Accessible Form Fields]
**Learning:** React 18's useId generates IDs with colons (e.g., :r1:), which breaks standard CSS selectors (#:r1:) unless escaped.
**Action:** Use attribute selectors ([id=':r1:']) for testing/styling dynamically generated React IDs.
