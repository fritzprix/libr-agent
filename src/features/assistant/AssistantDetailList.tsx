'use client';

import AssistantList from './List';

export default function AssistantDetailList() {
  return (
    <div className="flex h-full overflow-auto flex-col md:flex-row">
      <AssistantList />
    </div>
  );
}
