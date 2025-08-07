import { Tooltip } from '@/components/ui';
import { useState } from 'react';
import { GoogleDriveService } from './GoogleDriveService';

function GoogleTool() {
  const [enabled, setEnabled] = useState(false);

  const onChange = () => {
    setEnabled((prev) => !prev);
  };
  return (
    <Tooltip>
      <div className="flex items-center gap-2">
        <span>📁</span>
        <span>Google Drive</span>
        <input type="checkbox" checked={enabled} onChange={onChange} />
        {enabled && <GoogleDriveService />}
      </div>
    </Tooltip>
  );
}

export { GoogleTool };
