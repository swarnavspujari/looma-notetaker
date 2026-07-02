import type { ButtonHTMLAttributes, HTMLAttributes, ReactNode } from "react";

/* Shared building blocks for the Looma design language (see index.css tokens).
   Keep these tiny: components compose Tailwind utilities on top. */

type Variant = "primary" | "soft" | "outline" | "ghost" | "dark" | "record";
type Size = "xs" | "sm" | "md";

const VARIANT: Record<Variant, string> = {
  primary: "bg-coral text-white hover:brightness-105 active:brightness-95",
  soft: "border border-line bg-peach-2 text-clay hover:bg-peach",
  outline: "border border-line bg-surface text-ink-2 hover:bg-peach-2 hover:text-ink",
  ghost: "text-ink-2 hover:bg-peach-2 hover:text-ink",
  dark: "bg-ink text-white hover:brightness-125",
  record: "bg-rec text-white hover:brightness-105",
};

const SIZE: Record<Size, string> = {
  xs: "gap-1 rounded-md px-1.5 py-0.5 text-[11px]",
  sm: "gap-1.5 rounded-lg px-2.5 py-1 text-xs",
  md: "gap-2 rounded-[10px] px-3.5 py-1.5 text-[13px]",
};

interface BtnProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
}

export function Btn({ variant = "outline", size = "sm", className = "", ...rest }: BtnProps) {
  return (
    <button
      className={`inline-flex cursor-pointer items-center justify-center font-semibold transition-[filter,background-color,color] duration-100 disabled:pointer-events-none disabled:opacity-50 ${VARIANT[variant]} ${SIZE[size]} ${className}`}
      {...rest}
    />
  );
}

export function SectionLabel({ className = "", ...rest }: HTMLAttributes<HTMLParagraphElement>) {
  return (
    <p
      className={`text-[10.5px] font-semibold uppercase leading-none tracking-[0.09em] text-mute ${className}`}
      {...rest}
    />
  );
}

/** Overlay + warm card. Used by every modal so chrome stays consistent. */
export function ModalShell({
  children,
  className = "",
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-ink/40 p-6">
      <div
        className={`max-h-full overflow-y-auto rounded-2xl border border-line bg-surface shadow-warm ${className}`}
      >
        {children}
      </div>
    </div>
  );
}

/* Speaker identity: the mic channel ("you") is always coral; other speakers
   rotate through the design-system speaker palette. */
const SPEAKER_ROTATION = [
  "var(--color-spk-teal)",
  "var(--color-spk-violet)",
  "var(--color-spk-blue)",
  "var(--color-spk-amber)",
  "var(--color-spk-pink)",
  "var(--color-spk-green)",
];

export function speakerColor(speakerKey: string, index: number): string {
  if (speakerKey === "mic") return "var(--color-coral)";
  return SPEAKER_ROTATION[
    ((index % SPEAKER_ROTATION.length) + SPEAKER_ROTATION.length) % SPEAKER_ROTATION.length
  ];
}

export function speakerInitials(label: string): string {
  const words = label.trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return "?";
  if (words.length === 1) return words[0].slice(0, 2).toUpperCase();
  return (words[0][0] + words[words.length - 1][0]).toUpperCase();
}
