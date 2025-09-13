import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  useDnDContext,
  type DragAndDropEvent,
  type DragAndDropPayload,
} from '@/context/DnDContext';
import { getLogger } from '@/lib/logger';

interface ZoneState {
  label: string;
  isOver: boolean;
  lastEvent?: DragAndDropEvent;
  lastPaths?: string[];
}

export default function DnDTestPage() {
  const logger = useMemo(() => getLogger('DnDTestPage'), []);
  const leftRef = useRef<HTMLDivElement>(null);
  const rightRef = useRef<HTMLDivElement>(null);
  const { subscribe } = useDnDContext();

  const [left, setLeft] = useState<ZoneState>({
    label: 'Left Zone',
    isOver: false,
  });
  const [right, setRight] = useState<ZoneState>({
    label: 'Right Zone',
    isOver: false,
  });

  useEffect(() => {
    const handle =
      (zone: 'left' | 'right') =>
      (event: DragAndDropEvent, payload: DragAndDropPayload) => {
        const paths = payload.paths ?? [];
        logger.info('DnD event', { zone, event, payload });
        if (zone === 'left') {
          if (event === 'drag-over') {
            setLeft((s) => ({
              ...s,
              isOver: true,
              lastEvent: event,
              lastPaths: paths,
            }));
            // deactivate the other zone explicitly
            setRight((s) => ({ ...s, isOver: false }));
          } else if (event === 'drop' || event === 'leave') {
            setLeft((s) => ({
              ...s,
              isOver: false,
              lastEvent: event,
              lastPaths: paths,
            }));
            setRight((s) => ({ ...s, isOver: false }));
          }
        } else {
          if (event === 'drag-over') {
            setRight((s) => ({
              ...s,
              isOver: true,
              lastEvent: event,
              lastPaths: paths,
            }));
            // deactivate the other zone explicitly
            setLeft((s) => ({ ...s, isOver: false }));
          } else if (event === 'drop' || event === 'leave') {
            setRight((s) => ({
              ...s,
              isOver: false,
              lastEvent: event,
              lastPaths: paths,
            }));
            setLeft((s) => ({ ...s, isOver: false }));
          }
        }
      };

    const unsubLeft = subscribe(leftRef, handle('left'), { priority: 1 });
    const unsubRight = subscribe(rightRef, handle('right'), { priority: 1 });
    return () => {
      unsubLeft();
      unsubRight();
    };
  }, [logger, subscribe]);

  return (
    <div className="p-6 space-y-6">
      <h1 className="text-xl font-semibold">DnD Test Page</h1>
      <p className="text-sm text-muted-foreground">
        Drag files from your desktop into one of the zones below. The provider
        routes Tauri drag/drop events by position.
      </p>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <DropZoneCard refEl={leftRef} state={left} />
        <DropZoneCard refEl={rightRef} state={right} />
      </div>
    </div>
  );
}

function DropZoneCard({
  refEl,
  state,
}: {
  refEl: React.RefObject<HTMLDivElement>;
  state: ZoneState;
}) {
  return (
    <div
      ref={refEl}
      className={[
        'min-h-48 rounded-md border-2 border-dashed p-4 transition-colors',
        state.isOver ? 'border-blue-500 bg-blue-50/40' : 'border-muted',
      ].join(' ')}
    >
      <div className="font-medium mb-2">{state.label}</div>
      <div className="text-xs text-muted-foreground">
        {state.isOver ? 'drag-over' : 'idle'}
      </div>
      <div className="mt-3">
        <div className="text-xs font-semibold">Last Event</div>
        <div className="text-xs">{state.lastEvent ?? '—'}</div>
      </div>
      <div className="mt-3">
        <div className="text-xs font-semibold">Paths</div>
        {state.lastPaths && state.lastPaths.length > 0 ? (
          <ul className="text-xs list-disc pl-5 break-all">
            {state.lastPaths.map((p) => (
              <li key={p}>{p}</li>
            ))}
          </ul>
        ) : (
          <div className="text-xs text-muted-foreground">No files yet</div>
        )}
      </div>
    </div>
  );
}
