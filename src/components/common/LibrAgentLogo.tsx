import type { SVGProps } from 'react';
import { cn } from '@/lib/utils';

interface LibrAgentLogoProps extends SVGProps<SVGSVGElement> {
  size?: number;
  className?: string;
}

/**
 * LibrAgent brand logo component with full dark and light theme support.
 * Vector-sharp at any resolution, responding directly to Tailwind theme classes.
 */
export function LibrAgentLogo({
  size = 32,
  className,
  ...props
}: LibrAgentLogoProps) {
  return (
    <svg
      viewBox="0 0 256 256"
      width={size}
      height={size}
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={cn(
        'select-none shrink-0 transition-colors duration-200',
        className,
      )}
      {...props}
    >
      {/* Background circle: Dark mode slate-800 (#1e293b), Light mode crisp slate-100 (#f1f5f9) with slate-200 border */}
      <circle
        cx="128"
        cy="128"
        r="118"
        className="fill-[#f1f5f9] stroke-[#e2e8f0] dark:fill-[#1e293b] dark:stroke-transparent"
        strokeWidth="2"
      />
      {/* 'L' shape: Crisp Blue-600 in light mode, original vibrant Blue in dark mode */}
      <path
        d="M88 58h23v118h66v22H88V58Z"
        className="fill-[#2563eb] dark:fill-[#3b82f6]"
      />
      {/* Connecting lines between constellation nodes */}
      <line
        x1="158"
        y1="88"
        x2="177"
        y2="118"
        strokeWidth="4.5"
        strokeLinecap="round"
        className="stroke-[#9333ea] dark:stroke-[#9333ea]"
      />
      <line
        x1="177"
        y1="118"
        x2="168"
        y2="157"
        strokeWidth="4.5"
        strokeLinecap="round"
        className="stroke-[#9333ea] dark:stroke-[#9333ea]"
      />
      {/* Constellation dots */}
      <circle
        cx="158"
        cy="88"
        r="6.5"
        className="fill-[#9333ea] dark:fill-[#9333ea]"
      />
      <circle
        cx="177"
        cy="118"
        r="6.5"
        className="fill-[#9333ea] dark:fill-[#9333ea]"
      />
      <circle
        cx="168"
        cy="157"
        r="6.5"
        className="fill-[#9333ea] dark:fill-[#9333ea]"
      />
    </svg>
  );
}
