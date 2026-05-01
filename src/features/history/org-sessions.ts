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

// ⚡ Bolt: Using a structured tracking object to avoid multiple O(N) reduce passes later
interface OrgTrackingData {
  members: AgentSession[];
  busyCount: number;
  latestUpdate: Date;
}

export function selectOrgSummaries(sessions: AgentSession[]): OrgSummary[] {
  const grouped = new Map<string, OrgTrackingData>();

  for (const session of sessions) {
    if (!hasExplicitOrgIdentity(session)) {
      continue;
    }

    const isBusy = session.status === 'busy';
    const candidateDate = session.updatedAt ?? session.createdAt;

    const data = grouped.get(session.orgId);
    if (data) {
      data.members.push(session);
      if (isBusy) data.busyCount++;
      if (candidateDate.getTime() > data.latestUpdate.getTime()) {
        data.latestUpdate = candidateDate;
      }
    } else {
      grouped.set(session.orgId, {
        members: [session],
        busyCount: isBusy ? 1 : 0,
        latestUpdate: candidateDate,
      });
    }
  }

  const summaries: OrgSummary[] = [];

  for (const [orgId, data] of grouped.entries()) {
    const orgRootSessionId = data.members[0]?.orgRootSessionId;
    if (!orgRootSessionId) {
      continue;
    }

    const rootSession = data.members.find(
      (session) => session.id === orgRootSessionId,
    );
    if (!rootSession || !hasExplicitOrgIdentity(rootSession)) {
      continue;
    }

    summaries.push({
      orgId,
      orgName: rootSession.orgName,
      orgRootSessionId: rootSession.orgRootSessionId,
      rootSession,
      members: [...data.members].sort((left, right) => {
        const leftDepth = left.depth ?? 0;
        const rightDepth = right.depth ?? 0;
        if (leftDepth !== rightDepth) {
          return leftDepth - rightDepth;
        }
        return left.createdAt.getTime() - right.createdAt.getTime();
      }),
      memberCount: data.members.length,
      busyCount: data.busyCount,
      updatedAt: data.latestUpdate,
    });
  }

  return summaries.sort(
    (left, right) => right.updatedAt.getTime() - left.updatedAt.getTime(),
  );
}
