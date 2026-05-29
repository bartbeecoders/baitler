# Baitler frontend

React 19 + TypeScript + Vite 8, styled with Tailwind CSS v4. The app shell and base
portal for Baitler (see the repo-root `README.md` and `plan.md` for the bigger picture).

## Scripts

```bash
npm run dev        # Vite dev server (http://localhost:5173)
npm run build      # tsc -b && vite build  ->  dist/
npm run typecheck  # tsc -b
npm run lint       # eslint .
npm test           # vitest run
```

## Notes

- **Stack:** React Router 7 (routing), TanStack Query 5 (data fetching), Zustand
  (theme/UI state), lucide-react (icons), Vitest + Testing Library (tests).
- **Imports:** `@/…` resolves to `src/…`.
- **Env:** Vite reads `VITE_*` from the **repo-root** `.env` (`envDir` in `vite.config.ts`).
  `VITE_API_BASE_URL` points at the backend (defaults to `http://localhost:8080` in dev;
  required in production builds).
- **Theme:** light/dark via a `.dark` class on `<html>`; a pre-paint inline script in
  `index.html` avoids a flash of the wrong theme.
- **Auth:** intentionally not wired yet — the shell is auth-ready (credentialed API
  client, `UserMenu` slot); OAuth lands in the final phase.
