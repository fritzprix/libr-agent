import {
  FunctionDeclaration,
  FunctionCallingConfigMode,
  HarmCategory,
  HarmBlockThreshold,
} from '@google/genai';

/**
 * Defines the configuration specific to the Gemini service.
 * @internal
 */
export interface GeminiServiceConfig {
  responseMimeType: string;
  abortSignal?: AbortSignal;
  tools?: Array<{ functionDeclarations: FunctionDeclaration[] }>;
  toolConfig?: {
    functionCallingConfig?: {
      mode: FunctionCallingConfigMode;
    };
  };
  systemInstruction?: Array<{ text: string }>;
  maxOutputTokens?: number;
  temperature?: number;
  thinkingConfig?: {
    thinkingBudget?: number; // -1 (dynamic) | 0 (disabled) | positive number (token count)
    includeThoughts?: boolean; // Include thinking process in response
  };
  safetySettings?: Array<{
    category: HarmCategory;
    threshold: HarmBlockThreshold;
  }>;
}
