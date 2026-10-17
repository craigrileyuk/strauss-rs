---
layout: home

hero:
    name: "Strauss-RS"
    text: "Prefix your plugin's dependencies"
    tagline: A Rust port of Strauss for Wordpress plugins and themes. One binary, no PHP needed to run it.
    image:
        light: /logo.svg
        dark: /logo-light.svg
        alt: Strauss-RS
    actions:
        - theme: brand
          text: Getting started
          link: /guide/getting-started
        - theme: alt
          text: Why prefix?
          link: /guide/why
        - theme: alt
          text: Configuration
          link: /reference/configuration

features:
    - title: Avoids clashes between plugins
      details: Two plugins can bundle different versions of the same library. Strauss-RS gives each plugin its own copy under its own namespace.
    - title: Reads your existing setup
      details: It works out sensible prefixes from composer.json, and the migrate command moves your Strauss PHP settings across.
    - title: Tells you what it did and why
      details: The diff and explain commands show every change before it is made, and which setting decided it.
---
