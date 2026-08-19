---
name: Rentemester
colors:
  paper: "#F4F1EB"
  paper-raised: "#FBF8F3"
  ink: "#1B1A17"
  ink-muted: "#4C4740"
  accent: "#A6332A"
  on-accent: "#F4F1EB"
  danger: "#8F2A22"
  success: "#2E5E4E"
  warning: "#8A5A12"
  info: "#2D5673"
  accent-soft: "#E8D7D3"
  danger-soft: "#EED9D6"
  success-soft: "#DCE8E1"
  warning-soft: "#EEE3D1"
  info-soft: "#D9E4EB"
typography:
  headline-family: "Source Serif 4"
  body-family: "IBM Plex Sans"
  mono-family: "IBM Plex Mono"
  body-size: "16px"
  body-line-height: "1.5"
  mono-features: "tnum"
spacing:
  xxs: "4px"
  xs: "8px"
  sm: "12px"
  md: "16px"
  lg: "24px"
  xl: "32px"
rounded:
  sm: "2px"
  md: "4px"
  lg: "8px"
components:
  button-primary:
    background-color: "{colors.ink}"
    text-color: "{colors.paper}"
    border-radius: "{rounded.md}"
    padding-inline: "{spacing.md}"
    padding-block: "{spacing.xs}"
  button-secondary:
    background-color: "{colors.paperRaised}"
    text-color: "{colors.ink}"
    border-color: "{colors.inkMuted}"
    border-radius: "{rounded.md}"
  button-danger:
    background-color: "{colors.danger}"
    text-color: "{colors.paper}"
    border-radius: "{rounded.md}"
  table-row:
    background-color: "{colors.paper}"
    text-color: "{colors.ink}"
    border-color: "{colors.inkMuted}"
  badge-status-paid:
    background-color: "{colors.successSoft}"
    text-color: "{colors.ink}"
    border-radius: "{rounded.sm}"
  badge-status-overdue:
    background-color: "{colors.dangerSoft}"
    text-color: "{colors.ink}"
    border-radius: "{rounded.sm}"
  alert-danger:
    background-color: "{colors.dangerSoft}"
    text-color: "{colors.ink}"
    border-color: "{colors.danger}"
  amount-cell:
    text-color: "{colors.ink}"
    font-family: "{typography.monoFamily}"
---

## Overview

Rentemester skal ligne en moderne dansk bekendtgørelse mere end et SaaS-dashboard. Ro frem for stimulering, dokument-følelse frem for app-følelse, og determinisme er også en æstetisk beslutning.

## Colors

Paletten er varm, papirnær og nøgtern. Accent bruges sparsomt og primært til risici, afvigelser og handlinger med konsekvens.

## Typography

Overskrifter bruger serif for dokumentautoritet. Brødtekst bruger en neutral grotesk. Beløb og andre regnskabsnære værdier bruger monospace med tabular figures.

## Layout

Layout bygger på et 8px-grid med luft nok til at gøre fakturaer, lister og myndighedsdokumenter rolige at læse.

## Elevation & Depth

Ingen skygger eller glas-effekter i v1. Hierarki skabes med spacing, borders og papirtoner.

## Shapes

Former er næsten firkantede. Små afrundinger er tilladt for læsbarhed, men ingen pill-buttons.

## Components

Komponenter skal kunne mappes direkte til CLI-human-output, PDF-fakturaer og fremtidige tabeller uden at genopfinde semantik pr. overflade.

## Do's and Don'ts

Do: brug varme papirfarver, faste spacing-trin og tydelig tabular-mono til beløb. Don't: brug emoji, gradients, chatbobler, mørkt tema i v1 eller pynt der får produktet til at ligne generisk SaaS.
