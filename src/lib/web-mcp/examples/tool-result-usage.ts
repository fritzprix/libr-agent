/**
 * Examples demonstrating tool-result.ts usage patterns with WebMCP servers.
 *
 * These examples show how to use the ToolResult utility to access both
 * human-readable text and structured data from MCP tool responses.
 */

import { useWebMCP } from '@/context/WebMCPContext';
import { useWebMCPServer } from '@/hooks/use-web-mcp-server';
import {
  toToolResult,
  hasStructuredData,
  hasTextContent,
} from '../tool-result';

// Type definitions for structured responses
interface CreateStoreOutput {
  storeId: string;
  createdAt: string | Date;
}

interface PlanningGoalOutput {
  goal: string;
  action: 'create_goal';
  success: boolean;
}

interface AddTodoOutput {
  action: 'add_todo';
  success: boolean;
  todoName: string;
  totalTodos: number;
  newTodo: {
    id: number;
    name: string;
    status: 'pending' | 'completed';
  };
}

/**
 * Example 1: Using ToolResult with content-store server
 * Shows how to access both user-friendly messages and structured data
 */
export async function contentStoreExample() {
  const { proxy } = useWebMCP();
  if (!proxy) return;

  // Method 1: Direct proxy call with toToolResult
  const createResponse = await proxy.callTool('content-store', 'createStore', {
    metadata: { sessionId: 'session_123' },
  });

  const createResult = toToolResult<CreateStoreOutput>(createResponse);

  // Access user-friendly text for UI display
  if (hasTextContent(createResult)) {
    console.log('User message:', createResult.text);
    // "Store created with ID: store_1234567890"
  }

  // Access structured data for programmatic use
  if (hasStructuredData(createResult)) {
    console.log('Store ID:', createResult.data.storeId);
    console.log('Created at:', createResult.data.createdAt);
    // Use structured data for navigation, state updates, etc.
  }

  // Method 2: Using the server proxy (automatically parsed)
  const { server } = useWebMCPServer('content-store');
  if (
    server &&
    'createStore' in server &&
    typeof server.createStore === 'function'
  ) {
    // This returns structured data directly due to WebMCPContext parsing
    const autoResult = await (
      server.createStore as (args: Record<string, unknown>) => Promise<unknown>
    )({ metadata: { sessionId: 'session_456' } });
    console.log('Auto-parsed result:', autoResult);
    // { storeId: "store_...", createdAt: "..." }
  }
}

/**
 * Example 2: Using ToolResult with enhanced planning-server
 * Demonstrates accessing both text and structured responses from planning tools
 */
export async function planningServerExample() {
  const { server } = useWebMCPServer('planning');
  if (!server) return;

  // Create a goal - planning server now provides structured data
  const { proxy } = useWebMCP();
  if (!proxy) return;

  const goalResponse = await proxy.callTool('planning', 'create_goal', {
    goal: 'Implement user authentication system',
  });

  const goalResult = toToolResult<PlanningGoalOutput>(goalResponse);

  // Display user-friendly message
  if (hasTextContent(goalResult)) {
    console.log('Goal created:', goalResult.text);
    // "Goal created: \"Implement user authentication system\""
  }

  // Use structured data for state management
  if (hasStructuredData(goalResult)) {
    const { goal, action, success } = goalResult.data;
    if (success) {
      // Update UI state, trigger notifications, etc.
      console.log(`Action ${action} completed for goal: ${goal}`);
    }
  }

  // Add a todo with structured response
  const todoResponse = await proxy.callTool('planning', 'add_todo', {
    name: 'Set up authentication database',
  });

  const todoResult = toToolResult<AddTodoOutput>(todoResponse);

  // Both text and data are available
  if (hasTextContent(todoResult) && hasStructuredData(todoResult)) {
    console.log('UI Message:', todoResult.text);
    console.log('New Todo:', todoResult.data.newTodo);
    console.log('Total Todos:', todoResult.data.totalTodos);

    // Perfect for updating both UI and application state
    updateTodoList(todoResult.data.newTodo);
    showNotification(todoResult.text);
  }
}

/**
 * Example 3: Building a universal MCP response handler
 * Shows how to create reusable functions that work with any MCP server
 */
export function createUniversalHandler<T>(
  onText?: (text: string) => void,
  onData?: (data: T) => void,
  onRaw?: (raw: unknown) => void,
) {
  return function handleMCPResponse(mcpResponse: unknown) {
    const result = toToolResult<T>(mcpResponse);

    // Handle text content (user messages, summaries)
    if (hasTextContent(result) && onText) {
      onText(result.text);
    }

    // Handle structured data (API responses, state updates)
    if (hasStructuredData(result) && onData) {
      onData(result.data);
    }

    // Handle raw responses (debugging, logging)
    if (onRaw) {
      onRaw(result.raw);
    }

    return result;
  };
}

/**
 * Example 4: Error handling with ToolResult
 * Shows how to handle both error messages and structured error data
 */
export async function errorHandlingExample() {
  const { proxy } = useWebMCP();
  if (!proxy) return;

  try {
    const response = await proxy.callTool('planning', 'toggle_todo', {
      id: 999, // Non-existent ID
    });

    const result = toToolResult(response);

    // Even errors can have both text and structured data
    if (hasTextContent(result)) {
      console.log('Error message:', result.text);
      // "Todo with ID 999 not found. Available IDs: 1, 2, 3"
    }

    if (hasStructuredData(result)) {
      const errorData = result.data as {
        success: boolean;
        error?: string;
        availableIds?: number[];
      };
      if (!errorData.success) {
        console.log('Error code:', errorData.error);
        console.log('Available IDs:', errorData.availableIds);
        // Use structured error data for smart error handling
      }
    }
  } catch (error) {
    console.error('Tool call failed:', error);
  }
}

/**
 * Example 5: Real-world usage in React components
 * Shows practical integration with React state management
 */
export function useContentStoreWithToolResult() {
  const [userMessage, setUserMessage] = React.useState<string>('');
  const [storeData, setStoreData] = React.useState<CreateStoreOutput | null>(
    null,
  );
  const { proxy } = useWebMCP();

  const createStore = async (metadata: Record<string, unknown>) => {
    if (!proxy) return;

    try {
      const response = await proxy.callTool('content-store', 'createStore', {
        metadata,
      });
      const result = toToolResult<CreateStoreOutput>(response);

      // Update UI message state
      if (hasTextContent(result)) {
        setUserMessage(result.text);
      }

      // Update application state
      if (hasStructuredData(result)) {
        setStoreData(result.data);
      }

      return result;
    } catch (error) {
      setUserMessage('Failed to create store');
      console.error('Create store error:', error);
    }
  };

  return { createStore, userMessage, storeData };
}

// Helper functions for examples
function updateTodoList(todo: { id: number; name: string; status: string }) {
  console.log('Updating todo list with:', todo);
}

function showNotification(message: string) {
  console.log('Notification:', message);
}

// Mock React for TypeScript
declare const React: {
  useState: <T>(initial: T) => [T, (value: T) => void];
};
