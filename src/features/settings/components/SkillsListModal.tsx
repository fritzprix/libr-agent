import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui';
import { useTranslation } from 'react-i18next';
import { Code, FileText } from 'lucide-react';

interface SkillMetadata {
  name: string;
  description: string;
  path: string;
}

interface SkillsListModalProps {
  isOpen: boolean;
  onClose: () => void;
  skills: SkillMetadata[];
}

export function SkillsListModal({
  isOpen,
  onClose,
  skills,
}: SkillsListModalProps) {
  const { t } = useTranslation('common');

  return (
    <Dialog open={isOpen} onOpenChange={onClose}>
      <DialogContent className="max-w-3xl max-h-[80vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>
            {t('settings.skills.modalTitle', 'Available Skills')} (
            {skills.length})
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto min-h-0 pr-4">
          <div className="space-y-4 py-4">
            {skills.length === 0 ? (
              <div className="text-center text-muted-foreground py-8">
                {t('settings.skills.noSkills', 'No skills found.')}
              </div>
            ) : (
              skills.map((skill) => (
                <div
                  key={skill.path}
                  className="border rounded-lg p-4 bg-muted/30 flex flex-col gap-2"
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <Code className="w-4 h-4 text-primary" />
                      <h3 className="font-medium text-foreground">
                        {skill.name}
                      </h3>
                    </div>
                  </div>

                  {skill.description && (
                    <p className="text-sm text-muted-foreground ml-6">
                      {skill.description}
                    </p>
                  )}

                  <div className="flex items-center gap-2 mt-2 ml-6 text-xs text-muted-foreground bg-muted/50 p-2 rounded">
                    <FileText className="w-3 h-3" />
                    <code className="break-all">{skill.path}</code>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
