import React, {
  createContext,
  useContext,
  useCallback,
  useEffect,
  ReactNode,
  useMemo,
  useState,
  useRef,
} from 'react';
import { useAsyncFn, useQueue } from 'react-use';

// --- 스케줄러 핵심 로직 수정 ---

/**
 * idle 상태가 true로 변경될 때의 디바운스 시간 (밀리초)
 */
const IDLE_DEBOUNCE_MS = 1000;

/**
 * 스케줄러 내부에서 관리될 태스크의 구조입니다.
 * 비동기 task 함수와, 해당 task의 완료/실패를 처리할 resolve/reject 함수를 포함합니다.
 */
interface ManagedTask<R = unknown> {
  task: () => Promise<R>;
  resolve: (value: R | PromiseLike<R>) => void;
  reject: (reason?: unknown) => void;
}

/**
 * SchedulerContextType은 스케줄러가 제공하는 API를 정의합니다.
 * - schedule: 태스크를 예약하고, 해당 태스크의 결과를 반환하는 Promise를 즉시 반환합니다.
 * - idle: 실행 중인 태스크가 없고 큐가 비어있으면 true입니다.
 */
interface SchedulerContextType {
  schedule: <R>(task: () => Promise<R>) => Promise<R>;
  idle: boolean;
}

const SchedulerContext = createContext<SchedulerContextType | undefined>(
  undefined,
);

export const SchedulerProvider: React.FC<{ children: ReactNode }> = ({
  children,
}) => {
  // 내부 태스크 큐는 이제 ManagedTask 객체를 관리합니다.
  const { add, remove, first, size } = useQueue<ManagedTask<unknown>>();

  // 비동기 태스크 실행기는 이제 ManagedTask를 받아 처리합니다.
  const [{ loading }, run] = useAsyncFn(
    async (managedTask: ManagedTask<unknown>) => {
      try {
        // 태스크를 실행하고 결과를 얻습니다.
        const result = await managedTask.task();
        // 해당 태스크에 대한 Promise를 성공 상태로 만듭니다.
        managedTask.resolve(result);
        return result;
      } catch (error: unknown) {
        console.error('Scheduled task failed:', error);
        // 태스크 실패 시 Promise를 실패 상태로 만듭니다.
        managedTask.reject(error);
      }
    },
    [],
  );

  // 유휴 상태이고 큐가 비어있지 않을 때 다음 태스크를 실행합니다.
  useEffect(() => {
    if (first && !loading) {
      run(first).finally(() => {
        remove(); // 태스크 처리(성공/실패) 후 큐에서 제거합니다.
      });
    }
  }, [loading, first, run, remove]);

  const rawIdle = useMemo(() => !loading && size === 0, [loading, size]);

  // Debounced idle state
  const [debouncedIdle, setDebouncedIdle] = useState(rawIdle);
  const debounceTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Cleanup timer on unmount
  useEffect(() => {
    return () => {
      if (debounceTimer.current) {
        clearTimeout(debounceTimer.current);
        debounceTimer.current = null;
      }
    };
  }, []);

  // Handle raw idle changes
  useEffect(() => {
    if (!rawIdle) {
      // 즉시 false로 설정
      if (debounceTimer.current) {
        clearTimeout(debounceTimer.current);
        debounceTimer.current = null;
      }
      if (debouncedIdle) {
        setDebouncedIdle(false);
      }
    } else if (!debouncedIdle) {
      // rawIdle이 true이고 debouncedIdle이 false일 때만 타이머 설정
      debounceTimer.current = setTimeout(() => {
        setDebouncedIdle(true);
        debounceTimer.current = null;
      }, IDLE_DEBOUNCE_MS);
    }
    // debouncedIdle을 의존성에서 제거하여 무한 루프 방지
  }, [rawIdle, debouncedIdle]);

  /**
   * 새로운 태스크를 예약하고, 그 결과를 받을 수 있는 Promise를 반환합니다.
   */
  const schedule = useCallback(
    <R,>(task: () => Promise<R>): Promise<R> => {
      return new Promise<R>((resolve, reject) => {
        // ManagedTask 객체를 생성하여 큐에 추가합니다.
        const managedTask: ManagedTask<R> = {
          task,
          resolve,
          reject: (reason?: unknown) => reject(reason),
        };
        add(managedTask as ManagedTask<unknown>);
      });
    },
    [add],
  );

  const value = useMemo(
    () => ({ schedule, idle: debouncedIdle }),
    [schedule, debouncedIdle],
  );

  return (
    <SchedulerContext.Provider value={value}>
      {children}
    </SchedulerContext.Provider>
  );
};

/**
 * 스케줄러 컨텍스트에 접근하기 위한 `useScheduler` 훅입니다.
 */
export function useScheduler() {
  const context = useContext(SchedulerContext);
  if (context === undefined) {
    throw new Error('useScheduler must be used within a SchedulerProvider');
  }
  return context;
}
