# Test Framework for Safe Refactoring

## Overview

This document defines the testing strategy and framework to ensure safe refactoring of large files without breaking functionality. The framework includes unit tests, integration tests, regression tests, and automated validation.

## Testing Principles

### 1. Test First, Refactor Second
- Write comprehensive tests before any refactoring
- Ensure existing functionality is fully covered
- Create safety nets for complex operations

### 2. Incremental Validation
- Test each refactoring phase independently
- Maintain working state between phases
- Quick rollback capability at any point

### 3. Automated Quality Gates
- Automated test execution on every change
- Build verification tests (BVT)
- Performance regression detection

## Test Categories

### Unit Tests (Isolated Module Testing)

#### BrowserToolProvider.tsx Tests
```typescript
// Test individual tool modules after extraction
describe('CreateSessionTool', () => {
  it('should generate correct inputSchema', () => {
    expect(createSessionTool.inputSchema).toEqual({
      type: 'object',
      properties: {
        url: { type: 'string', description: expect.any(String) },
        title: { type: 'string', description: expect.any(String) },
      },
      required: ['url'],
    });
  });

  it('should execute successfully with valid args', async () => {
    const mockCreateBrowserSession = vi.fn().mockResolvedValue('session-123');
    const result = await createSessionTool.execute({ 
      url: 'https://example.com',
      title: 'Test Session'
    });
    expect(result).toBe('Browser session created successfully: session-123');
  });

  it('should handle missing optional parameters', async () => {
    const result = await createSessionTool.execute({ 
      url: 'https://example.com'
    });
    expect(result).toContain('Browser session created successfully');
  });
});
```

#### Chat.tsx Component Tests
```typescript
// Test extracted hooks
describe('useChatState', () => {
  it('should manage showToolsDetail state correctly', () => {
    const { result } = renderHook(() => useChatState());
    
    expect(result.current.showToolsDetail).toBe(false);
    
    act(() => {
      result.current.setShowToolsDetail(true);
    });
    
    expect(result.current.showToolsDetail).toBe(true);
  });

  it('should handle session state changes', () => {
    const mockSession = createMockSession();
    const { result } = renderHook(() => useChatState(mockSession));
    
    expect(result.current.currentSession).toEqual(mockSession);
  });
});

// Test extracted components
describe('SessionFilesPopover', () => {
  it('should render with correct props', () => {
    render(
      <SessionFilesPopover 
        session={mockSession}
        onFileSelect={mockOnFileSelect}
      />
    );
    
    expect(screen.getByTestId('session-files-popover')).toBeInTheDocument();
  });
});
```

#### db.ts Module Tests
```typescript
// Test separated CRUD operations
describe('CRUDService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('create', () => {
    it('should create new item with generated ID', async () => {
      const mockItem = { name: 'Test Item' };
      const result = await crudService.create(mockItem);
      
      expect(result).toMatch(/^[a-z0-9]+$/); // CUID format
      expect(mockDb.table).toHaveBeenCalledWith(expect.any(String));
    });

    it('should validate required fields', async () => {
      const invalidItem = {};
      await expect(crudService.create(invalidItem))
        .rejects
        .toThrow('Validation failed');
    });
  });

  describe('update', () => {
    it('should update existing item', async () => {
      const updates = { name: 'Updated Name' };
      await crudService.update('item-123', updates);
      
      expect(mockDb.table.modify).toHaveBeenCalledWith({
        ...updates,
        updatedAt: expect.any(Date)
      });
    });
  });
});
```

#### Rust Module Tests
```rust
// src-tauri/src/mcp/utils/schema_builder.rs tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_prop_basic() {
        let schema = string_prop(Some(1), Some(100), Some("Test description"));
        
        assert_eq!(schema.description, Some("Test description".to_string()));
        if let JSONSchemaType::String { min_length, max_length } = schema.schema_type {
            assert_eq!(min_length, Some(1));
            assert_eq!(max_length, Some(100));
        } else {
            panic!("Expected string schema type");
        }
    }

    #[test]
    fn test_object_schema_with_required() {
        let mut props = HashMap::new();
        props.insert("name".to_string(), string_prop(Some(1), None, None));
        
        let schema = object_schema(props, vec!["name".to_string()]);
        
        if let JSONSchemaType::Object { required, .. } = schema.schema_type {
            assert_eq!(required, Some(vec!["name".to_string()]));
        } else {
            panic!("Expected object schema type");
        }
    }
}

// src-tauri/src/mcp/builtin/filesystem.rs tests
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio_test]
    async fn test_create_read_file_tool() {
        let tool = create_read_file_tool();
        
        assert_eq!(tool.name, "read_file");
        assert!(!tool.description.is_empty());
        
        // Test schema structure
        if let JSONSchemaType::Object { properties, required, .. } = &tool.input_schema.schema_type {
            assert!(properties.is_some());
            assert_eq!(required, &Some(vec!["path".to_string()]));
        }
    }

    #[tokio_test]
    async fn test_filesystem_server_tools() {
        let server = FilesystemServer;
        let tools = server.get_tools().await.unwrap();
        
        assert!(!tools.is_empty());
        assert!(tools.iter().any(|t| t.name == "read_file"));
        assert!(tools.iter().any(|t| t.name == "write_file"));
    }
}
```

### Integration Tests (Cross-Module Testing)

#### Tool Provider Integration
```typescript
describe('BrowserToolProvider Integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should register all tools successfully', async () => {
    const { result } = renderHook(() => useBuiltInTool(), {
      wrapper: ({ children }) => (
        <BuiltInToolProvider>
          <BrowserToolProvider>
            {children}
          </BrowserToolProvider>
        </BuiltInToolProvider>
      ),
    });

    await waitFor(() => {
      expect(result.current.tools.length).toBeGreaterThan(15);
    });

    // Verify all expected tools are registered
    const toolNames = result.current.tools.map(t => t.name);
    expect(toolNames).toContain('createSession');
    expect(toolNames).toContain('closeSession');
    expect(toolNames).toContain('listSessions');
  });

  it('should execute tools through provider correctly', async () => {
    const { result } = renderHook(() => useBuiltInTool(), {
      wrapper: TestProviderWrapper,
    });

    const toolCall = {
      id: 'test-123',
      name: 'createSession',
      args: { url: 'https://example.com' },
    };

    const response = await result.current.executeTool(toolCall);
    expect(response).toContain('Browser session created successfully');
  });
});
```

#### Chat Component Integration  
```typescript
describe('Chat Integration Tests', () => {
  it('should compose all sub-components correctly', () => {
    const mockSession = createMockSession();
    
    render(
      <SessionContext.Provider value={{ current: mockSession }}>
        <Chat>
          <div>Test Content</div>
        </Chat>
      </SessionContext.Provider>
    );

    expect(screen.getByText('Test Content')).toBeInTheDocument();
    expect(screen.getByTestId('chat-container')).toBeInTheDocument();
  });

  it('should handle context provider interactions', async () => {
    const mockAssistant = createMockAssistant();
    const mockSession = createMockSession({ assistants: [mockAssistant] });

    render(
      <TestContextWrapper session={mockSession}>
        <Chat />
      </TestContextWrapper>
    );

    // Test interactions between context providers
    await waitFor(() => {
      expect(screen.getByText(mockAssistant.name)).toBeInTheDocument();
    });
  });
});
```

#### Database Integration
```typescript
describe('Database Service Integration', () => {
  beforeEach(async () => {
    await dbService.clear();
  });

  it('should handle cross-table operations', async () => {
    // Create assistant
    const assistantId = await dbService.assistants.create({
      name: 'Test Assistant',
      systemPrompt: 'Test prompt',
    });

    // Create session with assistant
    const sessionId = await dbService.sessions.create({
      title: 'Test Session',
      assistants: [{ id: assistantId, name: 'Test Assistant' }],
    });

    // Verify relationships
    const session = await dbService.sessions.get(sessionId);
    expect(session?.assistants[0].id).toBe(assistantId);
  });

  it('should maintain data consistency during concurrent operations', async () => {
    const operations = Array.from({ length: 10 }, (_, i) => 
      dbService.assistants.create({ name: `Assistant ${i}` })
    );

    const results = await Promise.all(operations);
    expect(results).toHaveLength(10);
    expect(new Set(results).size).toBe(10); // All unique IDs
  });
});
```

### Regression Tests (Feature Preservation)

#### Critical Path Testing
```typescript
describe('Critical Path Regression Tests', () => {
  it('should complete full browser tool workflow', async () => {
    const { result } = renderHook(() => useBuiltInTool(), {
      wrapper: FullTestWrapper,
    });

    // 1. Create session
    const createResult = await result.current.executeTool({
      id: 'test-1',
      name: 'createSession',
      args: { url: 'https://example.com' },
    });
    expect(createResult).toContain('session created');

    // 2. List sessions
    const listResult = await result.current.executeTool({
      id: 'test-2',
      name: 'listSessions',
      args: {},
    });
    expect(listResult).toContain('example.com');

    // 3. Close session
    const sessionId = extractSessionId(createResult);
    const closeResult = await result.current.executeTool({
      id: 'test-3',
      name: 'closeSession',
      args: { sessionId },
    });
    expect(closeResult).toContain('closed');
  });

  it('should maintain chat functionality', async () => {
    const mockMessage = createMockMessage();
    
    render(
      <FullTestWrapper>
        <Chat />
      </FullTestWrapper>
    );

    // Send message
    const input = screen.getByPlaceholderText('Type your message...');
    fireEvent.change(input, { target: { value: 'Test message' } });
    fireEvent.click(screen.getByText('Send'));

    // Verify message appears
    await waitFor(() => {
      expect(screen.getByText('Test message')).toBeInTheDocument();
    });
  });
});
```

### Performance Tests

#### Bundle Size Monitoring
```typescript
describe('Performance Regression Tests', () => {
  it('should not exceed bundle size limits', async () => {
    const stats = await getBuildStats();
    const mainChunkSize = stats.assets
      .find(asset => asset.name.includes('index'))
      ?.size || 0;

    // After refactoring, main chunk should be under 2MB
    expect(mainChunkSize).toBeLessThan(2 * 1024 * 1024);
  });

  it('should load tools within performance budget', async () => {
    const startTime = performance.now();
    
    const { result } = renderHook(() => useBuiltInTool(), {
      wrapper: BrowserToolProvider,
    });

    await waitFor(() => {
      expect(result.current.tools.length).toBeGreaterThan(0);
    });

    const loadTime = performance.now() - startTime;
    expect(loadTime).toBeLessThan(1000); // 1 second budget
  });
});
```

### Worker Compatibility Tests

#### Web Worker Integration
```typescript
describe('Worker Compatibility Tests', () => {
  it('should load MCP servers in worker context', async () => {
    const worker = new Worker('/src/lib/web-mcp/mcp-worker.ts', { 
      type: 'module' 
    });

    const response = await sendWorkerMessage(worker, {
      id: 'test-1',
      type: 'loadServer',
      serverName: 'content-store',
    });

    expect(response.result).toBeDefined();
    expect(response.error).toBeUndefined();
  });

  it('should handle tool calls through worker', async () => {
    const worker = createTestWorker();
    
    const response = await sendWorkerMessage(worker, {
      id: 'test-2',
      type: 'callTool',
      serverName: 'content-store',
      toolName: 'addContent',
      args: { content: 'test', storeId: 'test-store' },
    });

    expect(response.result?.content).toBeDefined();
  });
});
```

## Test Utilities and Helpers

### Mock Factories
```typescript
// test-utils/mock-factories.ts
export const createMockSession = (overrides = {}) => ({
  id: 'session-123',
  title: 'Test Session',
  assistants: [createMockAssistant()],
  createdAt: new Date(),
  ...overrides,
});

export const createMockAssistant = (overrides = {}) => ({
  id: 'assistant-123',
  name: 'Test Assistant',
  systemPrompt: 'You are a helpful assistant',
  ...overrides,
});

export const createMockTool = (overrides = {}) => ({
  name: 'testTool',
  description: 'A test tool',
  inputSchema: {
    type: 'object',
    properties: {},
    required: [],
  },
  execute: vi.fn().mockResolvedValue('success'),
  ...overrides,
});
```

### Test Wrappers
```typescript
// test-utils/test-wrappers.tsx
export const TestContextWrapper = ({ children, session = null }) => (
  <MemoryRouter>
    <SessionContext.Provider value={{ current: session }}>
      <AssistantContext.Provider value={mockAssistantContext}>
        <BuiltInToolProvider>
          {children}
        </BuiltInToolProvider>
      </AssistantContext.Provider>
    </SessionContext.Provider>
  </MemoryRouter>
);

export const FullTestWrapper = ({ children }) => (
  <TestContextWrapper>
    <BrowserToolProvider>
      <WebMCPProvider>
        {children}
      </WebMCPProvider>
    </BrowserToolProvider>
  </TestContextWrapper>
);
```

### Database Test Utilities
```typescript
// test-utils/db-test-utils.ts
export const setupTestDatabase = async () => {
  const testDb = new LocalDatabase('test-db');
  await testDb.open();
  return testDb;
};

export const clearTestDatabase = async (db) => {
  await Promise.all([
    db.assistants.clear(),
    db.sessions.clear(),
    db.messages.clear(),
    db.groups.clear(),
  ]);
};

export const seedTestData = async (db) => {
  const assistant = await db.assistants.add({
    name: 'Test Assistant',
    systemPrompt: 'Test prompt',
  });

  const session = await db.sessions.add({
    title: 'Test Session',
    assistants: [{ id: assistant, name: 'Test Assistant' }],
  });

  return { assistant, session };
};
```

## Automated Quality Gates

### Pre-commit Hooks
```bash
#!/bin/bash
# .husky/pre-commit

echo "Running pre-commit quality gates..."

# 1. Lint check
pnpm lint || exit 1

# 2. Type check
pnpm type-check || exit 1

# 3. Unit tests
pnpm test:unit --run || exit 1

# 4. Build verification
pnpm build || exit 1

echo "✅ All quality gates passed"
```

### CI/CD Pipeline Tests
```yaml
# .github/workflows/refactoring-tests.yml
name: Refactoring Quality Gates

on:
  pull_request:
    paths:
      - 'src/features/tools/BrowserToolProvider.tsx'
      - 'src/features/chat/Chat.tsx'
      - 'src/lib/db.ts'
      - 'src-tauri/src/mcp.rs'
      - 'src-tauri/src/mcp/builtin/filesystem.rs'

jobs:
  test-matrix:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        test-type: [unit, integration, regression]
    
    steps:
      - uses: actions/checkout@v3
      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: '18'
          
      - name: Install dependencies
        run: pnpm install
        
      - name: Run ${{ matrix.test-type }} tests
        run: pnpm test:${{ matrix.test-type }}
        
      - name: Check bundle size
        if: matrix.test-type == 'integration'
        run: |
          pnpm build
          node scripts/check-bundle-size.js
```

## Measurement and Reporting

### Test Coverage Requirements
- **Unit Tests**: >80% coverage for extracted modules
- **Integration Tests**: >90% coverage for critical paths  
- **Regression Tests**: 100% coverage for existing features

### Performance Benchmarks
- **Build Time**: Should not increase by >10%
- **Bundle Size**: Main chunk <2MB after refactoring
- **Test Execution**: All tests complete in <60 seconds

### Quality Metrics
- **Code Duplication**: Reduce by >70% in target files
- **Cyclomatic Complexity**: <10 per function
- **File Size**: All files <500 lines after refactoring

### Reporting Dashboard
```typescript
// scripts/generate-test-report.ts
interface TestReport {
  timestamp: string;
  phase: string;
  metrics: {
    coverage: Record<string, number>;
    performance: {
      buildTime: number;
      bundleSize: number;
      testExecutionTime: number;
    };
    quality: {
      duplicationReduction: number;
      filesSplit: number;
      lintErrors: number;
    };
  };
  regressions: string[];
  recommendations: string[];
}

export const generateTestReport = async (): Promise<TestReport> => {
  // Implementation for collecting and reporting metrics
};
```

## Emergency Procedures

### Rollback Process
1. **Immediate**: Revert last commit if tests fail
2. **Investigation**: Run isolated test suites to identify issues  
3. **Recovery**: Apply targeted fixes or rollback entire phase
4. **Verification**: Re-run full test suite before continuing

### Debugging Failed Refactoring
```bash
# Debug script for failed refactoring
#!/bin/bash

echo "🔍 Debugging refactoring failure..."

# Check build status
pnpm build 2>&1 | tee build-debug.log

# Run tests with verbose output
pnpm test --verbose --no-coverage 2>&1 | tee test-debug.log

# Check for circular dependencies
pnpm run dependency-cruiser src --output-type err

# Analyze bundle
pnpm run webpack-bundle-analyzer

echo "Debug logs saved to *-debug.log"
```

This test framework ensures safe, incremental refactoring with comprehensive validation at each step.