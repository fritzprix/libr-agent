import { GoogleGenAI } from '@google/genai';

async function main() {
  // API key should be set in environment or fetch it manually.
  // For test, just see if we can create a tiny cache.
  const apiKey = process.env.GEMINI_API_KEY;
  if (!apiKey) {
    console.error('No API key');
    return;
  }

  const ai = new GoogleGenAI({ apiKey });
  try {
    console.log('Creating cache...');
    const response = await ai.caches.create({
      model: 'gemini-2.5-flash',
      config: {
        systemInstruction: 'You are a helpful assistant.',
        contents: [
          {
            role: 'user',
            parts: [
              {
                // Gemini context caching requires at least 32K tokens.
                // This payload (~150K chars ≈ 37K tokens) satisfies that minimum.
                text: 'LibrAgent is a next-generation desktop AI agent platform combining Tauri with React. '
                  .repeat(1800),
              },
            ],
          },
        ],
      },
    });
    console.log('Cache created!', response.name);
  } catch (e) {
    console.error('Failed:', e);
  }
}
main();
