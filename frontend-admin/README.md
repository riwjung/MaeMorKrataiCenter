# frontend-admin

Admin dashboard (frontend side):

- Site overview
- Reader scoring page for the 3+1 card data ([`docs/context/11-phase-2-plan.md`](../docs/context/11-phase-2-plan.md), item 9)
- Competitor clip research (collect and review ideas from competitor videos)
- Control panel for the content automation program

Talks to the admin Worker ([`backend/admin`](../backend/admin)) only through the API in [`docs/api/admin/`](../docs/api/admin). Login uses the existing admin auth (access JWT + HttpOnly refresh cookie, see SPEC-001).

Status: empty. Reference for the previous version: `MaeMorKratai/services/social-center` (Next.js + FastAPI + SQLite) in the old repo.
