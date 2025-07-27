import React from "react";
import { Badge } from "./badge";
import { cn } from "@/lib/utils";

interface BadgeLegacyProps extends React.HTMLAttributes<HTMLSpanElement> {
  variant?: "default" | "secondary" | "destructive" | "outline" | "warning" | "active" | "error" | "success";
  children: React.ReactNode;
}

export default function BadgeLegacy({
  variant = "default",
  className = "",
  children,
  ...props
}: BadgeLegacyProps) {
  // Map legacy variants to Shadcn variants and custom styles
  const getVariantStyles = (v: string) => {
    switch (v) {
      case "warning":
        return {
          shadcnVariant: "outline" as const,
          customClass: "border-yellow-400 text-yellow-400 bg-yellow-900/20"
        };
      case "active":
        return {
          shadcnVariant: "default" as const,
          customClass: "bg-green-900/20 text-green-400 border-green-400"
        };
      case "error":
        return {
          shadcnVariant: "destructive" as const,
          customClass: ""
        };
      case "success":
        return {
          shadcnVariant: "outline" as const,
          customClass: "border-green-400 text-green-400 bg-green-900/20"
        };
      case "default":
      case "secondary":
      case "destructive":
      case "outline":
        return {
          shadcnVariant: v as "default" | "secondary" | "destructive" | "outline",
          customClass: ""
        };
      default:
        return {
          shadcnVariant: "default" as const,
          customClass: ""
        };
    }
  };

  const { shadcnVariant, customClass } = getVariantStyles(variant);

  return (
    <Badge
      variant={shadcnVariant}
      className={cn(customClass, className)}
      {...props}
    >
      {children}
    </Badge>
  );
}
