import { useState } from 'react';

export function useChatState() {
  const [showToolsDetail, setShowToolsDetail] = useState(false);

  return {
    showToolsDetail,
    setShowToolsDetail,
  };
}
