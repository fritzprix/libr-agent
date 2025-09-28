import { useCallback, useEffect, useState } from 'react';
import { getLogger } from '@/lib/logger';
import { useSystemPrompt } from '@/context/SystemPromptContext';

const logger = getLogger('TimeLocationSystemPrompt');

interface LocationInfo {
  city?: string;
  country?: string;
  region?: string;
  timezone: string;
  // coordinates may be omitted when unavailable or intentionally not shared
  coordinates?: {
    lat: number;
    lon: number;
  };
}

/**
 * System prompt component that provides current time and location information
 * in natural language format that's easily understandable by AI and users.
 *
 * This component automatically registers a system prompt extension that:
 * - Provides current date and time in readable format
 * - Shows location as city/country when available
 * - Includes timezone information
 * - Falls back gracefully when location is unavailable
 */
export function TimeLocationSystemPrompt() {
  const { register, unregister } = useSystemPrompt();
  const [locationInfo, setLocationInfo] = useState<LocationInfo | null>(null);
  const [isLocationLoading, setIsLocationLoading] = useState(true);

  // 역지오코딩을 통해 자연어 위치 정보 가져오기
  const getLocationName = async (
    lat: number,
    lon: number,
  ): Promise<LocationInfo> => {
    try {
      const response = await fetch(
        `https://nominatim.openstreetmap.org/reverse?lat=${lat}&lon=${lon}&format=json&accept-language=en`,
      );
      const data = await response.json();

      const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone;

      return {
        city: data.address?.city || data.address?.town || data.address?.village,
        country: data.address?.country,
        region: data.address?.state || data.address?.province,
        timezone,
        coordinates: { lat, lon },
      };
    } catch (error) {
      logger.warn('Failed to get location name from coordinates', error);
      const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone;
      return {
        timezone,
        // keep coordinates but allow callers to treat them as absent if needed
        coordinates: { lat, lon },
      };
    }
  };

  // 위치 정보 가져오기
  useEffect(() => {
    if ('geolocation' in navigator) {
      navigator.geolocation.getCurrentPosition(
        async (pos) => {
          try {
            const locationInfo = await getLocationName(
              pos.coords.latitude,
              pos.coords.longitude,
            );
            setLocationInfo(locationInfo);
            logger.debug('Location info retrieved', locationInfo);
          } catch (error) {
            logger.error('Failed to process location data', error);
          } finally {
            setIsLocationLoading(false);
          }
        },
        (error) => {
          logger.warn('Failed to get geolocation', error);
          const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone;
          // Do not set 0,0 as a real coordinate - omit coordinates so UI/LLM
          // consumers don't accidentally treat (0,0) as a valid location.
          setLocationInfo({
            timezone,
          });
          setIsLocationLoading(false);
        },
        {
          timeout: 10000,
          maximumAge: 3600000, // 1시간 캐시
          enableHighAccuracy: false,
        },
      );
    } else {
      logger.warn('Geolocation not supported');
      const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone;
      // Omit coordinates when geolocation isn't available
      setLocationInfo({
        timezone,
      });
      setIsLocationLoading(false);
    }
  }, []);

  const buildPrompt = useCallback(async () => {
    const now = new Date();

    // 자연어 형태의 날짜/시간
    const dateOptions: Intl.DateTimeFormatOptions = {
      weekday: 'long',
      year: 'numeric',
      month: 'long',
      day: 'numeric',
    };

    const timeOptions: Intl.DateTimeFormatOptions = {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      timeZoneName: 'short',
    };

    const currentDate = now.toLocaleDateString('en-US', dateOptions);
    const currentTime = now.toLocaleTimeString('en-US', timeOptions);

    // 위치 정보 구성
    let locationText = 'Location information not available';

    const isValidCoordinates = (c?: { lat: number; lon: number }) => {
      if (!c) return false;
      // Treat near-(0,0) as invalid/fallback. Use a tiny epsilon to avoid FP issues.
      const eps = 1e-6;
      return Math.abs(c.lat) > eps || Math.abs(c.lon) > eps;
    };

    if (isLocationLoading) {
      locationText = 'Loading location information...';
    } else if (locationInfo) {
      if (locationInfo.city && locationInfo.country) {
        locationText = `${locationInfo.city}, ${locationInfo.country}`;
        if (locationInfo.region && locationInfo.region !== locationInfo.city) {
          locationText = `${locationInfo.city}, ${locationInfo.region}, ${locationInfo.country}`;
        }
      } else if (locationInfo.country) {
        locationText = locationInfo.country;
      } else if (isValidCoordinates(locationInfo.coordinates)) {
        const coords = locationInfo.coordinates!;
        locationText = `Coordinates: ${coords.lat.toFixed(4)}, ${coords.lon.toFixed(4)}`;
      } else {
        // Keep the fallback text - coordinates are not reliable
        locationText = 'Location information not available';
      }

      locationText += ` (${locationInfo.timezone})`;
    }

    return `# Current Context Information

## Date and Time
- **Current Date**: ${currentDate}
- **Current Time**: ${currentTime}

## Location
- **User Location**: ${locationText}

*This information is automatically updated and provided to help you understand the user's current context.*`;
  }, [locationInfo, isLocationLoading]);

  useEffect(() => {
    const id = register('time-location', buildPrompt, 1);

    logger.debug('Registered time/location system prompt', { promptId: id });

    return () => {
      unregister(id);
      logger.debug('Unregistered time/location system prompt', {
        promptId: id,
      });
    };
  }, [buildPrompt, register, unregister]);

  return null;
}
