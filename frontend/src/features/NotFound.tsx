import { Link } from 'react-router-dom';

export function NotFound() {
  return (
    <div className="mx-auto flex max-w-md flex-col items-center gap-4 py-20 text-center">
      <p className="text-5xl font-bold text-primary-strong">404</p>
      <h1 className="text-xl font-semibold">Page not found</h1>
      <p className="text-muted-foreground">That page doesn&apos;t exist (yet).</p>
      <Link
        to="/"
        className="inline-flex h-10 items-center rounded-md border border-border px-4 text-sm font-medium transition-colors hover:bg-muted"
      >
        Back to dashboard
      </Link>
    </div>
  );
}
