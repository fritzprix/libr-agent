import type { AgentSession } from '@/models/agent';

export interface OrgSummary {
  orgId: string;
  orgName: string;
  orgRootSessionId: string;
  rootSession: AgentSession;
  members: AgentSession[];
  memberCount: number;
  busyCount: number;
  updatedAt: Date;
}

function hasExplicitOrgIdentity(
  session: AgentSession,
): session is AgentSession & {
  orgId: string;
  orgName: string;
  orgRootSessionId: string;
} {
  return (
    typeof session.orgId === 'string' &&
    session.orgId.length > 0 &&
    typeof session.orgName === 'string' &&
    session.orgName.length > 0 &&
    typeof session.orgRootSessionId === 'string' &&
    session.orgRootSessionId.length > 0
  );
}

export function selectOrgSummaries(sessions: AgentSession[]): OrgSummary[] {
  const grouped = new Map<string, AgentSession[]>();

  for (const session of sessions) {
    if (!hasExplicitOrgIdentity(session)) {
      continue;
    }

    const members = grouped.get(session.orgId) ?? [];
    members.push(session);
    grouped.set(session.orgId, members);
  }

  const summaries: OrgSummary[] = [];

  for (const [orgId, members] of grouped.entries()) {
    const orgRootSessionId = members[0]?.orgRootSessionId;
    if (!orgRootSessionId) {
      continue;
    }

    const rootSession = members.find(
      (session) => session.id === orgRootSessionId,
    );
    if (!rootSession || !hasExplicitOrgIdentity(rootSession)) {
      continue;
    }

    const updatedAt = members.reduce((latest, session) => {
      const candidate = session.updatedAt ?? session.createdAt;
      return candidate.getTime() > latest.getTime() ? candidate : latest;
    }, rootSession.updatedAt ?? rootSession.createdAt);

    summaries.push({
      orgId,
      orgName: rootSession.orgName,
      orgRootSessionId: rootSession.orgRootSessionId,
      rootSession,
      members: [...members].sort((left, right) => {
        const leftDepth = left.depth ?? 0;
        const rightDepth = right.depth ?? 0;
        if (leftDepth !== rightDepth) {
          return leftDepth - rightDepth;
        }
        return left.createdAt.getTime() - right.createdAt.getTime();
      }),
      memberCount: members.length,
      busyCount: members.reduce(
        (acc, session) => (session.status === 'busy' ? acc + 1 : acc),
        0,
      ),
      updatedAt,
    });
  }

  return summaries.sort(
    (left, right) => right.updatedAt.getTime() - left.updatedAt.getTime(),
  );
}
