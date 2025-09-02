
### 0. Project Scaffold & Tooling
+ [x] Bootstrap Vite + React TS project (`npm create vite@latest rib-react -- --template react-ts`)
+ [x] Tailwind CSS & DaisyUI integrated
+ [x] ESLint (+ `eslint-plugin-react`) & Prettier configs ported
+ [ ] Husky / pre-commit hook updated (todo)
+ [x] Dev server (`npm run dev`) functional

### 1. API Layer
- [x] Port `fetchJson`, `postJson`, `uploadImage`
- [x] Setup TanStack Query `queryClient`
- [x] Unit tests for helpers (basic error path)

### 2. Layout Shell
- [x] `Navbar` component with links
- [ ] `LoginModal` stub (JWT auth future)

### 3. Routing & Pages
- [x] React Router provider in `App.tsx`
- [x] `/` Boards list + create form
- [x] `/b/:slug` Catalog + new-thread form
- [x] `/thread/:id` Thread view + reply form

### 4. State / Data Hooks
+ [x] `useBoards`, `useThreads`, `useReplies` hooks
+ [ ] React Hook Form validation on create/reply forms (currently manual state)

### 5. Styling
- [x] Port existing Tailwind classes (initial pass)
- [ ] Replace Svelte components with DaisyUI equivalents (basic usage only)

### 6. Testing
- [x] Vitest + React Testing Library config
- [ ] Component & hook unit tests (only api helper test added)
- [ ] Manual parity checks vs Svelte build

### 7. Build & CI
- [ ] GitHub Actions job for lint/test/build
- [ ] Docker multi-stage build producing static files
- [ ] Nginx container serves React bundle

### 8. Cut-over & Cleanup
- [ ] Update deployment to serve new bundle
- [x] Remove SvelteKit code after verification (source deleted; docker & config pending)
- [ ] Update documentation & runbooks

*Document owner: FE team • Last updated: 2025-09-01*
