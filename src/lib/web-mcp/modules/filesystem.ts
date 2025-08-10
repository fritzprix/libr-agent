/**
 * 📁 File System MCP Server Module
 *
 * A file system MCP server that provides file operations using Tauri APIs.
 * This demonstrates how to integrate native system capabilities into web worker MCP servers.
 */

import { invoke } from '@tauri-apps/api/core';
import {
  WebMCPServer,
  MCPTool,
  createObjectSchema,
  createStringSchema,
  createBooleanSchema,
} from '../../mcp-types';

// Filesystem 서버 타입 정의 - 함수 형식에만 집중
export interface FilesystemServer {
  read_file(args: { path: string; encoding?: string }): Promise<{ content: string; size: number }>;
  write_file(args: { path: string; content: string; encoding?: string }): Promise<{ success: boolean; bytesWritten: number }>;
  append_file(args: { path: string; content: string; encoding?: string }): Promise<{ success: boolean; bytesAppended: number }>;
  delete_file(args: { path: string }): Promise<{ success: boolean }>;
  file_exists(args: { path: string }): Promise<{ exists: boolean }>;
  get_file_info(args: { path: string }): Promise<{ 
    size: number; 
    isFile: boolean; 
    isDirectory: boolean; 
    modified: string; 
    created: string; 
    accessed: string; 
  }>;
  create_directory(args: { path: string; recursive?: boolean }): Promise<{ success: boolean }>;
  list_directory(args: { path: string }): Promise<{ entries: Array<{ name: string; isFile: boolean; isDirectory: boolean; size?: number; modified?: string }> }>;
  delete_directory(args: { path: string; recursive?: boolean }): Promise<{ success: boolean }>;
  copy_file(args: { source: string; destination: string; overwrite?: boolean }): Promise<{ success: boolean }>;
  move_file(args: { source: string; destination: string; overwrite?: boolean }): Promise<{ success: boolean }>;
}

// Define the tools available in this server
const tools: MCPTool[] = [
  {
    name: 'read_file',
    description: 'Read the contents of a file',
    inputSchema: createObjectSchema({
      description: 'File reading parameters',
      properties: {
        path: createStringSchema({
          description: 'Absolute path to the file to read',
        }),
        encoding: {
          type: 'string',
          description: 'File encoding (default: utf8)',
          enum: ['utf8', 'base64', 'binary'],
        },
      },
      required: ['path'],
    }),
  },
  {
    name: 'write_file',
    description: 'Write content to a file',
    inputSchema: createObjectSchema({
      description: 'File writing parameters',
      properties: {
        path: createStringSchema({
          description: 'Absolute path to the file to write',
        }),
        content: createStringSchema({
          description: 'Content to write to the file',
        }),
        append: createBooleanSchema({
          description: 'Whether to append to existing file (default: false)',
        }),
        encoding: {
          type: 'string',
          description: 'File encoding (default: utf8)',
          enum: ['utf8', 'base64', 'binary'],
        },
      },
      required: ['path', 'content'],
    }),
  },
  {
    name: 'list_directory',
    description: 'List contents of a directory',
    inputSchema: createObjectSchema({
      description: 'Directory listing parameters',
      properties: {
        path: createStringSchema({
          description: 'Absolute path to the directory to list',
        }),
        recursive: createBooleanSchema({
          description: 'Whether to list recursively (default: false)',
        }),
        include_hidden: createBooleanSchema({
          description: 'Whether to include hidden files (default: false)',
        }),
      },
      required: ['path'],
    }),
  },
  {
    name: 'create_directory',
    description: 'Create a new directory',
    inputSchema: createObjectSchema({
      description: 'Directory creation parameters',
      properties: {
        path: createStringSchema({
          description: 'Absolute path to the directory to create',
        }),
        recursive: createBooleanSchema({
          description: 'Whether to create parent directories (default: true)',
        }),
      },
      required: ['path'],
    }),
  },
  {
    name: 'delete_file',
    description: 'Delete a file or directory',
    inputSchema: createObjectSchema({
      description: 'File deletion parameters',
      properties: {
        path: createStringSchema({
          description: 'Absolute path to the file or directory to delete',
        }),
        recursive: createBooleanSchema({
          description:
            'Whether to delete directories recursively (default: false)',
        }),
      },
      required: ['path'],
    }),
  },
  {
    name: 'file_exists',
    description: 'Check if a file or directory exists',
    inputSchema: createObjectSchema({
      description: 'File existence check parameters',
      properties: {
        path: createStringSchema({ description: 'Absolute path to check' }),
      },
      required: ['path'],
    }),
  },
  {
    name: 'get_file_info',
    description: 'Get detailed information about a file or directory',
    inputSchema: createObjectSchema({
      description: 'File information parameters',
      properties: {
        path: createStringSchema({
          description: 'Absolute path to the file or directory',
        }),
      },
      required: ['path'],
    }),
  },
  {
    name: 'copy_file',
    description: 'Copy a file or directory to a new location',
    inputSchema: createObjectSchema({
      description: 'File copy parameters',
      properties: {
        source: createStringSchema({ description: 'Source path' }),
        destination: createStringSchema({ description: 'Destination path' }),
        overwrite: createBooleanSchema({
          description: 'Whether to overwrite existing files (default: false)',
        }),
      },
      required: ['source', 'destination'],
    }),
  },
  {
    name: 'move_file',
    description: 'Move or rename a file or directory',
    inputSchema: createObjectSchema({
      description: 'File move parameters',
      properties: {
        source: createStringSchema({ description: 'Source path' }),
        destination: createStringSchema({ description: 'Destination path' }),
        overwrite: createBooleanSchema({
          description: 'Whether to overwrite existing files (default: false)',
        }),
      },
      required: ['source', 'destination'],
    }),
  },
];

/**
 * Tool implementation function
 */
async function callTool(name: string, args: unknown): Promise<unknown> {
  // Validate arguments
  if (!args || typeof args !== 'object') {
    throw new Error('Invalid arguments: must be an object');
  }

  const params = args as Record<string, unknown>;

  try {
    switch (name) {
      case 'read_file': {
        const { path, encoding = 'utf8' } = params;
        if (typeof path !== 'string') {
          throw new Error('Invalid argument: path must be a string');
        }

        const content = await invoke('read_file', { path, encoding });
        return {
          success: true,
          path,
          encoding,
          content,
          size: typeof content === 'string' ? content.length : 0,
        };
      }

      case 'write_file': {
        const { path, content, append = false, encoding = 'utf8' } = params;
        if (typeof path !== 'string' || typeof content !== 'string') {
          throw new Error(
            'Invalid arguments: path and content must be strings',
          );
        }

        await invoke('write_file', { path, content, append, encoding });
        return {
          success: true,
          path,
          encoding,
          append,
          bytes_written: content.length,
        };
      }

      case 'list_directory': {
        const { path, recursive = false, include_hidden = false } = params;
        if (typeof path !== 'string') {
          throw new Error('Invalid argument: path must be a string');
        }

        const entries = await invoke('list_directory', {
          path,
          recursive,
          includeHidden: include_hidden,
        });
        return {
          success: true,
          path,
          recursive,
          include_hidden,
          entries,
          count: Array.isArray(entries) ? entries.length : 0,
        };
      }

      case 'create_directory': {
        const { path, recursive = true } = params;
        if (typeof path !== 'string') {
          throw new Error('Invalid argument: path must be a string');
        }

        await invoke('create_directory', { path, recursive });
        return {
          success: true,
          path,
          recursive,
          message: 'Directory created successfully',
        };
      }

      case 'delete_file': {
        const { path, recursive = false } = params;
        if (typeof path !== 'string') {
          throw new Error('Invalid argument: path must be a string');
        }

        await invoke('delete_file', { path, recursive });
        return {
          success: true,
          path,
          recursive,
          message: 'File/directory deleted successfully',
        };
      }

      case 'file_exists': {
        const { path } = params;
        if (typeof path !== 'string') {
          throw new Error('Invalid argument: path must be a string');
        }

        const exists = await invoke('file_exists', { path });
        return {
          success: true,
          path,
          exists,
        };
      }

      case 'get_file_info': {
        const { path } = params;
        if (typeof path !== 'string') {
          throw new Error('Invalid argument: path must be a string');
        }

        const info = await invoke('get_file_info', { path });
        return {
          success: true,
          path,
          info,
        };
      }

      case 'copy_file': {
        const { source, destination, overwrite = false } = params;
        if (typeof source !== 'string' || typeof destination !== 'string') {
          throw new Error(
            'Invalid arguments: source and destination must be strings',
          );
        }

        await invoke('copy_file', { source, destination, overwrite });
        return {
          success: true,
          source,
          destination,
          overwrite,
          message: 'File/directory copied successfully',
        };
      }

      case 'move_file': {
        const { source, destination, overwrite = false } = params;
        if (typeof source !== 'string' || typeof destination !== 'string') {
          throw new Error(
            'Invalid arguments: source and destination must be strings',
          );
        }

        await invoke('move_file', { source, destination, overwrite });
        return {
          success: true,
          source,
          destination,
          overwrite,
          message: 'File/directory moved successfully',
        };
      }

      default:
        throw new Error(`Unknown tool: ${name}`);
    }
  } catch (error) {
    // Handle Tauri invoke errors
    const errorMessage = error instanceof Error ? error.message : String(error);
    return {
      success: false,
      error: errorMessage,
      tool: name,
      args: params,
    };
  }
}

// Create and export the MCP server
const filesystemServer: WebMCPServer = {
  name: 'filesystem',
  description:
    'File system operations using Tauri APIs for safe cross-platform file access',
  version: '1.0.0',
  tools,
  callTool,
};

export default filesystemServer;
