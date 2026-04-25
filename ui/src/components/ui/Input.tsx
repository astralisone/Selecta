import * as React from "react";
import { cn } from "@/lib/utils";

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {}

export const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, ...props }, ref) => {
    return (
      <input
        ref={ref}
        className={cn(
          "flex h-9 w-full rounded-md bg-white/[0.03] border border-white/10 px-3 py-1 text-sm text-foreground placeholder:text-muted-foreground/70",
          "transition-all focus-visible:outline-none focus-visible:border-iris-500/70 focus-visible:bg-white/[0.06] focus-visible:ring-0",
          "disabled:cursor-not-allowed disabled:opacity-50",
          className,
        )}
        {...props}
      />
    );
  },
);
Input.displayName = "Input";
