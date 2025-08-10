/**
 * 🧪 Web MCP Integration Test Script
 *
 * This script demonstrates how to test and validate the Web MCP system integration.
 * It can be used for development, debugging, and validation purposes.
 */

import { createWebMCPProxy } from './mcp-proxy';
import { getLogger } from '../logger';
import { SearchResult } from '@/models/search-engine';

const logger = getLogger('WebMCPTest');

interface TestResult {
  name: string;
  success: boolean;
  duration: number;
  result?: unknown;
  error?: string;
}

class WebMCPIntegrationTest {
  private proxy: ReturnType<typeof createWebMCPProxy> | null = null;
  private results: TestResult[] = [];

  async runAllTests(): Promise<TestResult[]> {
    logger.info('Starting Web MCP integration tests');
    this.results = [];

    try {
      // Initialize proxy
      await this.runTest('Initialize Proxy', () => this.testInitialization());

      // Test server loading
      await this.runTest('Load Calculator Server', () =>
        this.testLoadCalculatorServer(),
      );
      await this.runTest('Load Filesystem Server', () =>
        this.testLoadFilesystemServer(),
      );

      // Test tool discovery
      await this.runTest('List All Tools', () => this.testListAllTools());
      await this.runTest('List Calculator Tools', () =>
        this.testListCalculatorTools(),
      );

      // Test calculator operations
      await this.runTest('Calculator Add', () => this.testCalculatorAdd());
      await this.runTest('Calculator Multiply', () =>
        this.testCalculatorMultiply(),
      );
      await this.runTest('Calculator Power', () => this.testCalculatorPower());
      await this.runTest('Calculator Factorial', () =>
        this.testCalculatorFactorial(),
      );

      // Test error handling
      await this.runTest('Calculator Division by Zero', () =>
        this.testCalculatorDivisionByZero(),
      );
      await this.runTest('Invalid Tool Call', () => this.testInvalidToolCall());

      // Test filesystem operations (if available)
      await this.runTest('Filesystem File Exists', () =>
        this.testFilesystemFileExists(),
      );

      // Test file-store operations
      await this.runTest('FileStore Module', () => this.testFileStoreModule());
    } catch (error) {
      logger.error('Test suite failed', error);
    } finally {
      // Cleanup
      if (this.proxy) {
        this.proxy.cleanup();
      }
    }

    this.logResults();
    return this.results;
  }

  private async runTest(
    name: string,
    testFn: () => Promise<unknown>,
  ): Promise<void> {
    const startTime = Date.now();
    logger.debug(`Running test: ${name}`);

    try {
      const result = await testFn();
      const duration = Date.now() - startTime;

      this.results.push({
        name,
        success: true,
        duration,
        result,
      });

      logger.info(`✅ Test passed: ${name} (${duration}ms)`);
    } catch (error) {
      const duration = Date.now() - startTime;
      const errorMessage =
        error instanceof Error ? error.message : String(error);

      this.results.push({
        name,
        success: false,
        duration,
        error: errorMessage,
      });

      logger.error(`❌ Test failed: ${name} (${duration}ms)`, error);
    }
  }

  private async testInitialization(): Promise<string> {
    this.proxy = createWebMCPProxy({
      workerPath: '/workers/mcp-worker.js',
      timeout: 30000,
    });

    await this.proxy.initialize();
    const status = this.proxy.getStatus();

    if (!status.initialized) {
      throw new Error('Proxy failed to initialize');
    }

    return 'Proxy initialized successfully';
  }

  private async testLoadCalculatorServer(): Promise<unknown> {
    if (!this.proxy) throw new Error('Proxy not initialized');

    const result = await this.proxy.loadServer('calculator');

    if (!result.name || result.name !== 'calculator') {
      throw new Error('Calculator server not loaded correctly');
    }

    return result;
  }

  private async testLoadFilesystemServer(): Promise<unknown> {
    if (!this.proxy) throw new Error('Proxy not initialized');

    const result = await this.proxy.loadServer('filesystem');

    if (!result.name || result.name !== 'filesystem') {
      throw new Error('Filesystem server not loaded correctly');
    }

    return result;
  }

  private async testListAllTools(): Promise<unknown> {
    if (!this.proxy) throw new Error('Proxy not initialized');

    const tools = await this.proxy.listAllTools();

    if (!Array.isArray(tools) || tools.length === 0) {
      throw new Error('No tools found');
    }

    // Check for expected tools
    const calculatorTools = tools.filter((tool) =>
      tool.name.startsWith('calculator__'),
    );
    const filesystemTools = tools.filter((tool) =>
      tool.name.startsWith('filesystem__'),
    );

    if (calculatorTools.length === 0) {
      throw new Error('No calculator tools found');
    }

    if (filesystemTools.length === 0) {
      throw new Error('No filesystem tools found');
    }

    return {
      totalTools: tools.length,
      calculatorTools: calculatorTools.length,
      filesystemTools: filesystemTools.length,
    };
  }

  private async testListCalculatorTools(): Promise<unknown> {
    if (!this.proxy) throw new Error('Proxy not initialized');

    const tools = await this.proxy.listTools('calculator');

    if (!Array.isArray(tools) || tools.length === 0) {
      throw new Error('No calculator tools found');
    }

    // Check for specific tools
    const expectedTools = [
      'add',
      'subtract',
      'multiply',
      'divide',
      'power',
      'sqrt',
      'factorial',
    ];
    const actualToolNames = tools.map((tool) =>
      tool.name.replace('calculator__', ''),
    );

    for (const expectedTool of expectedTools) {
      if (!actualToolNames.includes(expectedTool)) {
        throw new Error(`Expected tool ${expectedTool} not found`);
      }
    }

    return {
      toolCount: tools.length,
      tools: actualToolNames,
    };
  }

  private async testCalculatorAdd(): Promise<unknown> {
    if (!this.proxy) throw new Error('Proxy not initialized');

    const result = await this.proxy.callTool('calculator', 'add', {
      a: 5,
      b: 3,
    });

    if (typeof result !== 'object' || result === null) {
      throw new Error('Invalid result format');
    }

    const typedResult = result as { result: number; operation: string };

    if (typedResult.result !== 8) {
      throw new Error(`Expected result 8, got ${typedResult.result}`);
    }

    if (typedResult.operation !== 'addition') {
      throw new Error(
        `Expected operation 'addition', got ${typedResult.operation}`,
      );
    }

    return result;
  }

  private async testCalculatorMultiply(): Promise<unknown> {
    if (!this.proxy) throw new Error('Proxy not initialized');

    const result = await this.proxy.callTool('calculator', 'multiply', {
      a: 7,
      b: 6,
    });

    const typedResult = result as { result: number };

    if (typedResult.result !== 42) {
      throw new Error(`Expected result 42, got ${typedResult.result}`);
    }

    return result;
  }

  private async testCalculatorPower(): Promise<unknown> {
    if (!this.proxy) throw new Error('Proxy not initialized');

    const result = await this.proxy.callTool('calculator', 'power', {
      base: 2,
      exponent: 8,
    });

    const typedResult = result as { result: number };

    if (typedResult.result !== 256) {
      throw new Error(`Expected result 256, got ${typedResult.result}`);
    }

    return result;
  }

  private async testCalculatorFactorial(): Promise<unknown> {
    if (!this.proxy) throw new Error('Proxy not initialized');

    const result = await this.proxy.callTool('calculator', 'factorial', {
      n: 5,
    });

    const typedResult = result as { result: number };

    if (typedResult.result !== 120) {
      throw new Error(`Expected result 120, got ${typedResult.result}`);
    }

    return result;
  }

  private async testCalculatorDivisionByZero(): Promise<unknown> {
    if (!this.proxy) throw new Error('Proxy not initialized');

    try {
      await this.proxy.callTool('calculator', 'divide', { a: 10, b: 0 });
      throw new Error('Expected division by zero to throw an error');
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);

      if (!errorMessage.includes('Division by zero')) {
        throw new Error(
          `Expected division by zero error, got: ${errorMessage}`,
        );
      }

      return { expectedError: true, message: errorMessage };
    }
  }

  private async testInvalidToolCall(): Promise<unknown> {
    if (!this.proxy) throw new Error('Proxy not initialized');

    try {
      await this.proxy.callTool('calculator', 'nonexistent_tool', {});
      throw new Error('Expected invalid tool call to throw an error');
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);

      if (!errorMessage.includes('Unknown tool')) {
        throw new Error(`Expected unknown tool error, got: ${errorMessage}`);
      }

      return { expectedError: true, message: errorMessage };
    }
  }

  private async testFilesystemFileExists(): Promise<unknown> {
    if (!this.proxy) throw new Error('Proxy not initialized');

    // Test with a path that should exist (or handle gracefully if it doesn't)
    const result = await this.proxy.callTool('filesystem', 'file_exists', {
      path: '/tmp',
    });

    if (typeof result !== 'object' || result === null) {
      throw new Error('Invalid result format');
    }

    const typedResult = result as { success: boolean; exists: boolean };

    if (typeof typedResult.success !== 'boolean') {
      throw new Error('Result should have success boolean property');
    }

    if (typeof typedResult.exists !== 'boolean') {
      throw new Error('Result should have exists boolean property');
    }

    return result;
  }

  private async testFileStoreModule(): Promise<unknown> {
    if (!this.proxy) throw new Error('Proxy not initialized');

    // 서버 로드
    await this.proxy.loadServer('file-store');

    // 1. 스토어 생성 테스트
    const { storeId } = (await this.proxy.callTool(
      'file-store',
      'createStore',
      {
        metadata: {
          name: 'BM25 Test Store',
          description: 'Testing BM25 search functionality',
        },
      },
    )) as { storeId: string };

    logger.info('Store created successfully', { storeId });

    // 2. 다양한 컨텐츠 추가 (BM25 테스트용)
    const testDocuments = [
      {
        filename: 'ml_basics.txt',
        content: `
          Machine learning is a subset of artificial intelligence (AI).
          It involves training algorithms on data to make predictions.
          Supervised learning uses labeled data for training.
          Unsupervised learning finds patterns in unlabeled data.
        `,
      },
      {
        filename: 'deep_learning.txt',
        content: `
          Deep learning uses neural networks with multiple layers.
          Convolutional neural networks are great for image processing.
          Recurrent neural networks handle sequential data well.
          Transformers have revolutionized natural language processing.
        `,
      },
      {
        filename: 'nlp_guide.txt',
        content: `
          Natural language processing helps computers understand text.
          Tokenization breaks text into words or subwords.
          Named entity recognition identifies important entities.
          Sentiment analysis determines emotional tone of text.
        `,
      },
    ];

    const contentIds: string[] = [];

    for (const doc of testDocuments) {
      const { contentId, chunkCount } = (await this.proxy.callTool(
        'file-store',
        'addContent',
        {
          storeId,
          content: doc.content,
          metadata: {
            filename: doc.filename,
            mimeType: 'text/plain',
            size: doc.content.length,
            uploadedAt: new Date().toISOString(),
          },
        },
      )) as { contentId: string; chunkCount: number };

      contentIds.push(contentId);
      logger.info('Content added', {
        filename: doc.filename,
        contentId,
        chunkCount,
      });
    }

    // 3. BM25 검색 테스트 (다양한 쿼리)
    const searchQueries = [
      {
        query: 'neural networks',
        expectedTerms: ['neural', 'networks', 'deep'],
      },
      {
        query: 'machine learning algorithms',
        expectedTerms: ['machine', 'learning', 'algorithms'],
      },
      {
        query: 'text processing NLP',
        expectedTerms: ['text', 'processing', 'nlp'],
      },
      {
        query: 'supervised unsupervised',
        expectedTerms: ['supervised', 'unsupervised'],
      },
    ];

    for (const { query, expectedTerms } of searchQueries) {
      const { results } = (await this.proxy.callTool(
        'file-store',
        'similaritySearch',
        {
          storeId,
          query,
          options: { topN: 3, searchType: 'keyword' },
        },
      )) as { results: SearchResult[] };

      logger.info('BM25 search completed', {
        query,
        resultCount: results.length,
        topScore: results[0]?.score || 0,
      });

      const hasRelevantResults = results.some((result) =>
        expectedTerms.some((term) =>
          result.context.toLowerCase().includes(term.toLowerCase()),
        ),
      );

      if (!hasRelevantResults && results.length > 0) {
        logger.warn('Search may not be working correctly', { query, results });
      }
    }

    // 4. 엣지 케이스 테스트
    const { results: emptyResults } = (await this.proxy.callTool(
      'file-store',
      'similaritySearch',
      {
        storeId,
        query: 'nonexistent random terms xyz123',
        options: { topN: 5 },
      },
    )) as { results: SearchResult[] };

    logger.info('Empty query test', {
      query: 'nonexistent terms',
      resultCount: emptyResults.length,
    });

    // 5. 성능 테스트 (검색 속도)
    const startTime = Date.now();

    await Promise.all([
      this.proxy.callTool('file-store', 'similaritySearch', {
        storeId,
        query: 'machine learning',
        options: { topN: 5 },
      }),
      this.proxy.callTool('file-store', 'similaritySearch', {
        storeId,
        query: 'deep networks',
        options: { topN: 5 },
      }),
      this.proxy.callTool('file-store', 'similaritySearch', {
        storeId,
        query: 'text processing',
        options: { topN: 5 },
      }),
    ]);

    const searchTime = Date.now() - startTime;
    logger.info('Concurrent search performance', {
      searchTime,
      searchCount: 3,
    });

    const allTestsPassed =
      storeId &&
      contentIds.length === testDocuments.length &&
      searchTime < 1000; // 3개 검색이 1초 이내

    logger.info('BM25 tests completed', {
      success: allTestsPassed,
      searchTime,
      contentCount: contentIds.length,
    });

    return allTestsPassed;
  }

  private logResults(): void {
    const totalTests = this.results.length;
    const passedTests = this.results.filter((r) => r.success).length;
    const failedTests = totalTests - passedTests;
    const totalTime = this.results.reduce((sum, r) => sum + r.duration, 0);

    logger.info('📊 Test Results Summary', {
      total: totalTests,
      passed: passedTests,
      failed: failedTests,
      duration: `${totalTime}ms`,
      successRate: `${((passedTests / totalTests) * 100).toFixed(1)}%`,
    });

    if (failedTests > 0) {
      logger.error('❌ Failed Tests:');
      this.results
        .filter((r) => !r.success)
        .forEach((r) => {
          logger.error(`  - ${r.name}: ${r.error}`);
        });
    }

    logger.info('✅ Passed Tests:');
    this.results
      .filter((r) => r.success)
      .forEach((r) => {
        logger.info(`  - ${r.name} (${r.duration}ms)`);
      });
  }

  /**
   * Get test results as a formatted report
   */
  getReport(): string {
    const totalTests = this.results.length;
    const passedTests = this.results.filter((r) => r.success).length;
    const failedTests = totalTests - passedTests;
    const totalTime = this.results.reduce((sum, r) => sum + r.duration, 0);

    let report = '# Web MCP Integration Test Report\n\n';
    report += `**Total Tests:** ${totalTests}\n`;
    report += `**Passed:** ${passedTests}\n`;
    report += `**Failed:** ${failedTests}\n`;
    report += `**Success Rate:** ${((passedTests / totalTests) * 100).toFixed(1)}%\n`;
    report += `**Total Duration:** ${totalTime}ms\n\n`;

    if (failedTests > 0) {
      report += '## Failed Tests\n\n';
      this.results
        .filter((r) => !r.success)
        .forEach((r) => {
          report += `- **${r.name}** (${r.duration}ms): ${r.error}\n`;
        });
      report += '\n';
    }

    report += '## Passed Tests\n\n';
    this.results
      .filter((r) => r.success)
      .forEach((r) => {
        report += `- **${r.name}** (${r.duration}ms)\n`;
      });

    return report;
  }
}

/**
 * Run the complete Web MCP integration test suite
 */
export async function runWebMCPTests(): Promise<TestResult[]> {
  const test = new WebMCPIntegrationTest();
  return await test.runAllTests();
}

/**
 * Quick test to verify Web MCP is working
 */
export async function quickWebMCPTest(): Promise<boolean> {
  try {
    const proxy = createWebMCPProxy({
      workerPath: '/workers/mcp-worker.js',
      timeout: 10000,
    });

    await proxy.initialize();
    await proxy.loadServer('calculator');
    const result = await proxy.callTool('calculator', 'add', { a: 2, b: 3 });

    proxy.cleanup();

    const typedResult = result as { result: number };
    return typedResult.result === 5;
  } catch (error) {
    logger.error('Quick test failed', error);
    return false;
  }
}

export { WebMCPIntegrationTest };
export type { TestResult };
