import {
  createMCPStructuredToolResult,
  createMCPErrorToolResult,
} from '@/lib/mcp-response-utils';
import type { MCPResult } from '@/lib/mcp-types';
import type { IMcpServerService } from '@/lib/services/mcp-server-service';
import type { MCPServerEntity } from '@/models/chat';
import { normalizePagination } from '../utils/pagination';
import type { SearchServersInput, SearchServersOutput } from '../types';
import { createBM25Index, defaultTokenizer } from '@/lib/search/bm25';
import { MCPResponseBuilder } from '@/lib/web-mcp/response-builder';

export async function searchServer(
  mcpService: IMcpServerService,
  args: Record<string, unknown>,
): Promise<MCPResult<SearchServersOutput>> {
  const input: SearchServersInput = {
    query: String(args.query || '').trim(),
    page: args.page !== undefined ? Number(args.page) : 1,
    pageSize: args.pageSize !== undefined ? Number(args.pageSize) : 20,
    searchMode: (args.searchMode as 'bm25' | 'simple') || 'bm25',
    byNameOnly: Boolean(args.byNameOnly ?? true),
    includeInactive: Boolean(args.includeInactive ?? true),
    weights: args.weights as SearchServersInput['weights'],
  };

  if (!input.query) {
    return createMCPErrorToolResult(
      'Search query is required',
    ) as MCPResult<SearchServersOutput>;
  }

  let servers = await mcpService.getAll();

  // Filter by active status
  if (!input.includeInactive) {
    servers = servers.filter((s: MCPServerEntity) => s.isActive);
  }

  // Apply search based on mode
  if (input.searchMode === 'simple') {
    // Simple substring matching (backward compatibility)
    const query = input.query.toLowerCase();

    servers = servers.filter((server: MCPServerEntity) => {
      const nameMatch = server.name.toLowerCase().includes(query);
      if (input.byNameOnly) return nameMatch;

      const descMatch = server.metadata?.description
        ?.toLowerCase()
        .includes(query);

      return nameMatch || descMatch;
    });

    // Improved relevance sorting: exact > startsWith > contains
    const scoreLevel = (name: string) => {
      const lowerName = name.toLowerCase();
      if (lowerName === query) return 3;
      if (lowerName.startsWith(query)) return 2;
      if (lowerName.includes(query)) return 1;
      return 0;
    };

    servers.sort((a: MCPServerEntity, b: MCPServerEntity) => {
      const scoreA = scoreLevel(a.name);
      const scoreB = scoreLevel(b.name);
      if (scoreA !== scoreB) return scoreB - scoreA;
      return a.name.localeCompare(b.name);
    });

    const result = normalizePagination(servers, input.page!, input.pageSize!);

    // Handle no results with improved guidance
    if (result.totalItems === 0) {
      const allServers = await mcpService.getAll();
      const totalCount = allServers.length;
      const activeCount = allServers.filter((s) => s.isActive).length;

      const suggestions: string[] = [];

      // Suggest switching to BM25 mode
      suggestions.push('Try searchMode: "bm25" for fuzzy matching');

      // Suggest browsing if database is small
      if (totalCount < 20) {
        suggestions.push('Use listServers to browse all servers');
      } else {
        suggestions.push('Try different or shorter keywords');
      }

      // Suggest including inactive if filtered
      if (!input.includeInactive && totalCount > activeCount) {
        suggestions.push('Set includeInactive: true to search all servers');
      }

      return new MCPResponseBuilder({
        ...result,
        query: input.query,
        mode: 'simple',
        databaseStats: { total: totalCount, active: activeCount },
        suggestions,
      })
        .withMessage(
          `No servers found matching "${input.query}".\n` +
            `Database has ${activeCount} active servers (${totalCount} total).`,
        )
        .withSuggestions(suggestions)
        .asSuccess();
    }

    // Build summary with search results
    const summaryLines = [
      `🔍 Search Results for "${input.query}" (simple)`,
      `   Found ${result.totalItems} matching server(s)`,
      `   Page ${result.page}/${result.totalPages}`,
      `   Showing ${result.items.length} server(s)`,
    ];

    // Add top results
    if (result.items.length > 0) {
      summaryLines.push('');
      summaryLines.push('Top Results:');
      result.items.slice(0, 5).forEach((server, idx) => {
        const status = server.isActive ? '🟢' : '🔴';
        const matchType =
          server.name.toLowerCase() === query
            ? '[exact]'
            : server.name.toLowerCase().startsWith(query)
              ? '[starts]'
              : '[contains]';
        summaryLines.push(
          `  ${idx + 1}. ${status} ${server.name} ${matchType}`,
        );
      });
      if (result.items.length > 5) {
        summaryLines.push(`  ... and ${result.items.length - 5} more`);
      }
    }

    const summary = summaryLines.join('\n');

    return createMCPStructuredToolResult(summary, {
      ...result,
      query: input.query,
      mode: 'simple',
    });
  }

  // BM25 mode (default)
  const nameWeight = input.weights?.nameWeight ?? 2.0;
  const descWeight = input.weights?.descWeight ?? 1.0;

  // Build BM25 documents with weighted token duplication
  const docs = servers.map((server) => {
    const nameTokens = defaultTokenizer(server.name);
    const descTokens = defaultTokenizer(server.metadata?.description || '');

    // Duplicate tokens based on weights (round to nearest integer, min 1)
    const weightedNameTokens = nameTokens.flatMap((token) =>
      Array(Math.max(1, Math.round(nameWeight))).fill(token),
    );
    const weightedDescTokens = descTokens.flatMap((token) =>
      Array(Math.max(1, Math.round(descWeight))).fill(token),
    );

    return {
      id: server.id,
      tokens: [...weightedNameTokens, ...weightedDescTokens],
    };
  });

  // Create or retrieve cached BM25 index
  const index = createBM25Index(docs);
  const queryTokens = defaultTokenizer(input.query);
  const scores = index.score(queryTokens);

  // Filter out servers with score of 0 (no match) and sort by BM25 score descending
  servers = servers.filter((server) => {
    const score = scores.get(server.id) || 0;
    return score > 0;
  });

  servers.sort((a, b) => {
    const scoreA = scores.get(a.id) || 0;
    const scoreB = scores.get(b.id) || 0;
    if (scoreA !== scoreB) return scoreB - scoreA;
    return a.name.localeCompare(b.name);
  });

  const result = normalizePagination(servers, input.page!, input.pageSize!);

  // Handle no results with improved guidance
  if (result.totalItems === 0) {
    const allServers = await mcpService.getAll();
    const totalCount = allServers.length;
    const activeCount = allServers.filter((s) => s.isActive).length;

    const suggestions: string[] = [];

    // Suggest switching to simple mode
    suggestions.push('Try searchMode: "simple" for exact matching');

    // Suggest browsing if database is small
    if (totalCount < 20) {
      suggestions.push('Use listServers to browse all servers');
    } else {
      suggestions.push('Try broader or alternative keywords');
    }

    // Suggest including inactive if filtered
    if (!input.includeInactive && totalCount > activeCount) {
      suggestions.push('Set includeInactive: true to search all servers');
    }

    return new MCPResponseBuilder({
      ...result,
      query: input.query,
      mode: 'bm25',
      weights: { name: nameWeight, desc: descWeight },
      databaseStats: { total: totalCount, active: activeCount },
      suggestions,
    })
      .withMessage(
        `No servers found matching "${input.query}" (BM25 search).\n` +
          `Database has ${activeCount} active servers (${totalCount} total).`,
      )
      .withSuggestions(suggestions)
      .asSuccess();
  }

  // Build summary with BM25 search results
  const summaryLines = [
    `🔎 BM25 Results for "${input.query}" (name×${nameWeight}, desc×${descWeight})`,
    `   Found ${result.totalItems} matching server(s)`,
    `   Page ${result.page}/${result.totalPages}`,
    `   Showing ${result.items.length} server(s)`,
  ];

  // Add top results with scores
  if (result.items.length > 0) {
    summaryLines.push('');
    summaryLines.push('Top Results (by relevance):');
    result.items.slice(0, 5).forEach((server, idx) => {
      const status = server.isActive ? '🟢' : '🔴';
      const score = scores.get(server.id) || 0;
      const scoreStr = score > 0 ? ` [score: ${score.toFixed(2)}]` : '';
      summaryLines.push(`  ${idx + 1}. ${status} ${server.name}${scoreStr}`);
    });
    if (result.items.length > 5) {
      summaryLines.push(`  ... and ${result.items.length - 5} more`);
    }
  }

  const summary = summaryLines.join('\n');

  return createMCPStructuredToolResult(summary, {
    ...result,
    query: input.query,
    mode: 'bm25',
  });
}
