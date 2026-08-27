# Agent Guidelines

## Frontend

- Use the existing shadcn-ui components and their documented variants.
- Do not introduce bare custom interactive controls when a project component exists.
- Do not create an ad-hoc visual system alongside the established component library.
- Keep JSX and TypeScript expanded across readable lines; optimize for human review, not compact output.
- Extract repeated lifecycle UI into named components instead of duplicating markup between pages.

## Temporary Publishing Pilot Guardrail

Through September 3, 2026, warn the user and obtain confirmation before changing either:

- the core worker/publishing algorithm
- the database schema or migrations

UI/UX changes that do not alter those areas may proceed normally.
