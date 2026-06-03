/**
 * Placeholder shown in the main content area when no object is selected. The
 * object lists now live in the sidebar "Objects" group, so each feature route
 * renders only the editor for the selected item (or this hint).
 */
export function EmptyDetail({ noun }: { noun: string }) {
  return (
    <div className="grid h-[calc(100svh-12rem)] place-items-center text-center text-sm text-muted-foreground">
      <p>
        Select a {noun} from the <span className="font-medium text-foreground">Objects</span> menu in
        the sidebar, or create a new one with the <span className="font-medium text-foreground">+</span> button.
      </p>
    </div>
  );
}
