import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-all select-none disabled:pointer-events-none disabled:opacity-40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-iris-500/60 focus-visible:ring-offset-2 focus-visible:ring-offset-bg",
  {
    variants: {
      variant: {
        primary:
          "bg-iris-500/90 text-white hover:bg-iris-500 shadow-glow-iris-sm hover:shadow-glow-iris active:scale-[0.98]",
        secondary:
          "bg-white/[0.04] text-foreground hover:bg-white/[0.08] border border-white/10",
        outline:
          "border border-white/10 hover:border-iris-500/60 hover:bg-iris-500/10 text-foreground",
        ghost:
          "text-muted-foreground hover:text-foreground hover:bg-white/[0.04]",
        gold: "bg-gold-300/90 text-bg-deep hover:bg-gold-300 shadow-glow-gold font-semibold",
        danger:
          "bg-red-500/20 text-red-200 border border-red-400/30 hover:bg-red-500/30",
      },
      size: {
        sm: "h-8 px-3 text-xs",
        md: "h-9 px-4",
        lg: "h-11 px-6 text-base",
        icon: "h-9 w-9",
      },
    },
    defaultVariants: {
      variant: "secondary",
      size: "md",
    },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, ...props }, ref) => {
    return (
      <button
        ref={ref}
        className={cn(buttonVariants({ variant, size, className }))}
        {...props}
      />
    );
  },
);
Button.displayName = "Button";
