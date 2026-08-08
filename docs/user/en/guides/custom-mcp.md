---
title: Custom MCP
---

# Custom MCP

> Add MCP servers that are not in the Extensions catalog (command, URL, or config).

---

## When to use Custom MCP

- Your own MCP server binary / `npx` package  
- Remote SSE / HTTP MCP endpoints  
- Fine-grained env and args

For packaged catalogs, prefer [Extensions](extensions.md).

---

## Typical steps

1. Open **Extensions** (or the custom server entry point in that screen).  
2. Add a custom server: command + args, or URL.  
3. Set environment variables if required.  
4. Save and attach to an assistant.  
5. Start a Chat session and confirm tools load.

Exact form labels depend on version — look for **Add** / **Custom** near the Extensions list.

---

## Tips

- Absolute paths are more reliable than `PATH`-only commands.  
- If Node/Python is missing, run **App Wizard** first.  
- Never paste secrets into Chat; put them in server env fields.

---

## Related

- [Extensions](extensions.md) · [Troubleshooting](troubleshooting.md)
