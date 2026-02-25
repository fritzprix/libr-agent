import React from 'react';
import { Card, CardHeader, CardContent } from '@/components/ui';
import { Skeleton } from '@/components/ui/skeleton';

export function ServerCardSkeleton() {
  return (
    <Card className="relative overflow-hidden">
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <div className="flex gap-3 items-start flex-1">
          {/* Logo Skeleton */}
          <Skeleton className="w-8 h-8 rounded-md flex-shrink-0 mt-0.5" />

          <div className="flex-1 space-y-2">
            {/* Title Skeleton */}
            <Skeleton className="h-5 w-3/4" />

            {/* Description Skeleton */}
            <Skeleton className="h-4 w-full" />

            {/* Transport Info Skeleton */}
            <Skeleton className="h-3 w-1/2" />

            {/* Status Skeleton */}
            <Skeleton className="h-3 w-1/3 mt-1" />
          </div>
        </div>

        <div className="flex items-center gap-2">
          <div className="flex flex-col items-end gap-1">
            {/* Switch Label Skeleton */}
            <Skeleton className="h-3 w-8" />
            {/* Switch Skeleton */}
            <Skeleton className="h-5 w-9 rounded-full" />
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <div className="flex gap-2">
          {/* Button Skeletons */}
          <Skeleton className="h-8 w-16" />
          <Skeleton className="h-8 w-16" />
        </div>
      </CardContent>
    </Card>
  );
}
