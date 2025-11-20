# Backend API Requirements for Assistants & MCP Servers

This document outlines the API requirements for the Remote Backend to support the "Assistants Backend as a Tool" feature. The frontend implements a dual-backend architecture (Local/Remote) where the `agentHubUrl` setting determines the active backend.

## Overview

The backend must provide RESTful APIs for managing `Assistants` and `MCP Servers`. The frontend expects standard CRUD operations.

**Base URL**: The user configures the `agentHubUrl` in the application settings (e.g., `https://api.my-agent-hub.com`). All endpoints below are relative to this base URL.

---

## 1. Assistants API

### Data Model: `Assistant`

```typescript
interface Assistant {
  id?: string; // UUID
  name: string;
  description?: string;
  avatar?: string; // URL or identifier
  systemPrompt: string;
  mcpServerIds?: string[]; // Array of MCPServerEntity IDs
  localServices?: string[]; // Deprecated/Legacy
  allowedBuiltInServiceAliases?: string[]; // e.g., ['browser', 'content_store']
  deletionProtected: boolean;
  createdAt: string; // ISO 8601 Date
  updatedAt: string; // ISO 8601 Date
}
```

### Assistant Endpoints

#### Get All Assistants

- **Method**: `GET`
- **Path**: `/assistants`
- **Response**: `200 OK` with `Assistant[]`
- **Description**: Returns a list of all available assistants.

#### Get Assistant by ID

- **Method**: `GET`
- **Path**: `/assistants/:id`
- **Response**:
  - `200 OK` with `Assistant`
  - `404 Not Found` if the assistant does not exist.
- **Description**: Returns details of a specific assistant.

#### Create or Update Assistant

- **Method**: `POST`
- **Path**: `/assistants`
- **Body**: `Assistant` (JSON)
- **Response**: `200 OK` or `201 Created` with the saved `Assistant`.
- **Description**: Creates a new assistant or updates an existing one. The backend should handle ID generation if not provided.

#### Delete Assistant

- **Method**: `DELETE`
- **Path**: `/assistants/:id`
- **Response**: `200 OK` or `204 No Content`
- **Description**: Deletes the specified assistant.

---

## 2. MCP Servers API

### Data Model: `MCPServerEntity`

```typescript
interface MCPServerEntity {
  id: string; // UUID
  isActive: boolean;
  createdAt: string; // ISO 8601 Date
  updatedAt: string; // ISO 8601 Date
  name: string;

  // Transport Configuration
  transport: {
    type: 'stdio' | 'sse';
    command?: string; // For stdio
    args?: string[]; // For stdio
    env?: Record<string, string>; // For stdio
    url?: string; // For sse
  };

  authentication?: {
    type: 'oauth2';
    clientId: string;
    clientSecret: string;
    authorizationUrl: string;
    tokenUrl: string;
    scopes: string[];
  };

  metadata?: {
    description?: string;
    icon?: string;
    [key: string]: any;
  };
}
```

### MCP Server Endpoints

#### Get All MCP Servers

- **Method**: `GET`
- **Path**: `/mcp-servers`
- **Response**: `200 OK` with `MCPServerEntity[]`
- **Description**: Returns a list of all registered MCP servers.

#### Get MCP Servers Page

- **Method**: `GET`
- **Path**: `/mcp-servers`
- **Query Parameters**:
  - `page`: number (1-based)
  - `pageSize`: number
- **Response**: `200 OK` with `Page<MCPServerEntity>`

  ```typescript
  interface Page<T> {
    items: T[];
    total: number;
    page: number;
    pageSize: number;
    totalPages: number;
  }
  ```

- **Description**: Returns a paginated list of MCP servers.

#### Get MCP Server by ID

- **Method**: `GET`
- **Path**: `/mcp-servers/:id`
- **Response**:
  - `200 OK` with `MCPServerEntity`
  - `404 Not Found`
- **Description**: Returns details of a specific MCP server.

#### Get MCP Server by Name

- **Method**: `GET`
- **Path**: `/mcp-servers/by-name/:name`
- **Response**:
  - `200 OK` with `MCPServerEntity`
  - `404 Not Found`
- **Description**: Returns details of an MCP server by its name (URL encoded).

#### Create or Update MCP Server

- **Method**: `POST`
- **Path**: `/mcp-servers`
- **Body**: `MCPServerEntity` (JSON)
- **Response**: `200 OK` or `201 Created` with the saved `MCPServerEntity`.
- **Description**: Creates a new MCP server configuration or updates an existing one.

#### Delete MCP Server

- **Method**: `DELETE`
- **Path**: `/mcp-servers/:id`
- **Response**: `200 OK` or `204 No Content`
- **Description**: Deletes the specified MCP server configuration.

---

## Implementation Notes

1. **Synchronization**: The frontend currently implements a simple "fetch and overwrite" strategy for syncing remote assistants to the local database.
2. **Error Handling**: The frontend expects standard HTTP error codes (4xx, 5xx).
3. **CORS**: The backend must support CORS to allow requests from the Tauri application (or browser in web mode).
