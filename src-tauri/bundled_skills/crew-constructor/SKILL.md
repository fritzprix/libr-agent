---
name: crew-constructor
description: "Automatically scan available tools (Builtin/MCP) and batch-create optimized Specialist Agents. Automates specialist deployment across research, data analysis, creative, and technical domains."
---

# Bulk Specialist Creator

This skill provides a robust framework to discover current system capabilities and batch-create specialized AI agents tailored to your specific requirements.

## Core Principles: Live Discovery
**Tool inventories and server IDs change frequently.** Never rely on hardcoded tool names or IDs. Every execution must start by discovering the ground truth of the current environment.

## Workflow

1.  **Environment Discovery (Mandatory)**: Call `tool__list(forceVerify=true)` to capture the latest list of available Builtin tools and MCP servers.
2.  **Capability Analysis**: Analyze the `description` fields of discovered tools to categorize them (e.g., "Search," "Financial Data," "Creative AI"). Refer to `references/tool-role-mapping.md`.
3.  **Template Selection**: Select desired agent roles from `references/specialist-templates.md`.
4.  **Dynamic Matching**: Match the analyzed categories to the required tools of your selected specialist templates.
5.  **Batch Creation**: Iteratively call `agent__create` using the *dynamically identified* tool IDs and customized system prompts.
6.  **Verification**: Call `agent__list` to confirm all specialists have the correct, current tool assignments.

## Tool-to-Role Mapping Strategy

Instead of hardcoding tool IDs, map roles based on functional categories:

- **Research Specialist**: Prioritize tools with descriptions involving "search," "academic," or "knowledge retrieval."
- **Financial/Data Analyst**: Prioritize tools with descriptions involving "finance," "economic data," "markets," or "statistical analysis."
- **Creative Specialist**: Prioritize tools with descriptions involving "image," "video," "multimodal," or "generative."
- **Technical/Dev Specialist**: Prioritize tools with descriptions involving "documentation," "code," "API," or "developer."
- **Core (Required for all)**: Always include Builtin tools like `workspace` and `planning`.

## Execution Example
When a user asks: "Create a group of research and data analysis experts," follow these steps:
1.  **Scan**: `tool__list(forceVerify=true)` and inspect the `description` of each tool to identify relevant ones.
2.  **Map**: Identify which dynamic server IDs match the "Research" and "Financial" categories.
3.  **Configure**: Draft prompts based on templates, injecting the dynamic server IDs identified in step 1.
4.  **Create**: Execute `agent__create` for each.
5.  **Report**: Confirm creation, mapping, and provide the resulting agent IDs.
