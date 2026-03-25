---
name: specialist-creator
description: This skill creates a specialized AI agent (sub-agent) configuration tailored to a specific task or workflow. It guides the setup of system prompts, model selection, tool integration (Builtin and MCP), and parameter tuning to yield high-quality specialist agents. Use this skill when a user requests to create, design, or configure a new expert agent or assistant for a particular domain.
---

# Specialist Creator

This skill automates the creation of a specialized sub-agent configuration. It focuses on properly combining system instructions, model parameters, and required tools (such as workspace capabilities or external MCP servers) into a cohesive agent identity.

## Workflow

1.  **Requirement Analysis**: Understand the user's goal for the new agent. Determine the required domain expertise, the tasks the agent will perform, and any specific tools or skills it needs to leverage.
2.  **Tool Identification**: Identify the built-in capabilities (e.g., `workspace`, `planning`) and external MCP servers (e.g., `gemini`, `arxiv`) the agent will need. Use `tool__list` to discover available tool IDs if needed.
3.  **Prompt Engineering**: Draft a robust `systemPrompt`. The prompt must establish the agent's identity, provide step-by-step workflow instructions, outline constraints, and explicitly reference any skills or tools the agent should use.
4.  **Configuration Drafting**: Combine the analysis into a configuration specification.
5.  **Agent Creation**: Use the `agent__create` API tool to create the agent configuration in the system.
6.  **Verification**: Report the generated Agent ID and its configuration details back to the user.

## Core Configuration Parameters

When creating an agent, you must define the following:

-   `name`: A descriptive name for the agent (e.g., "Meeting Minutes Specialist").
-   `description`: A brief summary of what the agent does.
-   `systemPrompt`: The core set of instructions guiding the agent's behavior. This is the most critical part.
-   `modelName`: The underlying LLM to use (e.g., `claude-3-7-sonnet-20250219`). Default to a highly capable model unless specified otherwise.
-   `modelProvider`: The provider for the model (e.g., `anthropic`).
-   `builtinCapabilities`: A list of built-in tool families required (e.g., `['workspace', 'planning']`).
-   `externalMcpServers`: A list of external MCP server IDs required for specific tasks.
-   `temperature`: Sampling temperature (e.g., `0.2` for precise tasks, `0.7` for creative tasks).

## Example: Creating a Data Analysis Agent

If a user asks to create a "Data Analysis Expert":

1.  **Name**: Data Analysis Expert
2.  **Description**: Analyzes CSV/JSON files and generates statistical reports.
3.  **Built-in Capabilities**: `['workspace']` (to read/write files).
4.  **System Prompt**: "You are a Data Analysis Expert. Your task is to read data files provided by the user in the workspace, perform statistical analysis, and output a detailed markdown report summarizing your findings. Always verify file paths before reading."
5.  **Execution**: Call `agent__create` with these parameters.

## Execution Steps for the AI

1.  **Analyze**: Review the user's request to extract the agent's purpose, required skills, and tools.
2.  **Identify Tools**: If external tools are needed (e.g., a search engine or specific API), look up their server IDs using `tool__list`.
3.  **Draft Prompt**: Write a comprehensive `systemPrompt` that instructs the new agent on exactly how to behave and what workflow to follow.
4.  **Create**: Execute the `agent__create` tool with the formulated parameters.
5.  **Report**: Inform the user that the agent has been created, providing its name, ID, and a summary of its capabilities.