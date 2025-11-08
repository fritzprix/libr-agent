# MCP Server Backend를 MCP Tool로 제공

- listServers(pagination: Pagination)
- searchServer(query: string)
- createServer(entity: MCPServerEntity)
- connectServer(id: string) => dynamically server is connected and available to the ai agent
