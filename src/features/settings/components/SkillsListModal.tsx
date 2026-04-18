import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  Button,
  Badge,
} from '@/components/ui';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Code, FileText, Lock, Trash2 } from 'lucide-react';
import type { SkillMetadata } from '@/types/skills';

interface SkillsListModalProps {
  isOpen: boolean;
  onClose: () => void;
  systemSkills: SkillMetadata[];
  userSkills: SkillMetadata[];
  deletingSkillName?: string | null;
  onDeleteUserSkill: (skillName: string) => void;
}

export function formatSkillDisplayPath(path: string): string {
  if (path.startsWith('\\\\?\\UNC\\')) {
    return `\\\\${path.slice('\\\\?\\UNC\\'.length)}`;
  }

  if (path.startsWith('\\\\?\\')) {
    return path.slice('\\\\?\\'.length);
  }

  return path;
}

function SkillRow({
  skill,
  action,
}: {
  skill: SkillMetadata;
  action?: ReactNode;
}) {
  return (
    <div className="border rounded-lg p-4 bg-muted/30 flex flex-col gap-2">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          <Code className="w-4 h-4 text-primary shrink-0" />
          <h3 className="font-medium text-foreground truncate">{skill.name}</h3>
          {skill.origin && (
            <Badge variant="outline" className="shrink-0">
              {skill.origin}
            </Badge>
          )}
        </div>
        {action}
      </div>

      {skill.description && (
        <p className="text-sm text-muted-foreground ml-6">
          {skill.description}
        </p>
      )}

      <div className="flex items-center gap-2 mt-2 ml-6 text-xs text-muted-foreground bg-muted/50 p-2 rounded">
        <FileText className="w-3 h-3 shrink-0" />
        <code className="break-all">{formatSkillDisplayPath(skill.path)}</code>
      </div>
    </div>
  );
}

export function SkillsListModal({
  isOpen,
  onClose,
  systemSkills,
  userSkills,
  deletingSkillName,
  onDeleteUserSkill,
}: SkillsListModalProps) {
  const { t } = useTranslation('common');
  const total = systemSkills.length + userSkills.length;

  return (
    <Dialog open={isOpen} onOpenChange={onClose}>
      <DialogContent className="max-w-4xl max-h-[80vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>
            {t('settings.skills.modalTitle', {
              count: total,
              defaultValue: 'Installed Skills ({{count}})',
            })}
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto min-h-0 pr-4">
          <div className="space-y-6 py-4">
            <section className="space-y-4">
              <div className="flex items-center gap-2">
                <Lock className="w-4 h-4 text-muted-foreground" />
                <h3 className="font-semibold">
                  {t(
                    'settings.skills.systemSection',
                    'System Skills (read-only)',
                  )}
                </h3>
              </div>
              {systemSkills.length === 0 ? (
                <div className="text-sm text-muted-foreground rounded-lg border border-dashed p-4">
                  {t(
                    'settings.skills.noSystemSkills',
                    'No system skills found.',
                  )}
                </div>
              ) : (
                systemSkills.map((skill) => (
                  <SkillRow
                    key={skill.path}
                    skill={skill}
                    action={
                      <Badge variant="secondary">
                        {t('settings.skills.locked', 'Locked')}
                      </Badge>
                    }
                  />
                ))
              )}
            </section>

            <section className="space-y-4">
              <div className="flex items-center gap-2">
                <Trash2 className="w-4 h-4 text-muted-foreground" />
                <h3 className="font-semibold">
                  {t('settings.skills.userSection', 'User Skills')}
                </h3>
              </div>
              {userSkills.length === 0 ? (
                <div className="text-sm text-muted-foreground rounded-lg border border-dashed p-4">
                  {t(
                    'settings.skills.noUserSkills',
                    'No user skills installed.',
                  )}
                </div>
              ) : (
                userSkills.map((skill) => (
                  <SkillRow
                    key={skill.path}
                    skill={skill}
                    action={
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        disabled={deletingSkillName === skill.name}
                        onClick={() => onDeleteUserSkill(skill.name)}
                        aria-label={t(
                          'settings.skills.deleteUserSkill',
                          'Delete user skill',
                        )}
                      >
                        <Trash2 className="w-4 h-4 text-destructive" />
                      </Button>
                    }
                  />
                ))
              )}
            </section>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
