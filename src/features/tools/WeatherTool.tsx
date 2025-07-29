import { LocalService, MCPResponse, useLocalTools } from '@/context/LocalToolContext';
import { createId } from '@paralleldrive/cuid2';
import { useCallback, useEffect, useMemo, useState } from 'react';

// 이 컴포넌트는 UI를 렌더링하지 않고, 날씨 확인 도구만 제공합니다。
export function WeatherTool() {
  const { registerService, unregisterService } = useLocalTools();
  const [unit] = useState<'celsius' | 'fahrenheit'>('celsius');

  const getWeatherHandler = useCallback(
    async (args: unknown): Promise<MCPResponse> => {
      const argsObj = args as Record<string, unknown>;
      const location = argsObj.location as string;
      // 실제 API 호출 로직...
      console.log(`Getting weather for ${location} in ${unit}`);
      const temperature = unit === 'celsius' ? 22 : 72;
      return {
        jsonrpc: "2.0",
        id: createId(),
        success: true,
        result: {
          content: [
            {
              type: 'text',
              text: `Current weather in ${location}: ${temperature}°${unit === 'celsius' ? 'C' : 'F'}`,
            },
          ],
          structuredContent: {
            location,
            temperature,
            unit,
          },
        },
      };
    },
    [unit],
  );

  const weatherService: LocalService = useMemo(
    () => ({
      name: 'weatherService',
      tools: [
        {
          toolDefinition: {
            name: 'get_current_weather',
            description: 'Get the current weather for a given location',
            input_schema: {
              type: 'object' as const,
              properties: {
                location: {
                  type: 'string',
                  description: 'The city and state, e.g. San Francisco, CA',
                },
              },
              required: ['location'],
            },
          },
          handler: getWeatherHandler,
        },
      ],
    }),
    [getWeatherHandler],
  );

  useEffect(() => {
    registerService(weatherService);
    return () => unregisterService(weatherService.name);
  }, [registerService, unregisterService, weatherService]);

  // 이 컴포넌트는 로직만 제공하므로 null을 렌더링합니다。
  // 필요 시 단위를 변경하는 UI를 렌더링할 수도 있습니다.
  return null;
}
