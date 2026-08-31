# Publo Vision

## Brand

**Publo.so**

**Title:** Your personal publishing pipeline.  
**Subtitle:** Your content stays local. Publo takes it from there.

## Name origin

**Publo = publish + local**

Local is not only an implementation detail. It is the product philosophy.

> Your content lives with you.  
> Publo helps get it published.

## Core identity

Publo is a **local-first publishing assistant**.

It is not “another SaaS dashboard where you copy/paste content.”  
It should feel like a tool that lives next to the user’s real work and helps publish from there.

## Positioning lines

- Your local publishing assistant.
- Publish from where your content lives.
- Your content stays local. Publo takes it from there.

## Product direction

Publo is not just a scheduler.

Publo is the assistant.  
Scheduling is one capability.

Example interaction style:

```text
You: "Publo, publish this to X and Substack."
Publo: "Done."

You: "Publo, turn this article into posts for next week."
Publo: "I made 2 LinkedIn posts, 3 X posts, 2 Notes and an Instagram post. They're ready for review."

You: "Publo, what am I publishing tomorrow?"
Publo: "LinkedIn at 10:00 and X at 15:00."
```

The current Substack milestone validates this direction: Publo can publish a text
Note directly from a local file through a supervised CLI action. Image Notes are
implemented and remain subject to a supervised live check before worker automation.

## Local-first philosophy (long-term)

User files remain user-owned:

```text
~/Writing/
~/Ideas/
~/Newsletter/
~/Projects/
```

Publo should understand, transform, organize, and publish from local files without forcing users into a proprietary content database.

## AI direction

AI can become a deeper capability without becoming the identity.

Today:

> Publo turns an article into an X post.

Future:

> Publo understands writing style, publication history, cross-links ideas over time, proposes variations for each platform, and prepares a schedule for review.

Still the same identity:

> Publo, my publishing assistant.

## Scope expansion

The current focus is social publishing, but the model should expand naturally to:

- blogs
- newsletters
- Telegram
- RSS
- websites
- future channels

## Strategic reminder

Publo should be built as:

> A local-first publishing assistant that turns files and ideas into scheduled publications.

## Safety guardrails

Publishing is a high-impact action. Publo should support explicit safeguards that reduce accidental external publishes from generic automation.

Current direction:

- scheduling and decision workflows stay easy
- direct CLI publish commands can require an explicit publish password
- AI-assisted workflows can prepare and verify, but publish only with deliberate authorization
- automatic execution is introduced through a supervised real-content pilot before a continuous worker is enabled
- an uncertain publish after a crash is surfaced for human review rather than retried automatically
- unofficial integrations graduate from supervised CLI use to scheduling only after live validation
- persisted behavior evolves through forward migrations; applied migrations are never rewritten
