import { useTranslation } from 'react-i18next';
import { Loader2 } from 'lucide-react';
import { Badge, Checkbox, ScrollArea } from '@/components/ui';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';
import type { SkillImportConflict } from '@/types/skills';
import type { PendingInstall } from '../skills-management-types';

interface SkillsConflictDialogProps {
  pendingInstall: PendingInstall | null;
  userConflicts: SkillImportConflict[];
  systemConflicts: SkillImportConflict[];
  isInstallingLocal: boolean;
  isInstallingGithub: boolean;
  onOpenChange: (open: boolean) => void;
  onToggleOverwrite: (skillName: string) => void;
  onApproveHooksChange: (approved: boolean) => void;
  onConfirm: () => Promise<void> | void;
  onConfirmWithoutHooks: () => Promise<void> | void;
}

export function SkillsConflictDialog({
  pendingInstall,
  userConflicts,
  systemConflicts,
  isInstallingLocal,
  isInstallingGithub,
  onOpenChange,
  onToggleOverwrite,
  onApproveHooksChange,
  onConfirm,
  onConfirmWithoutHooks,
}: SkillsConflictDialogProps) {
  const { t } = useTranslation('common');
  const hasHooks = (pendingInstall?.preview.hooks.length ?? 0) > 0;
  const mcpServers = pendingInstall?.preview.mcpServers ?? [];
  const hasMcp = mcpServers.length > 0;
  const requiresApproval = hasHooks || hasMcp;
  const discoveredSkills = pendingInstall?.preview.discoveredSkills ?? [];
  const title = requiresApproval
    ? t(
        'settings.skills.installConfirmTitle',
        'Review skills, hooks, and MCP servers',
      )
    : t('settings.skills.conflictTitle', 'Replace conflicting skills?');

  return (
    <AlertDialog open={pendingInstall !== null} onOpenChange={onOpenChange}>
      <AlertDialogContent className="grid max-h-[85vh] max-w-2xl grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden p-0">
        <AlertDialogHeader className="px-6 pt-6 pb-4">
          <AlertDialogTitle>{title}</AlertDialogTitle>
          <AlertDialogDescription>
            {requiresApproval
              ? t(
                  'settings.skills.installConfirmDescription',
                  'Confirm which skills to install and whether approved plugin hooks and MCP servers may run during your sessions.',
                )
              : t(
                  'settings.skills.conflictDescription',
                  'Choose which existing user skills should be overwritten. Bundled system skill collisions will be skipped automatically.',
                )}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <div className="min-h-0 overflow-hidden px-6">
          <ScrollArea className="h-full pr-4">
            <div className="space-y-3 pb-1">
              {pendingInstall?.preview.pluginManifest ? (
                <div className="rounded-md border bg-muted/20 p-3 text-sm space-y-1">
                  <div className="flex min-w-0 flex-wrap items-center gap-2">
                    <p className="font-medium">
                      {pendingInstall.preview.pluginManifest.name}
                    </p>
                    {pendingInstall.preview.pluginManifest.version ? (
                      <Badge variant="secondary">
                        v{pendingInstall.preview.pluginManifest.version}
                      </Badge>
                    ) : null}
                  </div>
                  {pendingInstall.preview.pluginManifest.description ? (
                    <p className="text-xs text-muted-foreground">
                      {pendingInstall.preview.pluginManifest.description}
                    </p>
                  ) : null}
                </div>
              ) : null}
              {pendingInstall?.preview.pluginCommands &&
              pendingInstall.preview.pluginCommands.length > 0 ? (
                <div className="rounded-md border bg-muted/20 p-3 text-sm space-y-2">
                  <p className="font-medium">
                    {t(
                      'settings.skills.commandsToInstall',
                      'Slash commands to install',
                    )}
                  </p>
                  <div className="flex flex-wrap gap-1.5">
                    {pendingInstall.preview.pluginCommands.map((command) => (
                      <Badge key={command} variant="secondary">
                        /{command}
                      </Badge>
                    ))}
                  </div>
                </div>
              ) : null}
              {pendingInstall?.preview.pluginAgents &&
              pendingInstall.preview.pluginAgents.length > 0 ? (
                <div className="rounded-md border bg-muted/20 p-3 text-sm space-y-2">
                  <p className="font-medium">
                    {t(
                      'settings.skills.agentsToInstall',
                      'Subagents to install',
                    )}
                  </p>
                  {pendingInstall.preview.pluginAgents.map((agent) => (
                    <div key={agent.name}>
                      <p className="font-medium">{agent.name}</p>
                      {agent.description ? (
                        <p className="text-xs text-muted-foreground">
                          {agent.description}
                        </p>
                      ) : null}
                      {agent.grantsAllServices ? (
                        <p className="text-xs text-amber-600 dark:text-amber-400">
                          {t(
                            'settings.skills.agentGrantsAllServices',
                            'No tools line: all LibrAgent services will be granted.',
                          )}
                        </p>
                      ) : agent.grantedServices.length > 0 ? (
                        <p className="text-xs text-muted-foreground">
                          {t(
                            'settings.skills.agentGrantedServices',
                            'Granted services: {{services}}',
                            { services: agent.grantedServices.join(', ') },
                          )}
                        </p>
                      ) : (
                        <p className="text-xs text-muted-foreground">
                          {t(
                            'settings.skills.agentGrantsNoServices',
                            'No services granted (no tool maps).',
                          )}
                        </p>
                      )}
                      {agent.grantsAllServices ||
                      agent.grantedServices.includes('workspace') ? (
                        <p className="text-xs text-amber-600 dark:text-amber-400">
                          {t(
                            'settings.skills.agentWorkspaceGrantNote',
                            'The workspace service includes file writes and shell execution.',
                          )}
                        </p>
                      ) : null}
                      {agent.unmappedTools.length > 0 ? (
                        <p className="text-xs text-destructive">
                          {t(
                            'settings.skills.agentUnmappedTools',
                            'Tools with no mapping are not granted (restricted entries such as Bash(git:*) count as unmapped): {{tools}}',
                            { tools: agent.unmappedTools.join(', ') },
                          )}
                        </p>
                      ) : null}
                    </div>
                  ))}
                </div>
              ) : null}
              {discoveredSkills.length > 0 ? (
                <div className="rounded-md border bg-muted/20 p-3 text-sm space-y-2">
                  <p className="font-medium">
                    {t('settings.skills.skillsToInstall', 'Skills to install')}
                  </p>
                  {discoveredSkills.map((skill) => (
                    <div key={skill.name}>
                      <p className="font-medium">{skill.name}</p>
                      <p className="text-xs text-muted-foreground">
                        {skill.description}
                      </p>
                    </div>
                  ))}
                </div>
              ) : null}
              {hasHooks ? (
                <div className="rounded-md border bg-muted/20 p-3 text-sm space-y-3">
                  <p className="font-medium">
                    {t('settings.skills.hooksToInstall', 'Plugin hooks')}
                  </p>
                  {pendingInstall?.preview.hooks.map((hook, index) => (
                    <div
                      key={`${hook.event}-${hook.command}-${index}`}
                      className="min-w-0 space-y-1 border-t border-border/60 pt-2 first:border-t-0 first:pt-0"
                    >
                      <div className="flex min-w-0 flex-wrap items-center gap-2">
                        <span className="font-medium">{hook.event}</span>
                        {hook.matcher ? (
                          <span className="min-w-0 break-all text-xs text-muted-foreground">
                            {hook.matcher}
                          </span>
                        ) : null}
                        <Badge variant="secondary">
                          {hook.executionStatus === 'executed'
                            ? t(
                                'settings.skills.hookStatusExecuted',
                                'Executed',
                              )
                            : t(
                                'settings.skills.hookStatusStubbed',
                                'Not supported',
                              )}
                        </Badge>
                      </div>
                      <p className="text-xs text-muted-foreground break-all font-mono">
                        {hook.command}
                      </p>
                    </div>
                  ))}
                </div>
              ) : null}
              {hasMcp ? (
                <div className="rounded-md border bg-muted/20 p-3 text-sm space-y-3">
                  <p className="font-medium">
                    {t(
                      'settings.skills.mcpServersToInstall',
                      'Plugin MCP servers',
                    )}
                  </p>
                  {mcpServers.map((server) => (
                    <div
                      key={`${server.name}-${server.transport}`}
                      className="min-w-0 space-y-1 border-t border-border/60 pt-2 first:border-t-0 first:pt-0"
                    >
                      <div className="flex min-w-0 flex-wrap items-center gap-2">
                        <span className="font-medium">{server.name}</span>
                        <Badge variant="secondary">{server.transport}</Badge>
                      </div>
                      <p className="text-xs text-muted-foreground break-all font-mono">
                        {[server.command ?? server.url, server.args]
                          .filter(Boolean)
                          .join(' ')}
                      </p>
                      {server.envNames || server.headerNames ? (
                        <p className="text-xs text-muted-foreground break-all">
                          {server.envNames
                            ? t('settings.skills.mcpEnvNames', {
                                names: server.envNames.join(', '),
                                defaultValue: 'env: {{names}}',
                              })
                            : null}
                          {server.envNames && server.headerNames ? ' · ' : null}
                          {server.headerNames
                            ? t('settings.skills.mcpHeaderNames', {
                                names: server.headerNames.join(', '),
                                defaultValue: 'headers: {{names}}',
                              })
                            : null}
                        </p>
                      ) : null}
                    </div>
                  ))}
                </div>
              ) : null}
              {requiresApproval ? (
                <label className="flex items-start gap-3 cursor-pointer pt-2">
                  <Checkbox
                    checked={pendingInstall?.approveHooks ?? false}
                    onCheckedChange={(checked) =>
                      onApproveHooksChange(checked === true)
                    }
                    className="mt-0.5"
                  />
                  <div>
                    <p className="font-medium">
                      {hasHooks && hasMcp
                        ? t(
                            'settings.skills.approveHooksMcpLabel',
                            'Allow hooks to run and MCP servers to start',
                          )
                        : hasMcp
                          ? t(
                              'settings.skills.approveMcpLabel',
                              'Allow MCP servers to start',
                            )
                          : t(
                              'settings.skills.approveHooksLabel',
                              'Allow hooks to run',
                            )}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {hasMcp
                        ? t(
                            'settings.skills.approveMcpHint',
                            'Approved MCP servers start automatically when a session opens. Approved hook events also run automatically; unsupported ones are skipped.',
                          )
                        : t(
                            'settings.skills.approveHooksHint',
                            'Supported hook events run automatically during the session once approved. Unsupported events are skipped.',
                          )}
                    </p>
                    {pendingInstall?.preview.hooks.some(
                      (hook) => hook.event === 'PreToolUse',
                    ) ? (
                      <p className="text-xs text-amber-600 dark:text-amber-400 mt-1">
                        {t(
                          'settings.skills.approveHooksToolApprovalHint',
                          'This plugin includes PreToolUse hooks that can allow, ask for confirmation on, or block tool calls.',
                        )}
                      </p>
                    ) : null}
                  </div>
                </label>
              ) : null}
              {userConflicts.length > 0 && (
                <div className="rounded-md border bg-muted/20 p-3 text-sm space-y-3">
                  {userConflicts.map((conflict) => {
                    const checked =
                      pendingInstall?.selectedOverwriteNames.includes(
                        conflict.name,
                      ) ?? false;

                    return (
                      <label
                        key={`${conflict.name}-${conflict.existingPath}`}
                        className="flex items-start gap-3 cursor-pointer"
                      >
                        <Checkbox
                          checked={checked}
                          onCheckedChange={() =>
                            onToggleOverwrite(conflict.name)
                          }
                          className="mt-0.5"
                        />
                        <div className="min-w-0">
                          <p className="font-medium">{conflict.name}</p>
                          <p className="text-xs text-muted-foreground">
                            {t(
                              'settings.skills.userConflictWillOverwrite',
                              'Checked items will overwrite the existing user skill.',
                            )}
                          </p>
                          <p className="text-xs text-muted-foreground break-all">
                            {conflict.existingPath}
                          </p>
                        </div>
                      </label>
                    );
                  })}
                </div>
              )}
              {systemConflicts.length > 0 && (
                <div className="rounded-md border bg-muted/20 p-3 text-sm space-y-2">
                  <p className="font-medium">
                    {t(
                      'settings.skills.systemConflictsSkippedTitle',
                      'Bundled skills that will be skipped',
                    )}
                  </p>
                  {systemConflicts.map((conflict) => (
                    <div
                      key={`${conflict.name}-${conflict.existingPath}`}
                      className="flex items-start justify-between gap-3"
                    >
                      <div>
                        <p className="font-medium">{conflict.name}</p>
                        <p className="text-xs text-muted-foreground break-all">
                          {conflict.existingPath}
                        </p>
                      </div>
                      <span className="text-xs uppercase tracking-wide text-muted-foreground">
                        {t('settings.skills.skipped', 'Skipped')}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </ScrollArea>
        </div>
        <AlertDialogFooter className="min-w-0 px-6 pt-4 pb-6 flex-col sm:flex-row sm:flex-wrap sm:justify-end gap-2">
          <AlertDialogCancel disabled={isInstallingLocal || isInstallingGithub}>
            {t('common.cancel', 'Cancel')}
          </AlertDialogCancel>
          {requiresApproval ? (
            <Button
              type="button"
              variant="outline"
              disabled={isInstallingLocal || isInstallingGithub}
              onClick={(event) => {
                event.preventDefault();
                void onConfirmWithoutHooks();
              }}
            >
              {(isInstallingLocal || isInstallingGithub) && (
                <Loader2 className="w-4 h-4 mr-2 animate-spin" />
              )}
              {t(
                'settings.skills.installWithoutHooks',
                'Install without approval',
              )}
            </Button>
          ) : null}
          <AlertDialogAction
            disabled={
              isInstallingLocal ||
              isInstallingGithub ||
              (requiresApproval && !pendingInstall?.approveHooks)
            }
            onClick={(event) => {
              event.preventDefault();
              void onConfirm();
            }}
          >
            {(isInstallingLocal || isInstallingGithub) && (
              <Loader2 className="w-4 h-4 mr-2 animate-spin" />
            )}
            {hasHooks
              ? t(
                  'settings.skills.installWithHooks',
                  'Install and approve hooks',
                )
              : hasMcp
                ? t(
                    'settings.skills.installWithMcp',
                    'Install and approve servers',
                  )
                : userConflicts.length > 0 &&
                    (pendingInstall?.selectedOverwriteNames.length ?? 0) > 0
                  ? t('settings.skills.replaceConflicts', 'Replace and install')
                  : t(
                      'settings.skills.installNonConflicting',
                      'Install remaining skills',
                    )}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
