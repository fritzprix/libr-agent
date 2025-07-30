import { useScheduler } from '@/context/SchedulerContext';
import { DependencyList, useCallback } from 'react';

/**
 * `useScheduledCallback` 훅: 비동기 콜백을 스케줄링 로직으로 감싸는 커스텀 훅입니다.
 * 제네릭을 사용하여 `any` 타입 없이 타입 안정성을 보장합니다.
 *
 * @param callback - 스케줄링할 비동기 함수입니다.
 * @param deps - 콜백 메모이제이션을 위한 의존성 배열입니다.
 * @returns 원본 콜백과 동일한 시그니처를 가지며, 호출 시 태스크를 큐에 등록하는 새로운 함수를 반환합니다.
 *
 * @example
 * ```typescript
 * const saveData = useScheduledCallback(
 *   async (data: UserData) => {
 *     return await api.saveUser(data);
 *   },
 *   [api]
 * );
 *
 * // 사용 시 원본 함수와 동일한 타입
 * const result = await saveData(userData); // Promise<SaveResult>
 * ```
 */
export function useScheduledCallback<A extends unknown[], R>(
  callback: (...args: A) => Promise<R>,
  deps: DependencyList,
): (...args: A) => Promise<R> {
  const { schedule } = useScheduler();

  return useCallback(
    (...args: A): Promise<R> => {
      // 스케줄러에 태스크 등록 - 불필요한 async/await 제거
      return schedule(() => callback(...args));
    },
    // schedule 함수는 일반적으로 안정적이므로 deps만 포함하는 것을 고려
    // 하지만 안전성을 위해 schedule도 포함
    [schedule, ...deps],
  );
}
