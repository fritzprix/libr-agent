import { AssistantContextProvider } from '@/context/AssistantContext';
import AssistantList from './List';

export default function AssistantListRoute() {
  return (
    <AssistantContextProvider>
      <AssistantList />
    </AssistantContextProvider>
  );
}
