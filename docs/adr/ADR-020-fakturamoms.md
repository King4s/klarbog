# ADR-020 — Fakturamoms: beløbskonvention efter partstype

Accepted (2026-08-21 — ejerbeslutning: «fakturabeløb er inkl. eller ekskl.
moms. inklusiv til private og ekskl. til erhverv»)

## Context

Fakturaflowet bogførte hele fakturabeløbet som omsætning (salg) eller udgift
(køb) uden momsben. Kontoplanen (ADR-019) har nu Salgsmoms 1200, Købsmoms
4000 og Momsafregning 4500, og kontoplansiden kan afregne — men intet
fakturasalg producerede salgsmoms.

Beløbskonventionen afhænger af modparten (dansk praksis):

| Partstype | Fakturabeløb | Udledning (25 %) |
|---|---|---|
| Privat (B2C) | **inkl. moms** (brutto) | net = brutto − moms, `split_vat25_inclusive` |
| Erhverv (B2B) | **ekskl. moms** (netto) | moms = 25 % af netto, `split_vat_from_net` |

## Decision

1. **Party får en type**: `kind: private | business` på CRM-parten;
   serde-default `private`, så eksisterende `parties.json` læses uændret.
2. **Momsen fastfryses ved oprettelse**: `Invoice.vat: Option<InvoiceVat>`
   (`net/vat/gross_minor`) beregnes i `create_draft` ud fra partens type og
   linjetotalen, og gemmes på fakturaen. En senere ændring af partens type
   ændrer ikke allerede oprettede fakturaer. `vat: None` = legacy-faktura →
   gammel bogføring uden momsben (ingen migrering; DEV-data genskabes).
3. **Bogføring (send)**: salg → AR debet brutto / omsætning kredit netto /
   Salgsmoms 1200 kredit moms. Køb → udgift debet netto / Købsmoms 4000
   debet moms / AP kredit brutto.
4. **Betaling er brutto**: `remaining/paid` regnes mod `gross_minor()`
   (= `vat.gross` eller linjetotalen for legacy). Bankafstemning matcher
   ligeledes brutto — det er bruttobeløbet der står på kontoudtoget.
5. **Sats**: flad 25 % (`DK_VAT_STANDARD_BPS`); afrunding genbruger
   rules-dk's `vat_split` (én afrundingskilde i systemet).

## Consequences

- `klarbog-plugin-invoice` afhænger nu af `klarbog-plugin-rules-dk`
  (vat-split-matematikken); ingen cyklus, rules-dk kender ikke invoice.
- `upsert_party` tager partstype; UI/HTTP/MCP-flader udstiller den
  (default `private` når feltet udelades — bagudkompatibelt).
- Momsafregningen på kontoplansiden nettoficerer nu salgs- og købsmoms fra
  både journal-momssplit og fakturaflowet.

## Deferred

- Momssats pr. linje / momsfrie ydelser (`vat0`), omvendt betalingspligt
  (EU-køb), kreditnotaer. Egen ADR når behovet opstår.
