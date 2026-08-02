import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center rounded-md border px-2 py-0.5 text-xs font-medium",
  {
    variants: {
      variant: {
        vendored:
          "border-transparent bg-sky-100 text-sky-800 dark:bg-sky-950 dark:text-sky-300",
        authored:
          "border-transparent bg-emerald-100 text-emerald-800 dark:bg-emerald-950 dark:text-emerald-300",
        muted:
          "border-transparent bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300",
      },
    },
    defaultVariants: { variant: "muted" },
  },
);

export type BadgeProps = React.ComponentProps<"span"> & VariantProps<typeof badgeVariants>;

export function Badge({ className, variant, ...props }: BadgeProps) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />;
}
