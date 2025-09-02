# Frontend Migration – SvelteKit → React SPA (TypeScript)

## 1 — Goals
• Replace SvelteKit codebase with React 18 SPA.  
• Full TypeScript coverage.  
• Preserve all existing features (boards, threads, replies, image upload, TanStack Query, Tailwind CSS + DaisyUI).  
• Keep API contract `/api/v1/**` unchanged.  
• Zero downtime: old frontend served until new build is ready.

## 2 — Non-Goals
• Server-side rendering (SSR) – SPA only for now.  
• Design/UI overhaul.  
• Feature creep (auth, WebSockets) – migrate first, then iterate.

## 3 — Tech Stack
| Concern           | Choice                               | Notes                              |
|-------------------|--------------------------------------|------------------------------------|
| Bundler           | Vite 5 (+ `@vitejs/plugin-react`)    | Fast dev & easy Tailwind setup     |
| State / Caching   | TanStack Query v5                    | Matches current Svelte usage       |
| Router            | React Router v6.22                   | File-like routes mirror existing   |
| Forms / Validation| React Hook Form                      | Minimal boilerplate                |
| Styling           | Tailwind CSS + DaisyUI              | Keep existing classes              |
| Testing           | Vitest + React Testing Library       | Unit & integration                 |
| Lint/Format       | ESLint (+ `eslint-plugin-react`) / Prettier | Same config extended             |

## 4 — Directory Layout
```
rib-web/
  src/
    api/                 // fetch helpers
    components/          // shared React components
    pages/               // top-level route components
      Boards.tsx         // "/"
      BoardCatalog.tsx   // "/b/:slug"
      ThreadView.tsx     // "/thread/:id"
    hooks/               // custom React hooks
    App.tsx              // root
    main.tsx             // Vite entry
  index.html
  tailwind.config.js
  postcss.config.js
```

## 5 — Migration Plan
1. Bootstrap fresh React + TS project inside `rib-web` (`npm create vite@latest rib-web -- --template react-ts`).  
2. Port **API layer** (`fetchJson`, `postJson`, `uploadImage`, `queryClient`) as pure TS modules.  
3. Re-implement **layout shell** (`Navbar`, `LoginModal`) using DaisyUI.  
4. Migrate **routes** one by one:  
   a. `/` Boards list and create form  
   b. `/b/:slug` Thread catalog and new-thread form  
   c. `/thread/:id` Thread view and reply form  
   Reuse existing Svelte logic; translate to React hooks + components.  
5. Configure **React Router** routes matching above pages.  
6. Integrate **TanStack Query** provider at root.  
7. Bring over **Tailwind** config; purge paths to `src/**/*.{ts,tsx}`.  
8. Add ESLint React rules; update `pre-commit` hooks.  
9. Parity testing: run side-by-side with Svelte version, compare API calls & UI behaviour.  
10. Cut-over: update Nginx/Docker image to serve `rib-web` React build instead of Svelte output.  
11. Remove stale Svelte code in a follow-up cleanup PR.

## 6 — Risks & Mitigations
| Risk | Mitigation |
|------|------------|
| Feature regressions | Manual parity tests + Cypress E2E (future) |
| Bundle size growth  | Analyze with `rollup-plugin-visualizer`, lazy-load routes |
| Learning curve      | Provide internal doc & code owners review |

## 7 — Estimated Timeline
| Task | Owner | ETA |
|------|-------|-----|
| Project scaffolding & tooling | FE team | Day 1 |
| API & hooks port | FE team | Day 2-3 |
| Page migrations | FE team | Day 4-7 |
| Testing & polish | FE + QA | Day 8-9 |
| Cut-over & cleanup | FE + DevOps | Day 10 |

## 8 — Open Questions
1. Keep `.svelte-kit` generated types in repo until cleanup?  
2. Any stakeholders relying on SSR features of SvelteKit? (none known)  
3. Plan to re-enable SSR later with Remix/Next or stay SPA?

## 9 — Implementation Checklist
> Tick items off as they are merged into `main`.
