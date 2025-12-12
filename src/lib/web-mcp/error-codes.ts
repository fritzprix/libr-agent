/**
 * Standardized error codes for Web MCP modules
 *
 * These error codes provide consistent, machine-readable error identification
 * across all Web MCP built-in tools.
 */

export const WebMCPErrorCodes = {
  // ========== Common Errors ==========
  /** Resource not found */
  NOT_FOUND: 'NOT_FOUND',
  /** Invalid input parameters */
  INVALID_INPUT: 'INVALID_INPUT',
  /** Operation not permitted */
  FORBIDDEN: 'FORBIDDEN',
  /** Internal server error */
  INTERNAL_ERROR: 'INTERNAL_ERROR',

  // ========== MCP Manager Errors ==========
  /** MCP server not found by ID or name */
  MCP_MANAGER: {
    SERVER_NOT_FOUND: 'MCP_MANAGER.SERVER_NOT_FOUND',
    /** Server already connected to the scope */
    ALREADY_CONNECTED: 'MCP_MANAGER.ALREADY_CONNECTED',
    /** Server not connected to the scope */
    NOT_CONNECTED: 'MCP_MANAGER.NOT_CONNECTED',
    /** Failed to start server process */
    SERVER_START_FAILED: 'MCP_MANAGER.SERVER_START_FAILED',
    /** Invalid server configuration */
    INVALID_CONFIG: 'MCP_MANAGER.INVALID_CONFIG',
  },

  // ========== Planning Server Errors ==========
  PLANNING: {
    /** Todo item not found by ID */
    TODO_NOT_FOUND: 'PLANNING.TODO_NOT_FOUND',
    /** Invalid todo status value */
    INVALID_STATUS: 'PLANNING.INVALID_STATUS',
    /** Invalid todo priority value */
    INVALID_PRIORITY: 'PLANNING.INVALID_PRIORITY',
    /** Scratchpad item not found */
    SCRATCHPAD_ITEM_NOT_FOUND: 'PLANNING.SCRATCHPAD_ITEM_NOT_FOUND',
    /** Circular dependency in todo graph */
    CIRCULAR_DEPENDENCY: 'PLANNING.CIRCULAR_DEPENDENCY',
    /** Goal not set */
    NO_GOAL: 'PLANNING.NO_GOAL',
    /** Duplicate todo content */
    DUPLICATE_TODO: 'PLANNING.DUPLICATE_TODO',
  },

  // ========== Knowledge Server Errors ==========
  KNOWLEDGE: {
    /** Knowledge item not found by ID */
    ITEM_NOT_FOUND: 'KNOWLEDGE.ITEM_NOT_FOUND',
    /** Invalid search parameters */
    INVALID_SEARCH_PARAMS: 'KNOWLEDGE.INVALID_SEARCH_PARAMS',
    /** Tag not found */
    TAG_NOT_FOUND: 'KNOWLEDGE.TAG_NOT_FOUND',
  },

  // ========== Assistant Manager Errors ==========
  ASSISTANT: {
    /** Assistant not found by ID */
    NOT_FOUND: 'ASSISTANT.NOT_FOUND',
    /** Invalid assistant configuration */
    INVALID_CONFIG: 'ASSISTANT.INVALID_CONFIG',
    /** Assistant name already exists */
    DUPLICATE_NAME: 'ASSISTANT.DUPLICATE_NAME',
  },

  // ========== Bootstrap Server Errors ==========
  BOOTSTRAP: {
    /** Tool not found in system */
    TOOL_NOT_FOUND: 'BOOTSTRAP.TOOL_NOT_FOUND',
    /** Platform not supported */
    UNSUPPORTED_PLATFORM: 'BOOTSTRAP.UNSUPPORTED_PLATFORM',
    /** Installation failed */
    INSTALL_FAILED: 'BOOTSTRAP.INSTALL_FAILED',
  },

  // ========== Playbook Store Errors ==========
  PLAYBOOK: {
    /** Playbook not found by ID */
    NOT_FOUND: 'PLAYBOOK.NOT_FOUND',
    /** Invalid playbook structure */
    INVALID_STRUCTURE: 'PLAYBOOK.INVALID_STRUCTURE',
    /** Missing required data for execution */
    MISSING_REQUIRED_DATA: 'PLAYBOOK.MISSING_REQUIRED_DATA',
  },

  // ========== UI Tools Errors ==========
  UI_TOOLS: {
    /** User cancelled the operation */
    USER_CANCELLED: 'UI_TOOLS.USER_CANCELLED',
    /** Invalid UI resource type */
    INVALID_RESOURCE_TYPE: 'UI_TOOLS.INVALID_RESOURCE_TYPE',
  },
} as const;

/**
 * Type helper to extract all error code values
 */
export type WebMCPErrorCode =
  | (typeof WebMCPErrorCodes)[keyof typeof WebMCPErrorCodes]
  | (typeof WebMCPErrorCodes.MCP_MANAGER)[keyof typeof WebMCPErrorCodes.MCP_MANAGER]
  | (typeof WebMCPErrorCodes.PLANNING)[keyof typeof WebMCPErrorCodes.PLANNING]
  | (typeof WebMCPErrorCodes.KNOWLEDGE)[keyof typeof WebMCPErrorCodes.KNOWLEDGE]
  | (typeof WebMCPErrorCodes.ASSISTANT)[keyof typeof WebMCPErrorCodes.ASSISTANT]
  | (typeof WebMCPErrorCodes.BOOTSTRAP)[keyof typeof WebMCPErrorCodes.BOOTSTRAP]
  | (typeof WebMCPErrorCodes.PLAYBOOK)[keyof typeof WebMCPErrorCodes.PLAYBOOK]
  | (typeof WebMCPErrorCodes.UI_TOOLS)[keyof typeof WebMCPErrorCodes.UI_TOOLS];

/**
 * Standard error data structure
 */
export interface MCPErrorData {
  /** Error code for machine-readable identification */
  code: string;
  /** Optional suggestions for resolving the error */
  suggestions?: string[];
  /** Additional context-specific data */
  [key: string]: unknown;
}
