/** The Baitler wordmark: a honeycomb hexagon (bee motif) + name. */
export function Brand({ showName = true }: { showName?: boolean }) {
  return (
    <div className="flex items-center gap-2.5">
      <svg viewBox="0 0 24 24" className="h-8 w-8" aria-hidden="true">
        <path d="M12 1.5l9.09 5.25v10.5L12 22.5l-9.09-5.25V6.75z" className="fill-primary" />
        <text
          x="12"
          y="16"
          textAnchor="middle"
          className="fill-primary-foreground"
          fontSize="11"
          fontWeight="700"
          fontFamily="var(--font-sans)"
        >
          B
        </text>
      </svg>
      {showName && (
        <span className="text-lg font-semibold tracking-tight text-foreground">Baitler</span>
      )}
    </div>
  );
}
