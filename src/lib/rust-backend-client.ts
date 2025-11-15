/**
 * 🔌 Rust Backend Client
 *
 * This file is kept for backward compatibility.
 * All implementations have been moved to the modular structure in ./backend/
 *
 * @deprecated Import from '@/lib/backend' instead for better tree-shaking and organization
 */

// Re-export everything from the modular backend
export * from './backend';

// For backward compatibility, also provide a default export
import * as backendClient from './backend';
export default backendClient;
