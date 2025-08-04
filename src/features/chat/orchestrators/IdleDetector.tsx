import { useScheduler } from '@/context/SchedulerContext';
import { useEffect, useRef } from 'react';
import { toast } from 'sonner';

function IdleDetector() {
  const { idle } = useScheduler();

  useEffect(() => {
    toast.info(`New Idle State ${idle}`);
  }, [idle]);
  return null;
}

export { IdleDetector };
