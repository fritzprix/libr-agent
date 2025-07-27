import { useLocalTools } from "@/context/LocalToolContext";

interface LocalServicesEditorProps {
  localServices: string[] | undefined;
  onChange: (localServices: string[]) => void;
}

export default function LocalServicesEditor({
  localServices,
  onChange,
}: LocalServicesEditorProps) {
  const { getAvailableServices, getToolsByService } = useLocalTools();

  const handleServiceToggle = (serviceName: string, checked: boolean) => {
    const newLocalServices = checked
      ? [...(localServices || []), serviceName]
      : localServices?.filter((s: string) => s !== serviceName) || [];
    onChange(newLocalServices);
  };

  return (
    <div>
      <label className="text-muted-foreground font-medium">
        로컬 서비스 활성화
      </label>
      <div className="space-y-2 mt-2 p-2 border border-muted rounded-md">
        {getAvailableServices().map((serviceName) => (
          <div key={serviceName}>
            <h4 className="text-sm font-semibold text-foreground mb-1">
              {serviceName}
            </h4>
            <div className="space-y-1 pl-2">
              {getToolsByService(serviceName).map((tool) => (
                <div key={tool.name} className="flex items-center">
                  <input
                    type="checkbox"
                    id={`tool-${tool.name}`}
                    checked={localServices?.includes(serviceName) || false}
                    onChange={(e) =>
                      handleServiceToggle(serviceName, e.target.checked)
                    }
                    className="mr-2"
                  />
                  <label
                    htmlFor={`tool-${tool.name}`}
                    className="text-sm text-muted-foreground"
                  >
                    {tool.name}
                  </label>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}