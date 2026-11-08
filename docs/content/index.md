---
seo:
  title: Collectanea — Documentation
  description: Official documentation for Collectanea, the free and self-hosted knowledge base.
---

::u-page-hero{class="dark:bg-gradient-to-b from-neutral-900 to-neutral-950"}
---
orientation: horizontal
---
#top
:hero-background

#title
Your knowledge, [at home]{.text-primary}.

#description
Collectanea is an open-source, self-hosted knowledge base for writing, organising, searching, and sharing documents, with extended Markdown and offline support.

#links
  :::u-button
  ---
  to: /discover/collectanea
  size: xl
  trailing-icon: i-lucide-arrow-right
  ---
  Discover Collectanea
  :::

  :::u-button
  ---
  icon: i-simple-icons-github
  color: neutral
  variant: outline
  size: xl
  to: https://github.com/Smaug6739/Alexandrie
  target: _blank
  ---
  See on GitHub
  :::

#default
  ```bash [Terminal]
  curl -O https://raw.githubusercontent.com/Smaug6739/Collectanea/main/docker-compose.yml
  curl -o .env https://raw.githubusercontent.com/Smaug6739/Collectanea/main/.env.example
  docker compose up -d
  ```
::

::u-page-section{class="dark:bg-neutral-950"}
#title
Documentation for every step

#features
  :::u-page-feature
  ---
  icon: i-lucide-rocket
  ---
  #title
  Get started quickly

  #description
  Launch a local instance in minutes and create your first workspace.
  :::

  :::u-page-feature
  ---
  icon: i-lucide-pen-line
  ---
  #title
  Write and organise

  #description
  Master documents, permissions, media, and Collectanea's extended Markdown.
  :::

  :::u-page-feature
  ---
  icon: i-lucide-server
  ---
  #title
  Self-host

  #description
  Deploy cleanly with Docker, Coolify, a VPS, and the reverse proxy of your choice.
  :::
::
