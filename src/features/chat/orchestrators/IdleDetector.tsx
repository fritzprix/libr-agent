import { useScheduler } from "@/context/SchedulerContext";
import { useEffect, useRef } from "react";
import { toast } from "sonner";




function IdleDetector() {
    const { idle } = useScheduler();
    const lastIdleRef = useRef<boolean>(idle);

    useEffect(() => {
        if(lastIdleRef.current !== idle) {
            lastIdleRef.current = idle;
            toast.info(`New Idle State ${idle}`);
        }
    },[idle]);
    return null;
}

export { IdleDetector };