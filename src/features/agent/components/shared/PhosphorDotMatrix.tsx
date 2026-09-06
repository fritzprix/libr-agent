import React, { useEffect, useRef } from 'react';

interface PhosphorDotMatrixProps {
  /** Size variant */
  size?: 'sm' | 'md' | 'lg';
  /** Optional custom className */
  className?: string;
}

const COLS = 3;
const ROWS = 4;

const SIZE_MAP = {
  sm: {
    width: 13,
    height: 17,
    dotRadius: 1.1,
    gapX: 4.2,
    gapY: 4.2,
    padX: 2.3,
    padY: 2.2,
  },
  md: {
    width: 16,
    height: 21,
    dotRadius: 1.4,
    gapX: 5.2,
    gapY: 5.2,
    padX: 2.8,
    padY: 2.7,
  },
  lg: {
    width: 20,
    height: 26,
    dotRadius: 1.8,
    gapX: 6.5,
    gapY: 6.5,
    padX: 3.5,
    padY: 3.3,
  },
};

// Coordinate list in perimeter snake order for 3x4 grid
// (0,0)->(1,0)->(2,0)->(2,1)->(2,2)->(2,3)->(1,3)->(0,3)->(0,2)->(0,1)
const PERIMETER_INDICES: [number, number][] = [
  [0, 0],
  [1, 0],
  [2, 0],
  [2, 1],
  [2, 2],
  [2, 3],
  [1, 3],
  [0, 3],
  [0, 2],
  [0, 1],
];
const INNER_INDICES: [number, number][] = [
  [1, 1],
  [1, 2],
];

/**
 * PhosphorDotMatrix - Canvas-based retro-futuristic CRT dot matrix loader.
 * Renders an animated 3x4 grid of glowing phosphor dots with smooth trails and bloom.
 */
export const PhosphorDotMatrix: React.FC<PhosphorDotMatrixProps> = ({
  size = 'md',
  className = '',
}) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d', { alpha: true });
    if (!ctx) return;

    const dimensions = SIZE_MAP[size];
    const dpr = window.devicePixelRatio || 1;

    // HiDPI scale
    canvas.width = dimensions.width * dpr;
    canvas.height = dimensions.height * dpr;
    canvas.style.width = `${dimensions.width}px`;
    canvas.style.height = `${dimensions.height}px`;

    let animationFrameId: number | null = null;
    let isDestroyed = false;

    const render = (timestamp: number) => {
      if (isDestroyed || document.hidden) {
        animationFrameId = null;
        return;
      }

      ctx.save();
      ctx.scale(dpr, dpr);
      ctx.clearRect(0, 0, dimensions.width, dimensions.height);

      // Dynamically read computed color every frame to immediately react to theme changes
      const currentColor =
        window.getComputedStyle(canvas).color || 'rgb(120, 120, 140)';

      const time = timestamp * 0.004; // smooth animation speed
      const leadPos = (time * 1.6) % PERIMETER_INDICES.length;

      // Draw 3x4 grid dots
      for (let r = 0; r < ROWS; r++) {
        for (let c = 0; c < COLS; c++) {
          const x = dimensions.padX + c * dimensions.gapX;
          const y = dimensions.padY + r * dimensions.gapY;

          // Determine dot intensity based on perimeter snake distance or inner pulse
          let intensity = 0.12; // Base dim level so the matrix grid remains subtly visible

          const perimIdx = PERIMETER_INDICES.findIndex(
            ([pc, pr]) => pc === c && pr === r,
          );
          if (perimIdx !== -1) {
            const dist =
              (leadPos - perimIdx + PERIMETER_INDICES.length) %
              PERIMETER_INDICES.length;
            if (dist < 4.5) {
              const trailWeight = Math.max(0, 1 - dist / 4.5);
              intensity = Math.max(
                intensity,
                0.12 + 0.88 * Math.pow(trailWeight, 1.8),
              );
            }
          } else {
            const innerIdx = INNER_INDICES.findIndex(
              ([ic, ir]) => ic === c && ir === r,
            );
            const innerPulse =
              Math.sin(time * 1.5 + (innerIdx === 0 ? 0 : Math.PI)) * 0.5 + 0.5;
            intensity = Math.max(
              intensity,
              0.15 + 0.45 * Math.pow(innerPulse, 2),
            );
          }

          ctx.beginPath();
          ctx.arc(x, y, dimensions.dotRadius, 0, Math.PI * 2);

          if (intensity > 0.6) {
            ctx.shadowBlur = 3 * (intensity - 0.5);
            ctx.shadowColor = currentColor;
          } else {
            ctx.shadowBlur = 0;
          }

          ctx.fillStyle = currentColor;
          ctx.globalAlpha = intensity;
          ctx.fill();
        }
      }

      ctx.restore();
      animationFrameId = requestAnimationFrame(render);
    };

    const startLoop = () => {
      if (!animationFrameId && !isDestroyed && !document.hidden) {
        animationFrameId = requestAnimationFrame(render);
      }
    };

    const stopLoop = () => {
      if (animationFrameId !== null) {
        cancelAnimationFrame(animationFrameId);
        animationFrameId = null;
      }
    };

    const handleVisibilityChange = () => {
      if (document.hidden) {
        stopLoop();
      } else {
        startLoop();
      }
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);
    startLoop();

    return () => {
      isDestroyed = true;
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      stopLoop();
    };
  }, [size]);

  return (
    <canvas
      ref={canvasRef}
      className={`inline-block shrink-0 align-middle ${className}`}
      aria-hidden="true"
    />
  );
};
