import { useBuiltInTool } from './index';

/**
 * Hook to access structured state from a specific built-in service.
 * The service must implement getServiceContext() that returns ServiceContext<T>.
 *
 * @param serviceId - The ID of the service to get context from
 * @returns The structured state from the service's ServiceContext
 */
export function useServiceContext<T = unknown>(
  serviceId: string,
): T | undefined {
  const { serviceContexts } = useBuiltInTool();
  return serviceContexts[serviceId] as T | undefined;
}
