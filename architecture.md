# Outbox: Personal Content Publishing Pipeline

## Project idea

Build a small local-first personal publishing tool that helps me prepare, schedule, and publish my content across the platforms I use for my newsletter and social presence.

This is a personal tool for one user, not a SaaS product or a generalized social media management platform. But it should be implemented in general format, so anyone can clone the repo, set some environment variables and be able to run it on their local computer and work with it.

The main problem it should solve is the friction of publishing the same idea across multiple platforms. I currently have to open several platforms, adapt the content for each one, copy/paste, upload images, and remember when to post. Because of that friction, I often publish less consistently than I intend to.

The tool should make the workflow closer to:

> Create something once → prepare platform-specific versions → schedule it → let the system publish it.

The content itself should remain under my control, preferably as files on my computer. The publishing system should be a layer around those files rather than a replacement for them.

## Initial platforms

The primary platforms are (in order of importance):

* LinkedIn
* Substack
* X
* Instagram

LinkedIn, X, and Instagram should use their official APIs where practical.

Substack is different. Its official API does not currently provide the same straightforward publishing capability as the other platforms, especially for Notes. It is acceptable to use a reliable community project or integration for Substack publishing if appropriate. This is a personal tool, so depending on a maintained community solution is acceptable, as long as the dependency is clearly isolated from the rest of the application.

The system should not assume that every platform has identical capabilities.

## Content model

A single source piece of content may produce multiple platform-specific versions.

For example:

```text
One idea / article
    ├── LinkedIn version
    ├── Substack version
    ├── X post or thread
    └── Instagram caption / media
```

The platform versions can be manually created or generated with AI.

AI generation is useful for transforming content rather than simply copying it everywhere. For example:

* shortening a thought for X (because of a character limit)
* turning a longer thought into an X thread
* adapting an article into a LinkedIn post
* creating a concise Substack Note
* creating an Instagram-friendly version
* suggesting whether a piece is suitable for a particular platform

AI should assist the content workflow, but it should not silently publish generated content without an explicit publishing decision unless that behavior is deliberately enabled later.

## Two important publishing modes

### Scheduled publishing

I want to be able to prepare content in batches.

For example, I may prepare several pieces for the coming week. The system should know what should be published, on which platform, and approximately when.

The scheduler should eventually be capable of publishing the queue automatically.

I may initially run the scheduler manually rather than keeping a permanent background service running.

### Immediate publishing

There should also be a very low-friction way to publish a spontaneous thought.

For example:

> I think hardware engineers underestimate how difficult maintenance is after shipping a product.

I may want to immediately publish this to X and/or a Substack Note without creating a full weekly content item.

The system should make this kind of "publish now" workflow easy.

## Content storage

Prefer a file-based model.

Content should be understandable and editable outside the application. Markdown and ordinary media files are preferred over storing everything in a proprietary database format.

A content item might conceptually contain:

```text
content/
  some-topic/
    source.md
    linkedin.md
    substack.md
    x.md
    instagram.md
    image.png
```

The exact file structure is an implementation decision. Do not over-engineer the format prematurely.

Metadata such as publication time, target platform, status, and source content may be stored alongside the content.

The system should make it easy to understand:

* what is scheduled
* what has been published
* what failed
* what is still waiting for review
* which platform version belongs to which source

## Publishing behavior

Publishing should be treated as a separate concern from content generation.

Conceptually:

content
   ↓
platform-specific content
   ↓
scheduled publication
   ↓
platform adapter
   ↓
published / failed

Each platform should have its own adapter or integration boundary.

The rest of the application should not need to know the details of LinkedIn OAuth, X APIs, Instagram APIs, or a Substack integration.

For example, the application should conceptually be able to say:

publish(post, platform)

without the rest of the system caring whether that ultimately uses the LinkedIn API, Instagram API, X API, or a community Substack integration.

## Authentication

Modern platforms generally use an application/project plus OAuth authorization rather than giving an individual account a permanent "personal API key."

This tool should therefore treat credentials roughly as:

```text
application credentials
        +
user authorization
        +
access/refresh tokens
```

The application is only for my own account, but it should still use the platform's intended authentication mechanisms.

Credentials and tokens must be treated as secrets and must never be stored inside content files or committed to source control.

## Reliability

This tool is allowed to be simple, but publishing is one place where mistakes are costly.

A publication should have a clear state such as:

```text
draft
ready
scheduled
publishing
published
failed
```

The system should avoid accidentally publishing the same item twice because of a retry or temporary failure.

Failures should be visible and understandable.

A failed Instagram publication should not prevent an unrelated LinkedIn or X publication from being handled.

## Human control

The system should optimize for low friction, not maximum automation at any cost.

The ideal long-term workflow is that I can prepare content, review the generated platform-specific versions, schedule them, and then let the system handle the repetitive work.

However, it should remain possible to intervene manually.

For example, I may want to:

* review generated content before scheduling
* disable a particular platform for one post
* edit a platform-specific version
* publish something immediately
* retry a failed publication manually
* manually handle a platform when an integration is unavailable

Do not design the system around the assumption that every platform will always behave perfectly.

## Local-first philosophy

This is a personal tool running on my own computer.

Prefer:

* local files
* simple configuration
* minimal infrastructure
* minimal external dependencies
* easy backups
* easy inspection and debugging

Do not introduce a server, hosted database, cloud infrastructure, or multi-user architecture unless there is a concrete reason to do so. If necessary, a local SQLite database is preferred.

A CLI is perfectly acceptable. A local graphical dashboard may be added later if it actually makes the workflow easier.

## Important product principle

The system should not become a social media management platform.

The goal is not analytics, team management, audience management, approval workflows, collaboration, billing, or supporting hundreds of users.

The goal is much simpler:

> Help one person consistently get good ideas from local files into the world with as little repetitive work as possible.

Keep the architecture flexible enough to support future improvements, but prefer the smallest useful implementation whenever there is a choice.

## Possible future capabilities

These are possibilities, not requirements:

* AI-assisted content transformation
* automatic detection of X character limits
* automatic X thread generation
* image generation or image preparation
* content previews
* a local publishing calendar
* publish-now commands
* scheduled batch publishing
* notifications when something fails
* history of published content
* retry support
* additional platforms
* local dashboard
* Git-friendly content storage

Do not implement these simply because they are listed here. They are context for the direction of the project.

## Definition of success

The project is successful when publishing stops feeling like a separate administrative task.

I should be able to create good content, put it into my normal file-based workflow, and have the system take care of the repetitive platform-specific work and scheduled publishing.

The most important metric is not how sophisticated the software is.

It is:

> How much less effort does it take me to consistently publish?

