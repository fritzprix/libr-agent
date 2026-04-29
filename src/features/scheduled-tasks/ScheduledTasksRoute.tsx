import { AssistantContextProvider } from '@/context/AssistantContext';
import { ScheduledTasksPage } from './ScheduledTasksPage';

export default function ScheduledTasksRoute() {
  return (
    <AssistantContextProvider>
      <ScheduledTasksPage />
    </AssistantContextProvider>
  );
}
