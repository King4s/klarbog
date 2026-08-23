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

## Kreditnotaer

Fuld kreditnota på en **sendt, ubetalt** faktura er implementeret som
to-faset flow: preview eksakt-negerer send-bogføringen (samme konti inkl.
momsben, flippede retninger, memo `invoice:{id}:credit:{CN-nr} · {reason}`);
commit bogfører og sætter status til void. Begrundelse er påkrævet som i
originalen (DK-CREDIT-NOTE-001). Fail-closed: kladder, betalte og
delbetalte fakturaer afvises (`CreditWithPayments` / ugyldig transition),
og betingelsen re-tjekkes ved commit.

**CN-nummerserie** (porteret fra originalens `sequences.ts` +
`credit-notes.ts`): fortløbende `CN-{regnskabsår}-{NNNN}` med floor fra
allerede udstedte numre. Preview *kigger* på næste nummer uden at skrive
og bager det ind i det digest-bundne memo; commit *reserverer* præcis det
nummer — fail-closed hvis en anden kreditnota kom først
(`SequenceConflict`, originalens reserveSequenceValue-semantik). Nummeret
persisteres på fakturaen (`credit_note_no`) og vises i fakturalisten.
Regnskabsår læses fra `policy.json` (`fiscalYearStartMonth`,
`fiscalYearLabelStrategy`) som i originalens company settings; default er
kalenderår med end-year label.

**Delkreditering** (porteret fra originalens `issueCreditNote`): valgfrit
bruttobeløb i øre (tom = rest); kumulativt loft mod original-brutto;
proportional momsfordeling med residual på den sidste note så summen
lander præcist på original net/vat/gross. Delkredit lader fakturaen som
sent; fuld kumulativ kredit sætter void. Kreditledger `credits[]` på
fakturaen.

## Forhold til originalprojektet (ejerordre 2026-08-21: originalen er facit)

Originalen (`origin/main`, TypeScript) er referencen; porten må ikke
opfinde egen semantik. Kendte afvigelser/huller pr. denne ADR:

- **Beløbskonvention**: originalen modellerer fakturalinjer **ekskl. moms**
  (`unitPriceExVat`) med eksplicitte net/vat/gross-totaler og
  `vatTreatment`. Partstype-konventionen (privat inkl. / erhverv ekskl.)
  er en eksplicit ejerbeslutning 2026-08-21 for portens indtastnings-UI —
  internt fryses net/vat/gross på fakturaen, hvilket matcher originalens
  totals-model.
- **Kreditnota-huller** (originalen har dem, porten endnu ikke):
  manuelt valgt CN-nummer.
  CN-nummerserie, delkreditering med kumulativt loft, konfigurerbart
  regnskabsår og immutable JSON-dokument (sha256 + retain_until) er porteret.
- **Statusmodel**: originalen afleder status af beløb (open/paid/credited/
  refunded/overpaid/written_off); porten har en eksplicit statusmaskine
  (draft/sent/part_paid/paid/void). Portens `record_payment` fail-closer
  på overbetaling hvor originalen registrerer og markerer `overpaid`.
- **Moms-huller**: originalen har `vat_code` pr. journallinje,
  evidensbaseret momssemantik, momsangivelse med rubrikker, omvendt
  betalingspligt (§46 + EU), OSS og momsfrie ydelser. Porten har fast
  25 %-split på faste konti (1200/4000/4500).

Videre arbejde porteres fra originalens featureliste — ikke fra egne idéer.
