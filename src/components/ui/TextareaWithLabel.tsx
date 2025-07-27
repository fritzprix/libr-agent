import React from "react";
import { Textarea } from "./textarea";

interface TextareaWithLabelProps extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?: string;
  error?: string;
  containerClassName?: string;
}

export default function TextareaWithLabel({
  label,
  error,
  className = "",
  containerClassName = "",
  ...props
}: TextareaWithLabelProps) {
  return (
    <div className={containerClassName}>
      {label && (
        <label className="block text-gray-400 mb-2 font-medium">{label}</label>
      )}
      <Textarea
        className={`${error ? "border-red-400" : ""} ${className}`}
        {...props}
      />
      {error && <p className="text-red-400 text-xs mt-1">{error}</p>}
    </div>
  );
}
