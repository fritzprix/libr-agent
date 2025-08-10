/**
 * 🧮 Calculator MCP Server Module
 *
 * A basic calculator MCP server that provides arithmetic operations.
 * This serves as an example of how to implement MCP servers for web workers.
 */

import {
  WebMCPServer,
  MCPTool,
  createObjectSchema,
  createNumberSchema,
} from '../../mcp-types';

// Define the tools available in this server
const tools: MCPTool[] = [
  {
    name: 'add',
    description: 'Add two numbers together',
    inputSchema: createObjectSchema({
      description: 'Addition operation parameters',
      properties: {
        a: createNumberSchema({ description: 'First number' }),
        b: createNumberSchema({ description: 'Second number' }),
      },
      required: ['a', 'b'],
    }),
  },
  {
    name: 'subtract',
    description: 'Subtract the second number from the first',
    inputSchema: createObjectSchema({
      description: 'Subtraction operation parameters',
      properties: {
        a: createNumberSchema({ description: 'First number (minuend)' }),
        b: createNumberSchema({ description: 'Second number (subtrahend)' }),
      },
      required: ['a', 'b'],
    }),
  },
  {
    name: 'multiply',
    description: 'Multiply two numbers',
    inputSchema: createObjectSchema({
      description: 'Multiplication operation parameters',
      properties: {
        a: createNumberSchema({ description: 'First number' }),
        b: createNumberSchema({ description: 'Second number' }),
      },
      required: ['a', 'b'],
    }),
  },
  {
    name: 'divide',
    description: 'Divide the first number by the second',
    inputSchema: createObjectSchema({
      description: 'Division operation parameters',
      properties: {
        a: createNumberSchema({ description: 'Dividend' }),
        b: createNumberSchema({ description: 'Divisor (cannot be zero)' }),
      },
      required: ['a', 'b'],
    }),
  },
  {
    name: 'power',
    description: 'Raise the first number to the power of the second',
    inputSchema: createObjectSchema({
      description: 'Power operation parameters',
      properties: {
        base: createNumberSchema({ description: 'Base number' }),
        exponent: createNumberSchema({ description: 'Exponent' }),
      },
      required: ['base', 'exponent'],
    }),
  },
  {
    name: 'sqrt',
    description: 'Calculate the square root of a number',
    inputSchema: createObjectSchema({
      description: 'Square root operation parameters',
      properties: {
        value: createNumberSchema({
          description: 'Number to calculate square root (must be non-negative)',
          minimum: 0,
        }),
      },
      required: ['value'],
    }),
  },
  {
    name: 'factorial',
    description: 'Calculate the factorial of a non-negative integer',
    inputSchema: createObjectSchema({
      description: 'Factorial operation parameters',
      properties: {
        n: createNumberSchema({
          description: 'Non-negative integer',
          minimum: 0,
          maximum: 170,
        }),
      },
      required: ['n'],
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

  switch (name) {
    case 'add': {
      const { a, b } = params;
      if (typeof a !== 'number' || typeof b !== 'number') {
        throw new Error('Invalid arguments: a and b must be numbers');
      }
      const result = a + b;
      return {
        result,
        operation: 'addition',
        operands: [a, b],
        formula: `${a} + ${b} = ${result}`,
      };
    }

    case 'subtract': {
      const { a, b } = params;
      if (typeof a !== 'number' || typeof b !== 'number') {
        throw new Error('Invalid arguments: a and b must be numbers');
      }
      const result = a - b;
      return {
        result,
        operation: 'subtraction',
        operands: [a, b],
        formula: `${a} - ${b} = ${result}`,
      };
    }

    case 'multiply': {
      const { a, b } = params;
      if (typeof a !== 'number' || typeof b !== 'number') {
        throw new Error('Invalid arguments: a and b must be numbers');
      }
      const result = a * b;
      return {
        result,
        operation: 'multiplication',
        operands: [a, b],
        formula: `${a} × ${b} = ${result}`,
      };
    }

    case 'divide': {
      const { a, b } = params;
      if (typeof a !== 'number' || typeof b !== 'number') {
        throw new Error('Invalid arguments: a and b must be numbers');
      }
      if (b === 0) {
        throw new Error('Division by zero is not allowed');
      }
      const result = a / b;
      return {
        result,
        operation: 'division',
        operands: [a, b],
        formula: `${a} ÷ ${b} = ${result}`,
      };
    }

    case 'power': {
      const { base, exponent } = params;
      if (typeof base !== 'number' || typeof exponent !== 'number') {
        throw new Error('Invalid arguments: base and exponent must be numbers');
      }
      const result = Math.pow(base, exponent);
      if (!Number.isFinite(result)) {
        throw new Error('Result is not a finite number');
      }
      return {
        result,
        operation: 'exponentiation',
        operands: [base, exponent],
        formula: `${base}^${exponent} = ${result}`,
      };
    }

    case 'sqrt': {
      const { value } = params;
      if (typeof value !== 'number') {
        throw new Error('Invalid argument: value must be a number');
      }
      if (value < 0) {
        throw new Error('Square root of negative number is not supported');
      }
      const result = Math.sqrt(value);
      return {
        result,
        operation: 'square root',
        operands: [value],
        formula: `√${value} = ${result}`,
      };
    }

    case 'factorial': {
      const { n } = params;
      if (typeof n !== 'number' || !Number.isInteger(n)) {
        throw new Error('Invalid argument: n must be an integer');
      }
      if (n < 0) {
        throw new Error('Factorial is not defined for negative numbers');
      }
      if (n > 170) {
        throw new Error('Factorial is too large for numbers greater than 170');
      }

      let result = 1;
      for (let i = 2; i <= n; i++) {
        result *= i;
      }

      return {
        result,
        operation: 'factorial',
        operands: [n],
        formula: `${n}! = ${result}`,
      };
    }

    default:
      throw new Error(`Unknown tool: ${name}`);
  }
}

// Create and export the MCP server
const calculatorServer: WebMCPServer = {
  name: 'calculator',
  description:
    'Basic calculator operations including arithmetic, power, square root, and factorial',
  version: '1.0.0',
  tools,
  callTool,
};

export default calculatorServer;
