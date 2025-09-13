import { useState } from 'react';
import { useWebMCPServer } from '@/hooks/use-web-mcp-server';
import type { PlanningServerProxy } from '@/lib/web-mcp/modules/planning-server';

interface TestResult {
  id: string;
  method: string;
  args: unknown;
  result?: unknown;
  error?: string;
  timestamp: Date;
}

export default function WebMCPTest() {
  const { server, loading, error } =
    useWebMCPServer<PlanningServerProxy>('planning');
  const [testResults, setTestResults] = useState<TestResult[]>([]);
  const [goalInput, setGoalInput] = useState('Test goal creation');
  const [todoInput, setTodoInput] = useState('Test todo item');
  const [observationInput, setObservationInput] = useState('Test observation');
  const [toggleIndex, setToggleIndex] = useState(1);

  const addTestResult = (
    method: string,
    args: unknown,
    result?: unknown,
    error?: string,
  ) => {
    const testResult: TestResult = {
      id: Math.random().toString(36).substr(2, 9),
      method,
      args,
      result,
      error,
      timestamp: new Date(),
    };
    setTestResults((prev) => [testResult, ...prev]);
  };

  const runTest = async (testName: string, testFn: () => Promise<unknown>) => {
    try {
      console.log(`[WebMCPTest] Running test: ${testName}`);
      const result = await testFn();
      console.log(`[WebMCPTest] Test ${testName} success:`, result);
      addTestResult(testName, {}, result);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error(`[WebMCPTest] Test ${testName} failed:`, errorMessage);
      addTestResult(testName, {}, undefined, errorMessage);
    }
  };

  const testCreateGoal = () => {
    if (!server) return;
    runTest('create_goal', () => server.create_goal({ goal: goalInput }));
  };

  const testClearGoal = () => {
    if (!server) return;
    runTest('clear_goal', () => server.clear_goal());
  };

  const testAddTodo = () => {
    if (!server) return;
    runTest('add_todo', () => server.add_todo({ name: todoInput }));
  };

  const testToggleTodo = () => {
    if (!server) return;
    runTest('toggle_todo', () => server.toggle_todo({ index: toggleIndex }));
  };

  const testGetCurrentState = () => {
    if (!server) return;
    runTest('get_current_state', () => server.get_current_state());
  };

  const testAddObservation = () => {
    if (!server) return;
    runTest('add_observation', () =>
      server.add_observation({ observation: observationInput }),
    );
  };

  const testClearTodos = () => {
    if (!server) return;
    runTest('clear_todos', () => server.clear_todos());
  };

  const testClearSession = () => {
    if (!server) return;
    runTest('clear_session', () => server.clear_session());
  };

  const clearResults = () => {
    setTestResults([]);
  };

  if (loading) {
    return (
      <div className="p-6 max-w-4xl mx-auto">
        <h1 className="text-2xl font-bold mb-4">WebMCP Server Test</h1>
        <p className="text-blue-600">Loading planning server...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-6 max-w-4xl mx-auto">
        <h1 className="text-2xl font-bold mb-4">WebMCP Server Test</h1>
        <div className="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded">
          <strong>Error:</strong> {error}
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 max-w-4xl mx-auto">
      <h1 className="text-2xl font-bold mb-4">WebMCP Server Test</h1>

      {/* Server Status */}
      <div className="mb-6 p-4 bg-green-100 border border-green-400 rounded">
        <h2 className="text-lg font-semibold text-green-800">Server Status</h2>
        <p className="text-green-700">✅ Planning server loaded successfully</p>
        <p className="text-sm text-green-600">
          Tools available: {server?.tools?.length || 0}
        </p>
      </div>

      {/* Test Controls */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-6">
        {/* Goal Tests */}
        <div className="space-y-4">
          <h3 className="text-lg font-semibold">Goal Management</h3>

          <div className="space-y-2">
            <label className="block text-sm font-medium">Goal Text:</label>
            <input
              type="text"
              value={goalInput}
              onChange={(e) => setGoalInput(e.target.value)}
              className="w-full px-3 py-2 border border-gray-300 rounded-md"
              placeholder="Enter goal text"
            />
            <button
              onClick={testCreateGoal}
              className="w-full px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600"
            >
              Create Goal
            </button>
          </div>

          <button
            onClick={testClearGoal}
            className="w-full px-4 py-2 bg-red-500 text-white rounded hover:bg-red-600"
          >
            Clear Goal
          </button>
        </div>

        {/* Todo Tests */}
        <div className="space-y-4">
          <h3 className="text-lg font-semibold">Todo Management</h3>

          <div className="space-y-2">
            <label className="block text-sm font-medium">Todo Name:</label>
            <input
              type="text"
              value={todoInput}
              onChange={(e) => setTodoInput(e.target.value)}
              className="w-full px-3 py-2 border border-gray-300 rounded-md"
              placeholder="Enter todo text"
            />
            <button
              onClick={testAddTodo}
              className="w-full px-4 py-2 bg-green-500 text-white rounded hover:bg-green-600"
            >
              Add Todo
            </button>
          </div>

          <div className="space-y-2">
            <label className="block text-sm font-medium">Toggle Index:</label>
            <input
              type="number"
              value={toggleIndex}
              onChange={(e) => setToggleIndex(parseInt(e.target.value) || 1)}
              className="w-full px-3 py-2 border border-gray-300 rounded-md"
              placeholder="1"
              min="1"
            />
            <button
              onClick={testToggleTodo}
              className="w-full px-4 py-2 bg-yellow-500 text-white rounded hover:bg-yellow-600"
            >
              Toggle Todo
            </button>
          </div>

          <button
            onClick={testClearTodos}
            className="w-full px-4 py-2 bg-red-500 text-white rounded hover:bg-red-600"
          >
            Clear All Todos
          </button>
        </div>
      </div>

      {/* Other Tests */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
        <div className="space-y-2">
          <label className="block text-sm font-medium">Observation:</label>
          <input
            type="text"
            value={observationInput}
            onChange={(e) => setObservationInput(e.target.value)}
            className="w-full px-3 py-2 border border-gray-300 rounded-md"
            placeholder="Enter observation"
          />
          <button
            onClick={testAddObservation}
            className="w-full px-4 py-2 bg-purple-500 text-white rounded hover:bg-purple-600"
          >
            Add Observation
          </button>
        </div>

        <button
          onClick={testGetCurrentState}
          className="px-4 py-2 bg-indigo-500 text-white rounded hover:bg-indigo-600"
        >
          Get Current State
        </button>

        <button
          onClick={testClearSession}
          className="px-4 py-2 bg-gray-500 text-white rounded hover:bg-gray-600"
        >
          Clear Session
        </button>
      </div>

      {/* Results */}
      <div className="space-y-4">
        <div className="flex justify-between items-center">
          <h3 className="text-lg font-semibold">
            Test Results ({testResults.length})
          </h3>
          <button
            onClick={clearResults}
            className="px-4 py-2 bg-gray-500 text-white rounded hover:bg-gray-600"
          >
            Clear Results
          </button>
        </div>

        <div className="space-y-2 max-h-96 overflow-y-auto">
          {testResults.length === 0 ? (
            <p className="text-gray-500 italic">
              No test results yet. Run some tests above!
            </p>
          ) : (
            testResults.map((result) => (
              <div
                key={result.id}
                className={`p-4 rounded border ${
                  result.error
                    ? 'bg-red-50 border-red-200'
                    : 'bg-green-50 border-green-200'
                }`}
              >
                <div className="flex justify-between items-start mb-2">
                  <h4 className="font-semibold">
                    {result.method}
                    {result.error ? ' ❌' : ' ✅'}
                  </h4>
                  <span className="text-xs text-gray-500">
                    {result.timestamp.toLocaleTimeString()}
                  </span>
                </div>

                {result.error ? (
                  <p className="text-red-700 text-sm">
                    <strong>Error:</strong> {result.error}
                  </p>
                ) : (
                  <pre className="text-xs bg-white p-2 rounded border overflow-x-auto">
                    {JSON.stringify(result.result, null, 2)}
                  </pre>
                )}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
