# Research sources: noise reduction, clock and power (stages 6, 14, 15)

A reuse list for researchers and AI agents. It was compiled on 2026-10-17 from the eight research notes in [`research_notes/Noise reduction clock and power/`](<research_notes/Noise reduction clock and power/>) and the report [`reports/Noise reduction clock and power.md`](<reports/Noise reduction clock and power.md>). Its purpose is to let later work on related topics reuse the sources without searching again: the Panasonic RR-DR60's hardware, its OKI codec, its DSP, power, clock, playback speed, noise reduction, or 1990s speech coding.

**This is not the project's source register.** The register is [`docs/hardware/sources.md`](../sources.md) (S-001 … S-011). An ID marked **proposed** (S-012 … S-018, or "S-new (conditional)") comes from the report's "Proposed register changes" table and is **not registered yet**. Nothing here has passed the register's review, so cite the register's row, not this file, in specs and code.

Every URL below is copied from the notes or the report. Where a note names a source without a URL, the entry says "no URL recorded". The facts quoted are the notes' readings. They were not re-checked for this list.

## How to read an entry

Each entry has the title (with author or assignee, and the date or priority date where the notes give one), then a tag line:

- **Type:** manual, service manual, product page, spec sheet, datasheet, patent, standard, paper, article, forum, blog, listing, search tool, and so on.
- **Reliability**, as the notes tagged it: **Primary** (manufacturer documents, datasheets, patent full text, archived official pages), **Secondary** (reputable technical writing, reviews, patents used for a class rather than this device), **Anecdotal** (forums, blogs, sellers, community articles).
- **ID:** an existing S-ID from the register, a **proposed** ID from the report, or "none".
- **Read status:**
  - **read**: read in full or in the relevant parts.
  - **partly read**: summary, claims or selected pages only.
  - **not read**: cited from an abstract, a search snippet or a search summary, or it failed to load. The entry says which.
- **Used by:** which note(s) cite it, abbreviated:

| Code | Note |
|---|---|
| CLK | `clock_codec_rate.md` |
| MSM | `msm7702_datasheet.md` |
| NRD | `nr_documentary_evidence.md` |
| NRT | `nr_technical_candidates.md` |
| SPD | `playback_speed_control.md` |
| SEQ | `power_sequencing_transients.md` |
| PWR | `power_supply_effects.md` |
| SMR | `service_manual_power_clock.md` |
| RPT | the report |

## Access tips

Retrieval tricks and dead ends recorded in the notes. Read these first: several cost the researchers real time.

### OKI MSM7702 datasheet (S-003, E2U0018-28-81)

- **Working URL:** `https://pdf.jiepei.com/msm7702-9605035.html?page=N`, N = 1 … 17. **The host and path must be lowercase.** The uppercase `pdf.jiepei.com/MSM7702-…` answers 301. `mpdf.jiepei.com/MSM7702-…` (the form once in sources.md) returns empty pages or a certificate error (CLK, MSM).
- The page HTML is a positioned text layer over a per-page vector SVG. Reading the text layer out of position gives "garbled" tables (the first pass's problem). Render the page instead (MSM).
- **The SVG drawings** are at `https://pdffile.jiepei.com/pdf1/p00077/html/MSM7702_403306/MSM7702_403306_files/img_NN.svg` (NN = 01 … 17), with `style.css` in the same folder. **They answer 404 without a `Referer` header** and 200 with `Referer: https://pdf.jiepei.com/msm7702-9605035.html?page=1` (MSM).
- **The page images on the mirror are thumbnails only:** `https://pdffile.jiepei.com/pdf1/p00077/img/page/MSM7702_403306_files/MSM7702_403306_N.jpg` returns 240 × 310 px JPEGs for N = 1 … 17 (N = 18 gives 404), too small to read. No high-resolution scan exists there (MSM).
- **Certificates:** curl on the research machine could not verify `pdffile.jiepei.com`'s certificate chain (openssl s_client could). The images were fetched with certificate checks off and treated as untrusted data. Plain `http://` returns 404 (MSM).
- **Render recipe** (MSM): fetch `?page=N`, cut the `<div class="stl_02">` block, point its `<object>` at a local copy of `img_NN.svg` (fetched with the Referer), inline `style.css`, then run `chrome --headless --screenshot --force-device-scale-factor=2 --window-size=760,1080`. The renders use substitute fonts: the layout is approximate, the digits exact (typeset text, not OCR).
- **Symbol-font unit key** (confirmed from the HTML class `stl_21`): `m` → µ, `W` → Ω, `¥` → ×, `Æ` → →, `£` → ≤. So the pages print "±200 mA", "0.1 mF", "1 ms" (jitter), "2.0 mA" (leakage) and "kW", meaning ±200 µA, 0.1 µF, 1 µs, 2.0 µA and kΩ. The supply currents IDD1–3 are in a normal font and really are mA (MSM).
- The mirror page has a "下载PDF" (download PDF) link, which CLK suggests for checking page images. MSM's renders made it unnecessary.
- The same mirror serves the pin-compatible **MSM7579**: `https://pdf.jiepei.com/msm7579-9756125.html?page=N` (CLK read pages 5 and 8 as text; its 9.0 kHz sync maximum may be an extraction error).
- Earlier dead ends, from the register's S-003 row (not re-tried in this effort): datasheet4u served pages 1–3 as images on 2026-10-10 and answered 403 on 2026-10-15; datasheetarchive lists a 17-page original and a 16-page scan, but its download is script-gated and its PDF links are tracking redirects; alldatasheet, datasheet.es and datasheetbank are behind access checks.

### RR-DR60 service manual (S-002, AD9810180A3)

- Internet Archive item **`manual_RRDR60_SM_PANASONIC_EN`**: the file **`RRDR60_SM_PANASONIC_EN_jp2.zip`** holds 14 page images at **600 dpi**. Read them at full resolution with crops; the PDF and viewer renders are too coarse for the schematic's small print (SMR, SEQ).
- Image index (0–13, as SMR and SEQ cite it): 0 cover (LCD with a battery icon); 1 specifications; 3–6 parts list (printed pages 4–7); 7 PCB side B; 8 schematic sheet 3.1 (analog and power); 9 sheet 3.2 (DSP IC2); 10 sheet 3.3 (microcontroller IC4, flash IC7, oscillator IC3, IC5, IC6); 11 the "operation tone" addendum; 12 the schematic notes (voltage legend, package outlines); 13 PCB side A.
- **Reading traps** (SMR Q7, SEQ Q1–Q2):
  - Sheet 3.3 carries **two** bracket buses that both use the letters A–H: a left-hand one at IC4 pins 16–23 (sheet 3.2's key ladders and DSP flags) and a right-hand one at pins 59–66 plus G/H (sheet 3.1's power-control bus). Mixing them up produced four wrong statements in the register's S-002 row. Cite the pin name and number on both ends of a bus.
  - The voltage legend reads "No mark ... Recording, ( ) .... Playback". Only 17 points carry a parenthesized playback value, and none of them is on sheet 3.1. An unmarked value is a recording reading only.
  - Several printed DC values mix two states (Q10/Q13, Q14, Q9). Treat a value that contradicts the topology as suspect.
- ManualsLib has a page viewer of the same supplement (link in the entry below). ManualsLib lists only this 14-page supplement and the operating instructions.

### Owner's manual (S-001, RQT4606-1P)

- The Internet Archive OCR text (`…_djvu.txt`) is titled "Microcassette Recorder", but it is the IC recorder's manual. A second scan is the item `manuallib-id-2654684`.
- OCR quirks: the battery "Lo" symbol comes out as "UOr" or "U01" (PWR).
- OCR line numbers for the speed passages (SPD): controls list about lines 2793–2820; the summary page about 3390; page 10's procedure about 3677–3739; troubleshooting about 3823; the 10-second skip about 3358.
- A web-search summary claimed the manual "presents [the speed control] as a transcription aid". That wording is not in the OCR text (SPD).

### Panasonic product pages and spec sheets

- **Japanese spec sheets (仕様書)** follow the path `https://partner.connect.panasonic.com/sites/default/files/CS/PCO/Product%20Documents/Basic%20Docs/asset/solweb/first/shnm_files/sks-v/<MODEL>.PDF`. NRD probed 37 plausible names (RR-DR10 … RR-DR200, RR-QR10 … RR-QR400, RR-QR005/007/010, RR-XR320/800, RR-US001/006). **Only `RR-DR60.PDF` and `RR-QR240.PDF` exist there.**
- **panasonic.jp** (`https://panasonic.jp/other/products/RR-DR60.html`) still has the RR-DR60's support page, but its body and the manual-download database are rendered by JavaScript. A plain fetch reads neither; a headless browser might.
- **US catalog on the Wayback Machine:** product pages of 1999–2000 are at `http://www.prodcat.panasonic.com:80/shop/product.asp?sku=<MODEL>` (both upper- and lowercase `sku` values were captured). By 2001 the catalog moved to `…/shop/templates/vert_template.asp?ModelId=…&ModelNo=…` and `…/square_template.asp?ModelId=…`. The category page was `http://www.panasonic.com:80/consumer_electronics/audio/tape_recorders.htm` (1999).
- **The Wayback CDX regex search** for "DR60" across panasonic.com, panasonic.co.jp and panasonic.ca returned nothing, and the API intermittently refused connections. The working route was through the category page and `prodcat.panasonic.com/shop/product.asp` directly (NRD). No 1997–2000 captures of panasonic.co.jp pages were found.

### Sites that blocked, failed or need a workaround

| Site | What happened | Workaround used | Note |
|---|---|---|---|
| torexsemi.com, mouser.com (Torex XC6372 datasheet) | HTTP 403 | the `uploadcdn.oneyac.com` mirror of ETR0402_005; the `pdf.searchdatasheet.com` mirror of the older family sheet | SMR, PWR |
| alldatasheet (XC6372, XC61, 2SB1218A pages) | used through search snippets and summaries only | none needed for the figures quoted | CLK, PWR, SEQ |
| datasheet4u, datasheetarchive, datasheet.es, datasheetbank (MSM7702) | 403, script-gated, tracking redirects, access checks | the jiepei mirror (above) | register S-003 |
| electronicsdatasheets.com (2SB1218A listing) | did not load | none | register's "To find" list |
| eBay item pages | 403 | listing titles from search results | NRD, SPD |
| rarecanuse, recorderi, ghoststop (ghost-hunting retailers) | 403 | none | NRD |
| dictaphonerd.com | DNS did not resolve | none | NRD |
| Medium post "The hype around the Panasonic RR-DR60" | 403 | a search snippet only | NRD |
| Google Play (EVP Tape Recorder app) | description not retrievable by fetch | a search snippet only | SPD |
| YouTube | video descriptions not searchable with the available tools | a playlist page only | NRD, SPD |
| bitsavers, 1997 *Microcomputer Family* Selection Guide | first download returned an error page; not retried | none | NRT |
| kakaku.com news (RR-QR280) | page encoding garbled; the fetch tool reconstructed the passage | quote with care | SPD |
| Internet Archive full-text search | worked; `advancedsearch.php?q=…&output=json` returns identifiers | see the search entry below | SEQ |
| worldradiohistory.com, archive.org text search (print catalogs) | not reached within the time budget | none | NRD |

### Other tips

- **US patents as PDFs:** `https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/<number>` serves the full PDF by patent number (NRT used it for nine patents). Google Patents pages give priority dates and machine translations of JP documents.
- **Murata CERALOCK** application manual P17E is hosted on DigiKey's media server; the older catalogue P16E-13 is on media.elv.com. Its text extraction renders "±" as "–" (CLK).
- **Energizer and Torex curves** were read from rendered page images, not from text extraction (PWR; the render script is in the local copies below).
- **ManualsLib's product page** for the RR-DR60 shows the operating instructions' table of contents; NRT read the contents list only, not the body.

## 1. RR-DR60 primary documents

1. **Panasonic RR-DR60 Operating Instructions, RQT4606-1P** (Panasonic; print code "F0698R3078 (D)", read as June 1998 by the F + MMYY pattern).
   URLs: [Internet Archive OCR](https://archive.org/stream/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual_djvu.txt); [Internet Archive item](https://archive.org/details/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual); [second scan, `manuallib-id-2654684`](https://archive.org/details/manuallib-id-2654684); [ManualsLib, page 6 (troubleshooting)](https://www.manualslib.com/manual/1088379/Panasonic-Rr-Dr60.html?page=6). The register's S-001 link is [ManualsLib 304890](https://www.manualslib.com/manual/304890/Panasonic-Rr-Dr60.html) (not used by these notes).
   *Owner's manual; Primary; **S-001**; read (OCR searched in full).*
   Provides: "noise" appears once, in a troubleshooting line about a dirty earphone plug; no filter, hiss or noise-reduction wording. Microphone sensitivity five levels (factory 3); VAS ("works better the lower the microphone sensitivity level"); no VAS or AGC switch. SPEED CONTROL switch, five playback levels (3 at purchase, SLOW 1 to FAST 5), changeable only during playback, its indicator sharing the SENS LEVEL bar; no pitch statement, no ratios. 10-second skip by turning the dial. 110 mW max; earphone 2.5 mm 16 Ω; speaker 2.8 cm 8 Ω. Battery life about 10 h record / 8 h play on Panasonic alkaline at 25 °C; "This unit indicates that the batteries have run down when there is actually still some life left in them. This is to preserve the contents of the memory"; battery icon not shown in stop; replace cells "within about three minutes"; with an earphone in, "Sound will not be heard from the speaker"; no record-monitoring feature described.
   Used by: NRD, SPD, PWR, SEQ, RPT.

2. **Panasonic RR-DR60 Service Manual, Order No. AD9810180A3** (© 1998 Matsushita; a 14-page supplement for the RR-DR60P-S revision).
   URLs: [Internet Archive `manual_RRDR60_SM_PANASONIC_EN`](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN) (600 dpi `RRDR60_SM_PANASONIC_EN_jp2.zip`); [ManualsLib viewer](https://www.manualslib.com/manual/2950781/Panasonic-Rr-Dr60.html).
   *Service manual; Primary; **S-002**; read (all 14 images re-read at full resolution 2026-10-17).*
   Provides: specifications (CELP, 6 kHz, 4 kbit/s, "8 bit µ-law" digital filter, 16 Mbit flash, memory back-up "About 3 minutes", battery life); the full power tree (IC8 XC6372D331PR D type with external Q8 2SD1119, CE = Vbat/2 via R85/R86 270 kΩ, so always on; Q20 3.2 V analog rail and Q9 amplifier supply both switched by P72; Q13/Q10 "H" rail for flash, IC3 and IC5 on P73; codec PDN and DSP /ACFL on P11 with R29 1 MΩ and C30 1000 pF as shunts; amplifier CD via R91 56 kΩ from P70; IC6 XC61AN2002MR on the 3.3 V rail into IC4 /RST; battery sense R40 100 kΩ into AN3; lithium back-up latch Q16–Q18); the clock tree (X1 RSXY8M00M07T 3-terminal 8 MHz resonator with built-in capacitors; IC3 TC7WU04 with R37 1 MΩ and R69 680 Ω; buffered 8 MHz to DSP OSCF0, IC4 OSC1 and IC5; X2 32.768 kHz as IC4's sub-clock; DSP PLL loop filter R23 560 Ω, R22 56 Ω + C23 0.047 µF); scope annotations "T≒2us" (BCLOCK), "T≒170us" (RXSYNC2 → XSYNC and RSYNC), "F=8MHz 3.2V P-P"; S6 SPEED CONTROL into IC4 IRQ3 (pin 41); beep from IC4 P12 via R19 220 kΩ / C16 10 pF; no DSP pin named for noise reduction (/DTMFCS tied high; LMUTE, PSACK, PLLSLEEP, GO5–GO10 unconnected); parts list with part numbers; the recording-mode DC voltages. The cover names the original manual, Order No. AD9807107C1, which was not found (see Open leads).
   Used by: all eight notes and RPT.

3. **ManualsLib product page, Panasonic RR-DR60** (ManualsLib).
   URL: [manualslib.com/products/Panasonic-Rr-Dr60-619491](https://www.manualslib.com/products/Panasonic-Rr-Dr60-619491.html).
   *Listing; Secondary; none; partly read (the contents list only).*
   Provides: lists one 14-page service manual (the supplement) and the operating instructions; the instructions' contents show "Changing Microphone Sensitivity" and "Troubleshooting" and no noise-reduction item.
   Used by: NRT, SEQ.

4. **Panasonic US product page "RR-DR60 Tapeless Digital Audio IC Recorder"** (Panasonic / Matsushita Electric Corporation of America; captured 1999-09-13, 1999-11-18, 2000-05-30 with identical text).
   URLs: [Wayback 1999-09-13](https://web.archive.org/web/19990913100520/http://www.prodcat.panasonic.com:80/shop/product.asp?sku=RR-DR60); alternates [1999-11-18](https://web.archive.org/web/19991118230552/http://www.prodcat.panasonic.com:80/shop/product.asp?sku=RR-DR60), [2000-05-30 (lowercase sku)](https://web.archive.org/web/20000530034535/http://www.prodcat.panasonic.com:80/shop/product.asp?sku=rr-dr60). Removal: [2001-08-20, "Sorry, product 'RR-DR60' does not exist."](https://web.archive.org/web/20010820214239/http://www.prodcat.panasonic.com:80/shop/product.asp?sku=RR-DR60)
   *Product page (marketing copy); Primary, with known errors; **proposed S-012**; read.*
   Provides: MSRP $199.95; 60 minutes; "CLEP [sic] (Code Excited Linear Predictive Coding)"; "16 Meg DRAM [sic]" (S-002: 16 Mbit flash); 99 files; Intro-Scan, LCD, jog dial; **"Slow/Fast Play (+/-40%)"**, the only speed figure in any Panasonic document found; protect marking; automatic time stamp; electret microphone; "Digital VAS"; earphone jack; "3 [sic] 'AAA' Batteries". No noise reduction, filter or AGC claim. The 2001 capture brackets the end of US catalog life.
   Used by: NRD, RPT.

5. **Panasonic US "Tape Recorders" category page** (© 1998 Matsushita Electric Corporation of America; captured 1999-05-08).
   URL: [Wayback 1999-05-08](https://web.archive.org/web/19990508164228/http://www.panasonic.com:80/consumer_electronics/audio/tape_recorders.htm).
   *Category page; Primary; none; read.*
   Provides: lists RQ-L309, RQ-L319, RQ-L349, RN-202, RN-302, RN-404, RN-502, RQ-2102 and "RR-DR60 Tapeless Digital Audio Recorder", the only tapeless model; no feature text.
   Used by: NRD.

6. **Panasonic Japan spec sheet 仕様書 "ICレコーダー RR-DR60"** (Panasonic; one page; undated).
   URL: [partner.connect.panasonic.com … /sks-v/RR-DR60.PDF](https://partner.connect.panasonic.com/sites/default/files/CS/PCO/Product%20Documents/Basic%20Docs/asset/solweb/first/shnm_files/sks-v/RR-DR60.PDF).
   *Spec sheet; Primary; **proposed S-013**; read.*
   Provides: max 60 min / 99 items; jog dial; large LCD; speaker 2.8 cm 8 Ω; 実用最大出力 (EIAJ) 80 mW (the US manual's 110 mW max is a different rating); inside-phone jack 2.5 φ 16 Ω; battery life (EIAJ) about 8 h play / 10 h record on alkaline, about 2 h / 3 h on the supplied manganese cells (National Neo 黒, R03); DC 3 V (単4形 × 2); 55.8 × 91.5 × 13.7 mm, about 69 g; 15 named parts including 可変速再生つまみ (variable-speed play knob) and 10秒前再生・ビープボタン; no ノイズリダクション, ノイズキャンセル, ノイズカット, 雑音 or ローカット. Shows a Japanese domestic product.
   Used by: NRD, RPT.

7. **Panasonic Japan support page "ＩＣレコーダー RR-DR60 概要"** (panasonic.jp).
   URL: [panasonic.jp/other/products/RR-DR60.html](https://panasonic.jp/other/products/RR-DR60.html).
   *Product support page; Primary; none; **not read** (body rendered by JavaScript).*
   Provides: only that the page exists, with a gallery image tagged "RR-DR60-S" and an image ID 4984824359796 (49 = Japan GS1 prefix).
   Used by: NRD.

8. **Panasonic "TSN Technical Service Navigation" June 2000 Vol. 1, CDR00061A0X** (Panasonic service CD-ROM image).
   URL: [Internet Archive](https://archive.org/details/tsnsystem-technical-service-navigation-june-2000-vol.-1-cdr00061a0x).
   *Service documentation catalog; Primary; none; partly read (contents).*
   Provides: lists "IC Recorder: RR-QR240 (Simplified) … RR-QR80 … (Supplement)" and no RR-DR60. Other TSN volumes on the Archive are 2001–2004, so the 1998 original manual is unlikely to be on them.
   Used by: SEQ.

9. **Internet Archive advanced-search API, query "AD9807107C1"** (a search tool, not a document).
   URL: [archive.org/advancedsearch.php?q=AD9807107C1…](https://archive.org/advancedsearch.php?q=AD9807107C1&fl%5B%5D=identifier&rows=20&output=json).
   *Search tool; n/a; none; run 2026-10-17.*
   Provides: 0 results for the original service manual's order number. A search for "RRDR60" found only the S-002 item and the two owner's-manual scans.
   Used by: SEQ.

## 2. Panasonic sibling models and later products

1. **RR-QR80 US product page** (Panasonic; captured 2000-05-24).
   URL: [Wayback](https://web.archive.org/web/20000524195334/http://www.prodcat.panasonic.com:80/shop/product.asp?sku=RR-QR80).
   *Product page; Primary; none; read.*
   Provides: MSRP $89.95; 60 min with 8 MB; LP/SP/HQ modes; 198 files / 2 folders; electret microphone; timer play; no noise reduction.
   Used by: NRD.

2. **RR-QR240 US product page** (Panasonic; captured 2000-04-07).
   URL: [Wayback](https://web.archive.org/web/20000407215737/http://www.prodcat.panasonic.com:80/shop/product.asp?sku=rr-qr240).
   *Product page; Primary; none; read.*
   Provides: MSRP $149.95; 32 MB; LP 240 / SP 120 / HQ 60 min; 396 files / 4 folders; timer recording and playing; no noise reduction.
   Used by: NRD.

3. **RR-QR240 Japanese spec sheet 仕様書** (Panasonic).
   URL: [partner.connect.panasonic.com … /sks-v/RR-QR240.PDF](https://partner.connect.panasonic.com/sites/default/files/CS/PCO/Product%20Documents/Basic%20Docs/asset/solweb/first/shnm_files/sks-v/RR-QR240.PDF).
   *Spec sheet; Primary (for the sibling); none; read.*
   Provides: LP about 266 / SP 133 / HQ 66 min; 4 folders × 99 items; 32 MB flash; 周波数特性 **450–5000 Hz (−6 dB, external mic → inside-phone, HQ mode)**; **S/N 58 dB** (external mic, HQ mode); 90 mW (EIAJ); φ3.5 external microphone jack; no noise reduction. The only Panasonic-published frequency response and S/N for an IC recorder of the period; a sibling figure, not evidence for the RR-DR60.
   Used by: NRD.

4. **RR-QR100 / RR-QR200 / RR-QR400 US product pages** (Panasonic; captured 2001-10-06; three pages, one shared feature list).
   URLs: [RR-QR100](https://web.archive.org/web/20011006054904/http://www.prodcat.panasonic.com/shop/templates/vert_template.asp?ModelId=12000&show_all=false&product_exists=True&active=1&ModelNo=RR-QR100); [RR-QR200](https://web.archive.org/web/20011006054847/http://www.prodcat.panasonic.com/shop/templates/vert_template.asp?ModelId=11656&show_all=false&product_exists=True&active=1&ModelNo=RR-QR200); [RR-QR400](https://web.archive.org/web/20011006060709/http://www.prodcat.panasonic.com/shop/templates/vert_template.asp?ModelId=11988&show_all=false&product_exists=True&active=1&ModelNo=RR-QR400).
   *Product pages; Primary; none; read.*
   Provides: HQ/SP/LP modes; 200 mW output; 99 files in each of 4 folders; timer recording and playback; rec pause; divide; jog dial; "Digital VAS (Voice Activate System)"; Intro-Scan; MSRP $79.95 / $109.95 / $139.95; no noise reduction.
   Used by: NRD.

5. **RR-XR320 "SD IC Recorder" US product page** (Panasonic; captured 2001-05-05).
   URL: [Wayback](https://web.archive.org/web/20010505022710/http://www.prodcat.panasonic.com/shop/templates/square_template.asp?ModelId=9530&show_all=false&product_exists=True&active=1).
   *Product page; Primary; none; read.*
   Provides: "World's Smallest* SD Memory Card Applied IC Recorder" (as of Dec. 2000); "16kHz Sampling Frequency (HQ Mode)"; no noise reduction.
   Used by: NRD.

6. **RR-US310 announcement with ノイズキャンセル再生 (noise-cancel playback)** (Panasonic Japan; February 2013; two news outlets).
   URLs: [AV Watch](https://av.watch.impress.co.jp/docs/news/584343.html); [Phile-web, 2013-01-21](https://www.phileweb.com/sp/news/d-av/201301/21/32342.html).
   *News articles; Secondary; none; read.*
   Provides: the earliest Panasonic IC-recorder "noise cancel" found, a playback feature against air-conditioner and similar noise, about 15 years after the RR-DR60.
   Used by: NRD, RPT.

7. **RR-US090 / US070 / US050 family announcement** (Matsushita; 2005-01-17) **[later]**.
   URL: [ASCII.jp](https://ascii.jp/elem/000/000/346/346061/).
   *News article; Secondary; none; read.*
   Provides: 「自然な音程で再生速度を変更できる"スピードコントロール機能(早聞き/遅聞き)"」, a speed control that changes playback speed "at a natural pitch".
   Used by: SPD, RPT.

8. **ITmedia review of the RR-US050** (2005-05-30, from the URL) **[later]**.
   URL: [ITmedia](https://www.itmedia.co.jp/lifestyle/articles/0505/30/news062_2.html).
   *Review; Secondary; none; read.*
   Provides: 「0.5～2倍の間で5段階の可変速再生」 (five-step variable speed between 0.5× and 2×) with SPEED CONTROL ON; bundled PC software 0.7–2.8× in five steps; no pitch statement.
   Used by: SPD, RPT.

9. **RR-QR280 announcement** (kakaku.com news, 2008-06-05) **[later]**.
   URL: [kakaku.com](https://news.kakaku.com/prdnews/cd=kaden/ctcd=2077/id=2812/).
   *News article; Secondary; none; read (garbled encoding, passage reconstructed by the fetch tool).*
   Provides: 「自然な音程で再生速度を変えられる「5段階スピードコントロール機能」」, a five-step speed control "at a natural pitch".
   Used by: SPD.

## 3. OKI codec datasheets

1. **OKI MSM7702-01/02/03 "Single Rail CODEC" datasheet, E2U0018-28-81** (OKI; this version Aug. 1998, previous Nov. 1996; 17 pages).
   URLs (jiepei mirror, one URL per page): [p. 1](https://pdf.jiepei.com/msm7702-9605035.html?page=1), [p. 2](https://pdf.jiepei.com/msm7702-9605035.html?page=2), [p. 3](https://pdf.jiepei.com/msm7702-9605035.html?page=3), [p. 4](https://pdf.jiepei.com/msm7702-9605035.html?page=4), [p. 5](https://pdf.jiepei.com/msm7702-9605035.html?page=5), [p. 6](https://pdf.jiepei.com/msm7702-9605035.html?page=6), [p. 7](https://pdf.jiepei.com/msm7702-9605035.html?page=7), [p. 8](https://pdf.jiepei.com/msm7702-9605035.html?page=8), [p. 9](https://pdf.jiepei.com/msm7702-9605035.html?page=9), [p. 10](https://pdf.jiepei.com/msm7702-9605035.html?page=10), [p. 11](https://pdf.jiepei.com/msm7702-9605035.html?page=11), [p. 12](https://pdf.jiepei.com/msm7702-9605035.html?page=12), [p. 13](https://pdf.jiepei.com/msm7702-9605035.html?page=13), [p. 14](https://pdf.jiepei.com/msm7702-9605035.html?page=14), [p. 15](https://pdf.jiepei.com/msm7702-9605035.html?page=15), p. 16 (package only; same pattern), [p. 17](https://pdf.jiepei.com/msm7702-9605035.html?page=17). The notes also write the pattern as `https://pdf.jiepei.com/msm7702-9605035.html?page=N`. Page assets (see Access tips): `https://pdffile.jiepei.com/pdf1/p00077/html/MSM7702_403306/MSM7702_403306_files/img_NN.svg` (needs a Referer) and the thumbnails `https://pdffile.jiepei.com/pdf1/p00077/img/page/MSM7702_403306_files/MSM7702_403306_N.jpg`.
   *Datasheet; Primary; **S-003**; read in full 2026-10-17 (rendered page images, cross-checked against the text layer).*
   Provides (page in brackets):
   - **Clocking:** "Built-in PLL eliminates a master clock" [1]; XSYNC "triggers the PLL and synchronizes all timing signals"; sync "8 kHz ±50 ppm to guarantee the AC characteristics", operable over 8 kHz ± 2 kHz but "not guaranteed" [5]; BCLK list of 13 rates (64 … 2048 kHz, 1544, 200; no 500 kHz) [5]; recommended sync **6.0 / 8.0 / 10.0 kHz**, sync pulse width 1 BCLK … 100 µs, **allowable jitter 1 µs** [8]; application circuit labels "8 kHz SYNC signal input" [14]. No PLL lock time, no internal switched-capacitor clock rate.
   - **Levels:** transmit 0 dBm0 = 0.35 Vrms (0.338–0.362) [11]; GSX ±0.7 V [9], VAIN ≤ 1.4 Vpp [8], so full scale = +3.0 dBm0; receive 0 dBm0 = 0.50 Vrms (0.483–0.518) at **AOUT**, ±1.0 V about VDD/2 into ≥ 1.2 kΩ [4, 9, 14].
   - **Noise and distortion:** idle noise transmit **−70.5 typ / −68 max dBm0p** (AIN = SG), receive **−78 / −74** [11]; transmit S/D 43/41/37/29.5/25 dB at +3/0/−30/−40/−45 dBm0, receive 43/41/40/33.5/30 [10]; gain tracking [10]; IMD −52 typ / −35 max dBm0 [12]; crosstalk T→R 75/85, R→T 70/80 dB [11].
   - **Templates:** transmit loss 60 Hz ≥ 20 (26 typ), 300 Hz +0.1, 2020 Hz −0.04, 3000 Hz +0.13, **3400 Hz 0.5 typ (0 … 0.80)**; receive 300 Hz −0.04, 2020 Hz +0.02, 3000 Hz +0.10 (**−0.15 … +0.20**), **3400 Hz 0.47 typ** [10]; group delay distortion identical to the MSM7717 [11]; discrimination 4.6–72 kHz ≥ 30 dB (32 typ), out-of-band spurious −37.5 typ / −35 max dBm0 [12]; absolute delay ≤ 0.60 ms at BCLK 64 kHz [11].
   - **Power:** VDD 2.7–3.8 V, "Voltage must be fixed" [5, 8]; IDD 5/9 mA operating, 0.01/0.05 mA power-down, 1.2/3.0 mA power-save [9]; **PSRR 30 dB typ only, 0–50 kHz, 50 mVpp, idle** [12]; SG = VDD/2, ±200 µA drive, "undefined during power saving or power down", 0.1 µF on SGC [7]; AOUT at SG and GSX "at AG voltage" in power-down [4]; latch-up and low-noise-supply design notes [15].
   - **Pins:** 20-pin SSOP and 24-pin SOP maps; -02 = µ-law; ALAW only on -01, internally pulled down [1, 3, 7]; µ-law codes [6]; input op-amp configurations, gain ≤ 10 [4]; package drawings only [16–17].
   Absent anywhere in pp. 1–17: internal filter clock, PLL lock time, SG settling, noise or impedance, a start-up mute, op-amp open-loop gain, PSRR versus frequency, overload behavior.
   Used by: CLK, MSM, SEQ, PWR (as a lead), SPD (as a lead), RPT.

2. **OKI MSM7717-01/02/03 datasheet, E2U0041-28-81** (OKI; Aug. 1998; 19 pages).
   URL: [igorkov.org mirror PDF](http://igorkov.org/revo/datasheets/M7717.pdf).
   *Datasheet; Primary for the MSM7717, now a corroborating sibling of the MSM7702; **S-005**; read in full (2026-10-10, register).*
   Provides: the same clocking rules (jitter stated as "1000 ns", p. 9), µ-law S/D 43/41 dB, idle noise −72.5 / −76.5 dBm0p typ, PSR 30 dB typ (0–50 kHz, 50 mVpp; column alignment medium confidence in PWR's extraction), SG = VDD/2, "undefined during power-saving or power-down mode", the low-noise-supply note. The MSM7702 read (entry 1) supersedes it where they differ.
   Used by: CLK, MSM, PWR, SPD, RPT.

3. **OKI MSM7578 / MSM7579 datasheet** (OKI; pin-for-pin compatible with the MSM7702).
   URLs: [p. 5](https://pdf.jiepei.com/msm7579-9756125.html?page=5), [p. 8](https://pdf.jiepei.com/msm7579-9756125.html?page=8) (jiepei mirror).
   *Datasheet; Primary; none; partly read (pp. 5 and 8, text extraction).*
   Provides: the same XSYNC/RSYNC/BCLK wording and BCLK list; sync range extracted as 6.0 / 8.0 / **9.0** kHz max, possibly an extraction error (the MSM7702 and MSM7717 say 10.0).
   Used by: CLK.

4. **OKI 1987 Voice Synthesis LSI Data Book** (OKI, 1987).
   URL: [bitsavers.trailing-edge.com](http://bitsavers.trailing-edge.com/components/oki/_dataBooks/1987_OKI_Voice_Synthesis_LSI_Data_Book.pdf).
   *Data book; Primary; none; **not read** (known from a search summary).*
   Provides (per the summary only): in OKI's ADPCM speech-LSI family (MSM5205 / MSM6388 class) the playback clock is selectable and the stored data carries no rate, so a clock change alters speed and pitch together: the low-cost, pitch-changing route.
   Used by: SPD.

## 4. Panasonic / Matsushita DSPs and microcontrollers

1. **Panasonic 1997 Semiconductor Selection Guide** (Microcomputers, Memories, ASICs, Bipolar ICs, Discrete Semiconductors; Panasonic, 1997).
   URL: [bitsavers PDF](http://www.bitsavers.org/components/panasonic/1997_Panasonic_Semiconductor_Selection_Guide.pdf).
   *Selection guide; Primary; **proposed S-new (optional)**; read (DSP table and data pages).*
   Provides: table "Digital Signal Processors (DSP), MN1900/1920/1930 Family" (printed p. 48, PDF p. 50): **MN1932801** (27 ns at 2.7–3.3 V, TQFP100, double-speed MAC, 6K RAM, 30K data ROM) and **MN1933211** (under development; 21.7 ns; 32K × 24-bit ROM); MN1920 series at 92–93 ns. **MN1931712 is not listed.** MN1932801 data page (PDF pp. 266–267): 16-bit fixed point, 28K × 24-bit instruction ROM, bit-reverse addressing, max/min instructions, 16 × 16 → 32 MAC with 40-bit accumulation, PLL input clock "/N", 40 (typ) – 130 (max) mA. MN1933211 (PDF p. 268): input clock 64/N – 92/N MHz. Application diagrams (PDF pp. 36–37): MN1920811/813 and MN1932801 as the "Speech Signal Processor" of PDC digital mobile phones; MN1901611/012 in cordless handsets; AN6182K/S analog record/playback ICs for answering phones. No noise-suppression or VAD function listed for any MN19xx part. Bounds the RR-DR60 DSP at about 33–37 MIPS by family resemblance (an inference).
   Used by: NRT, RPT.

2. **bitsavers Panasonic components directory**.
   URL: [bitsavers.org/components/panasonic/](http://www.bitsavers.org/components/panasonic/).
   *Archive directory; n/a; none; browsed.*
   Provides: for this era only the 1990-91 distributor catalog and the two 1997 selection guides (the *Semiconductor* guide above and a *Microcomputer Family* guide whose first download failed).
   Used by: NRT.

3. **Panasonic "Maintenance and Discontinued Types" list** (a later Panasonic catalog, printed pp. 255–256).
   URL: [tvsat.com.pl PDF](https://tvsat.com.pl/PDF/M/MN15814_PAN.pdf).
   *Catalog page; Primary; none; read.*
   Provides: **MN1931712** listed between MN1930021 and MN1932801, with no alternative and no function stated: the only catalog trace of the RR-DR60's DSP found.
   Used by: NRT.

4. **SeekIC part page "MN1931712BB1"** (parts aggregator).
   URL: [seekic.com/part/8744.html](http://www.seekic.com/part/8744.html).
   *Listing; Anecdotal; none; **not read** (the PDF link was not opened; unverified).*
   Provides: lists MN1931712BB1 in the MN1 series, QFP package, with a PDF link.
   Used by: NRT.

5. **Panasonic MN101C51F LSI user's manual, 21451-010E** (Panasonic; the microcontroller family of the RR-DR60's MN101C16ACA3).
   URL: [bitsavers mirrorservice](https://bitsavers.mirrorservice.org/components/panasonic/panaXseries/21451-010E_MN101C51F.pdf).
   *LSI manual (family proxy); Primary for the MN101C51F, Secondary as a proxy; none; partly read.*
   Provides: "the starting mode is NORMAL mode that high oscillation is the base clock"; ports are tri-state, and "At reset, the input mode is selected and pull-up resistors are disabled (high impedance output)"; some port bits offer n-channel open drain. Supports the reading that the H rail (R63 pull-up on Q13) comes on at reset. The MN101C16's own manual was not found.
   Used by: SEQ.

## 5. Patents

Grouped by assignee. Dates are earliest priority unless stated. "Read status" distinguishes full text from summary or claims.

### Matsushita / Panasonic: noise suppression in the CELP group

1. **US 5,774,846, "Speech coding apparatus, linear prediction coefficient analyzing apparatus and noise reducing apparatus"** (Matsushita; Toshiyuki Morii; priority JP 1994-12-19, further 1994-12-26 and 1995-07-14; filed 1995-11-20; granted 1998-06-30).
   URL: [Google Patents](https://patents.google.com/patent/US5774846).
   *Patent; Secondary (class; nothing ties it to the device); **proposed S-new (conditional)**; partly read (the FIG. 17 embodiment not read past the summary).*
   Provides: a pre-encoder spectral-subtraction noise reducer: per-frequency noise estimate as the minimum of the current and a set number of past spectra ("works whether or not speech is present"), stabilizing and noise-reducing coefficients, over-subtracted bins filled from the previous output, LPC-based spectral emphasis, frame-to-frame waveform matching; target: digital portable telephones.
   Used by: NRT, RPT.

2. **JP 3183104 B2, "Noise reduction device"** (Matsushita; Morii; filed 1995-07-14; granted 2001-07-03).
   URL: [Google Patents](https://patents.google.com/patent/JP3183104B2/en).
   *Patent; Secondary (class); **proposed S-new (conditional)**; read (machine translation).*
   Provides: 256-point FFT on 160-sample frames at 8 kHz plus a 60-sample look-ahead; two noise candidates per frequency with persistence counts; noise-reduction coefficient 20.0 (lowered when no noise is detected); negative bins replaced by the smaller of input and previous spectrum; input phase; LPC pole, high-frequency and power emphasis; triangular-window overlap; a memory-saving mode sharing one noise value over four bins. Background cites VSELP 11.2 kbit/s and PSI-CELP 5.6 kbit/s; applications phones, multimedia, voice guidance; no recorders.
   Used by: NRT, RPT.

3. **US 6,799,160 B2, "Noise canceller"** (Matsushita; Yasunaga, Morii, Ehara; priority JP 1996-11-07; filed 2001-04-30 as a continuation in the S-010 family).
   URL: [Google Patents](https://patents.google.com/patent/US6799160B2/en).
   *Patent; Secondary (class); **proposed S-new (conditional)**; read (the eighteenth embodiment, Table 10 and claims, from the continuation's text).*
   Provides: a fully parameterized noise canceller ahead of the coder's buffer (FIG. 27, Table 10): frame 160 samples (20 ms at 8 kHz), pre-read 80, 256-point FFT, LPC order 10, sustaining number 30 frames (0.6 s), designated minimum power 20.0, noise reference power 20000.0, unvoiced-segment detection 0.05, cancellation coefficient 1.5, learning 0.8, **unvoiced-segment power reduction 0.3** (power or amplitude not stated), compensation power increase 2.0, 5 consecutive noise references, power enhancement 1.2; random phase in compensated bins (counter stepping by 2, reset at 16); three spectral-enhancement coefficient sets; triangular cross-fade. The report's template if stage 6 is ever modeled. Whether the canceller was already in the 1996 priority text is unconfirmed.
   Used by: NRT, RPT.

4. **EP 1 136 985 B1** (Matsushita; Yasunaga, Morii, Watanabe, Ehara; priority JP 1996-11-07, further 1996-11-21 and 1997-02-19; filed 1997-11-06).
   URL: [Google Patents](https://patents.google.com/patent/EP1136985B1).
   *Patent; Secondary (class); part of **S-010**; partly read (the fetched part showed only the coder).*
   Provides: the November-1996 CELP coder/decoder family in which the noise canceller of entry 3 appears as one embodiment.
   Used by: NRT.

5. **EP 0 700 166, "Noise suppressing device"** (Matsushita; Nohara, Kane; priority 1994-09-02).
   URL: [EPO publication server, EP0700166 A3](https://data.epo.org/publication-server/rest/v1.2/patents/EP0700166NWA3/document.html).
   *Patent; off topic; none; read enough to exclude.*
   Provides: a radio-tuner noise suppressor driven by carrier level and modulation degree, not speech-coder noise reduction (a negative result).
   Used by: NRT.

6. **US 7,054,808 B2, "Noise suppressing apparatus and noise suppressing method"** (Matsushita; Koji Yoshida; priority not checked).
   URL: [Google Patents](https://patents.google.com/patent/US7054808B2).
   *Patent; unknown; none; **not read** (surfaced by search).*
   Used by: NRT.

### Matsushita / Panasonic: playback speed

7. **JPH0854895A, "Playback device"** (Matsushita; Shuji Miyasaka; priority 1994-08-11; published 1996-02-27).
   URL: [Google Patents](https://patents.google.com/patent/JPH0854895A/en).
   *Patent; Primary (the device maker's practice); none; partly read (the pitch method is not stated in the text read).*
   Provides: speech in "a semiconductor memory" read frame by frame, each frame's speed chosen from a per-frame parameter (amplitude, level, voiced/unvoiced, formant change, scale factors or filter coefficients); silent frames at about 2×, hard-to-hear frames slower.
   Used by: SPD, RPT.

8. **JPH01152499A, "double speed playback device"** (Matsushita; filed 1987; pre-era).
   URL: [Google Patents](https://patents.google.com/patent/JPH01152499A/en).
   *Patent; Primary; none; partly read.*
   Provides: pitch restored after fast playback by a frequency-conversion stage driven by the speed setting (probably tape-oriented): pitch correction as a company design goal by the late 1980s.
   Used by: SPD.

### OKI (the codec maker)

9. **EP 0 714 089 B1, "Code-excited linear predictive coder and decoder"** (OKI; Aoyagi, Ariyama, Hosoda; priority JP 1994-11-22; filed 1995-11-16).
   URL: [Google Patents](https://patents.google.com/patent/EP0714089B1/en).
   *Patent; Primary for the codec maker's practice, Secondary for the device; **proposed S-018**; read.*
   Provides: the decoder "deletes or interpolates a number of samples equal to an integer multiple (n) of the cycle count (cc)", one pitch cycle at a time on the excitation, so it "can speed up or slow down the reproduced speech signal Sp without altering its pitch"; "The simplest way … is to delete the initial cycles"; disturbances reduced by filtering; aimed at answering machines that replace tape with IC memory (at 2/3 speed "the recording time is extended by fifty per cent").
   Used by: SPD, RPT.

### Lucent

10. **US 5,717,823 A, "Speech-rate modification for linear-prediction based analysis-by-synthesis speech coders"** (Lucent; W. B. Kleijn; priority 1994-04-14).
    URL: [Google Patents](https://patents.google.com/patent/US5717823A/en).
    *Patent; Secondary (class); **proposed S-018**; read.*
    Provides: insert or remove pitch-period segments of the decoder's excitation before LPC synthesis, "without substantially altering the pitch"; artifacts: "pop and click sounds", speech that sounds "buzzy or noisy", and slowing beyond 1.5× that "often sounds annoying"; combining coding with rate change "is demanding of hardware resources", reduced by reusing the coder's own pitch delay and LPC filter; applications include machine dictation.
    Used by: SPD, RPT.

### Sony

11. **US 5,873,059 A** (Sony; Iijima, Nishiguchi, Matsumoto, Omori; priority 1995-10-26).
    URL: [Google Patents](https://patents.google.com/patent/US5873059A/en).
    *Patent; Secondary (class: a harmonic/sinusoidal coder, not CELP); none; read.*
    Provides: time-compress or expand the encoded parameters' output period and decode at a fixed 20 ms frame interval; "the contents of a solid-state recording can be reproduced at a speed, for example, twice the real-time speed"; artifacts: "a foreign noise" from abrupt LPC changes, and slowed playback that becomes "unrealistic since many identical parameters" are reused (adds noise as a fix).
    Used by: SPD.

### U.S. Philips

12. **US 5,675,333** (U.S. Philips; priority 1994-08-31; a "digital compressed sound recorder" with CELP).
    URL: [Google Patents](https://patents.google.com/patent/US5675333).
    *Patent; Secondary; none; read enough to exclude.*
    Provides: a negative result: surfaced by a Matsushita search but is Philips, and mentions no silence detection or noise reduction.
    Used by: NRT.

### Toshiba

13. **EP 1 154 408 B1** (Toshiba; priority 2000-05-10; after the device).
    URL: [Google Patents](https://patents.google.com/patent/EP1154408B1/en).
    *Patent; Secondary; none; partly read.*
    Provides: the noise suppressor selected or parameterized per coding rate; the low-rate set suppresses more aggressively, and the highest rate can switch suppression off per band.
    Used by: NRT.

### Motorola and IS-127-related patents (assignees not recorded in the notes unless stated)

14. **US 4,811,404, "Noise suppression system"** (Motorola per NRT; date not recorded).
    URL: [USPTO PDF](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/4811404).
    *Patent; Secondary; none; partly read.*
    Provides: the origin of the 16-channel layout later used by the IS-127 (EVRC) noise suppressor.
    Used by: NRT.

15. **US 7,555,075, "Adjustable noise suppression system"** (assignee and date not recorded).
    URL: [USPTO PDF](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/7555075).
    *Patent; Secondary; none; partly read (its description of IS-127).*
    Provides: IS-127's suppressor as patents describe it: 64-point FFT bands grouped into 16 channels; per-channel gain in dB = slope × (channel SNR − threshold) plus a minimum overall gain (e.g. −13 dB), clamped to 0 dB; noise estimate updated in energy minima or non-voice frames; a voice metric from quantized channel SNRs.
    Used by: NRT.

16. **US 7,912,567, "Noise suppressor"** (assignee and date not recorded).
    URL: [Google Patents](https://patents.google.com/patent/US7912567).
    *Patent; Secondary; none; partly read.*
    Provides: a further description of the IS-127 suppressor (cited with entry 15).
    Used by: NRT.

17. **US 6,397,177** (assignee and date not recorded).
    URL: [USPTO PDF](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/6397177).
    *Patent; Secondary; none; partly read.*
    Provides: dates IS-127 to 1996 (against CouthIT's 1995; see Standards).
    Used by: NRT.

18. **US 5,533,133, "Noise suppression in digital voice communications systems"** (assignee and date not recorded).
    URL: [USPTO PDF](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/5533133).
    *Patent; Secondary; none; partly read.*
    Provides: a Hughes design that put echo cancelling and noise suppression on a separate TMS320C31 (no MIPS figure).
    Used by: NRT.

### Other noise-suppression, comfort-noise, postfilter and swirl patents (assignees not recorded)

19. **US 7,133,824, "Noise reduction method"**. URL: [USPTO PDF](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/7133824). *Patent; Secondary; none; partly read (background).* Provides: musical noise as "little islands of spectrum power … that appear randomly in different frequency bins from frame to frame"; the Berouti–Schwartz–Makhoul over-subtraction and spectral floor; Boll (1979) as the founding paper. Used by: NRT.
20. **US 6,317,709, "Noise suppressor having weighted gain smoothing"**. URL: [USPTO PDF](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/6317709). *Patent; Secondary; none; partly read.* Provides: further background on over-subtraction and musical noise. Used by: NRT.
21. **US 6,807,525, "SID frame detection with human auditory perception compensation"**. URL: [Google Patents](https://patents.google.com/patent/US6807525). *Patent; Secondary; none; partly read.* Provides: G.729 Annex B's SID decision (Itakura distance between LPC filters, energy change since the last SID) and its over-sensitivity to very-low-level background noise; SID frames of 15/16 bits. Used by: NRT.
22. **US 7,092,875, "Speech transcoding method and apparatus for silence compression"**. URL: [USPTO PDF](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/7092875). *Patent; Secondary; none; partly read.* Provides: G.723.1 Annex A's VAD/CNG structure (the SID size was not verified). Used by: NRT.
23. **US 6,941,263, "Frequency domain postfiltering for quality enhancement of coded speech"**. URL: [Google Patents](https://patents.google.com/patent/US6941263). *Patent; Secondary; none; partly read.* Provides: cites Chen and Gersho (1995): the short-term postfilter attenuates noise in spectral valleys, where coding noise is most audible. Used by: NRT.
24. **US 7,124,077 B2**. URL: [Google Patents](https://patents.google.com/patent/US7124077B2). *Patent; Secondary; none; partly read.* Provides: a second citation of the Chen–Gersho postfilter. Used by: NRT.
25. **US 7,103,539 B2** (priority 2001). URL: [Google Patents](https://patents.google.com/patent/US7103539B2/en). *Patent; Secondary; none; partly read.* Provides: conventional postfilters reduce energy in local spectral valleys proportionally more than at peaks. Used by: NRT.
26. **US 8,457,953, "Method and arrangement for smoothing of stationary background noise"**. URL: [USPTO PDF](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/8457953). *Patent; Secondary; none; partly read (background section).* Provides: "swirling" as one of the most severe degradations of stationary backgrounds in CELP coders (temporal fluctuation of decoded power and spectrum, lessening with bit rate); cites the pre-1996 fixes US 5,632,004 (LPC smoothing or bandwidth expansion during inactivity), US 5,579,432 (only for stationary backgrounds) and US 5,487,087 (reduced-fluctuation quantizers) (no URL recorded for those three; not read). Used by: NRT.
27. **US 5,633,982, "Removal of swirl artifacts from CELP-based speech coders"**. URL: [USPTO PDF](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/5633982). *Patent; Secondary; none; partly read.* Provides: a VSELP-era statement that the coders "do a poor job of modelling the signal" with background noise. Used by: NRT.

### Speech-speed conversion, identified but not read

28. **JP 2955247 B2, "Speech speed conversion method and apparatus"** (assignee not checked). URL: [Google Patents](https://patents.google.com/patent/JP2955247B2/en). *Patent; unknown; none; **not read**.* Used by: SPD.
29. **JPH08147874A, "Speech speed conversion device"** (1996; assignee not confirmed). No URL recorded. *Patent; unknown; none; **not read**.* Used by: SPD.

## 6. Standards and standard implementations

1. **ITU-T G.729 (03/96) and its Annex B (VAD, DTX, CNG)**.
   URLs: [Wikipedia, G.729](https://en.wikipedia.org/wiki/G.729) (the notes' citation); the register's S-009 link to the Recommendation is [itu.int](https://www.itu.int/rec/T-REC-G.729-199603-I/en).
   *Standard (via an encyclopedia summary); Secondary as cited here; **S-009** (the main Recommendation); partly read.*
   Provides: Annex B's VAD, DTX (updates background-noise parameters in non-speech frames) and comfort-noise generation from 2-byte (15/16-bit) SID frames. The 140 Hz pre-processing high-pass is quoted from the standard texts but was not re-verified in this effort.
   Used by: NRT.

2. **ITU-T G.723.1 and its Annex A (VAD/CNG)**.
   URL: none recorded for Annex A (described through US 7,092,875 in section 5); the register's S-008 link to G.723.1 is [itu.int](https://www.itu.int/rec/T-REC-G.723.1-200605-I/en).
   *Standard; Primary for the standard; **S-008**; Annex A **not read** in this effort.*
   Provides: the same VAD/CNG structure for G.723.1; a DC-removal pre-filter (from the standard text, not re-verified). The SID size remains unverified.
   Used by: NRT.

3. **TIA/EIA IS-127 (EVRC) noise suppressor**: dating source, CouthIT "EVRC-A" portfolio page.
   URL: [couthit.com/portfolio/evrc](https://couthit.com/portfolio/evrc/).
   *Vendor page; Secondary; none; partly read.*
   Provides: dates IS-127 to 1995 (US 6,397,177 says 1996). The standard text itself was not read; the suppressor's details come from patents (section 5, entries 14–17).
   Used by: NRT.

4. **TI SPRA566B, "IS-127 EVRC on TMS320C6000"** (Texas Instruments application report).
   URL: [ti.com/lit/pdf/spra566b](https://www.ti.com/lit/pdf/spra566b).
   *Application report; Secondary; none; partly read.*
   Provides: in TI's EVRC implementation, submodule 1 holds "the input signal preprocessing and all the subroutines associated with noise suppression"; no MIPS figure for the suppressor.
   Used by: NRT.

## 7. Speech-processing literature (noise estimation, spectral subtraction, postfilters)

1. **R. Martin, minimum-statistics noise estimation, EUSIPCO 1994, pp. 1182–1185**.
   URLs: [RWTH Aachen IKS publication page](https://www.iks.rwth-aachen.de/publikationen/publications-detail//martin94b/); [Ruhr-Uni Bochum, noise power estimation](https://www.ruhr-uni-bochum.de/ika/forschung/forschungsbereich_martin/speech_audio_processing/noise_power.htm).
   *Paper (publication pages); Secondary; none; partly read (pages, not the paper).*
   Provides: noise power estimated from minima of a smoothed power estimate with no speech-activity detector; minima are biased low and need compensation; the window sets the adaptation time.
   Used by: NRT.

2. **G. Doblinger, subband minimum tracking, Eurospeech 1995**. No URL recorded. *Paper; Secondary; none; **not read** (cited from secondary material).* Provides: a cheaper subband variant of minimum tracking. Used by: NRT.

3. **M. Berouti, R. Schwartz, J. Makhoul, ICASSP 1979 (over-subtraction and spectral floor); S. Boll, IEEE Trans. ASSP 27(2), 1979 (spectral subtraction)**. No URL recorded. *Papers; Secondary; none; **not read** (known through US 7,133,824 and US 6,317,709).* Provides: over-subtraction factor ≥ 1 set from the frame SNR, a spectral floor, and the trade-off between low-level speech distortion and musical noise. Used by: NRT.

4. **VOCAL Technologies, "Spectral subtraction"** (vendor technical note).
   URL: [vocal.com](https://vocal.com/noise-reduction/spectral-subtraction/).
   *Technical note; Secondary; none; partly read.*
   Provides: a summary of spectral subtraction and musical noise, cited alongside the patents.
   Used by: NRT.

5. **J.-H. Chen and A. Gersho, "Adaptive Postfiltering for Quality Enhancement of Coded Speech", IEEE Trans. Speech Audio Process. 3(1), 1995, pp. 59–71**. No URL recorded. *Paper; Secondary; none; **not read** (known through US 6,941,263 and US 7,124,077).* Provides: a pole-zero long-term postfilter cascaded with a short-term postfilter that attenuates noise in spectral valleys. Used by: NRT.

6. **H. Ehara et al., noise post-processing for low-bit-rate CELP decoders, IEICE Trans. E87-D (2004)** (Matsushita authors; after the device).
   URL: [IEICE author listing, Hiroyuki Ehara](https://global.ieice.org/en_transactions/Author/a_name=Hiroyuki%20EHARA).
   *Paper; Secondary; none; **not read** (abstract via search).*
   Provides: a stationary-noise generator at the decoder; about +0.25 CMOS on a 4 kbit/s candidate decoder and 0.2–0.8 DMOS on G.729 / G.723.1 decoders. Related to S-010 and S-011 (Matsushita's 4 kbit/s work).
   Used by: NRT.

## 8. Answering-machine and TAD chipsets of the period

1. **Dallas Semiconductor DS2132A/Q "Digital Answering Machine Processor"** (© 1995).
   URL: [nyang-tech.com PDF](https://www.nyang-tech.com/datasheet/1761322853/Analog-Devices-Inc.-Maxim-Integrated/DS2132A.pdf).
   *Datasheet; Primary (for that chip); none; read.*
   Provides: 9.8 and 4.9 kbit/s compression (7 or 14 min in 4 Mbit), DTMF and fax-tone detection, echo cancellation for DTMF detection, "Precise signal level detection capability", record/playback gain in 3 dB steps (−9 … +30 dB); the host does AGC from the energy level. **No noise suppression, VAD or comfort noise.** The period's TAD baseline.
   Used by: NRT.

2. **DSP Group** (Wikipedia article) and **Tech Monitor article on DSP Group's D6000 family**.
   URLs: [Wikipedia, DSP Group](https://en.wikipedia.org/wiki/DSP_Group); [Tech Monitor](https://www.techmonitor.ai/?p=169971).
   *Encyclopedia and trade article; Secondary; none; partly read.*
   Provides: DSP Group's first TAD speech processor (1989); D6000 "Xchip sets" used in custom versions by Sanyo and PhoneMate; the D6005 combined a speech-compression DSP, a memory/host ASIC and a codec; no noise-suppression feature mentioned.
   Used by: NRT.

## 9. Power parts

### Boost converter (IC8, XC6372D331PR)

1. **Torex XC6371/XC6372 datasheet ETR0402_005** (Torex; current sheet, A and C types only).
   URL: [uploadcdn.oneyac.com mirror](https://uploadcdn.oneyac.com/attachments/files/05/FC/XC6371A501PR-G.pdf).
   *Datasheet; Primary; **proposed S-014**; read (text and rendered curves, pp. 8–12).*
   Provides: XC6372 "switches from PWM to PFM control during light loads"; output 2.0–7.0 V ±2.5 %; oscillator 50/100/180 kHz ±15 %; soft-start; operation start 0.9–10 V. Electrical table (XC6371/72A501, 5.0 V, 100 kHz): 85/100/115 kHz; max duty 80/87/92 %; **PFM duty 10/17/25 %**; start ≤ 0.90 V; no-load input 12.8 µA; Lx 1.4 Ω; efficiency 85 %; **slow-start 4.0/10.0/20.0 ms**; CE ≥ 0.75 V / ≤ 0.20 V. Curves (A types, 100 µH, 47 µF tantalum): ripple ≈ 5–8 mVp-p light load to ≈ 20–40 mVp-p at 30–100 mA; efficiency ≈ 75–80 % at 0.1 mA, ≈ 80–88 % at 1–50 mA; hold voltage; 1 → 30 mA transient ≈ 0.1 V for a few ms. The figures are for the built-in-transistor A types at 5 V, not the RR-DR60's D type.
   Used by: PWR, RPT.

2. **Torex XC6371 older full-family datasheet (six types)** (Torex; searchdatasheet mirror of torex.co.jp).
   URL: [pdf.searchdatasheet.com](http://pdf.searchdatasheet.com/b30b978d/torex.co.jp/XC6371B350PL.html).
   *Datasheet; Primary; **proposed S-014**; read.*
   Provides: the ordering code that decodes XC6372**D331PR**: D = standby (CE) capability with an **external** switching transistor; "33" = 3.3 V; "1" = **100 kHz**; P = SOT-89-5; R = embossed tape; "External tr. type units use the EXT pin".
   Used by: PWR, RPT.

3. **Torex XC6372 series product page**.
   URL: [product.torexsemi.com](https://product.torexsemi.com/en/series/xc6372).
   *Product page; Primary; none; **not read** (the datasheet behind it answered 403; a search summary gave the frequency digit).*
   Used by: SMR.

4. **Torex end-of-life list EOL-E0520**.
   URL: [torexsemi.com PDF](https://www.torexsemi.com/file/en/products/discontinued/-2016/EOL-E0520.pdf).
   *Discontinuation notice; Primary; none; **not read** (search-result snippet).*
   Provides: the XC6371D/XC6372D types are discontinued (per the snippet).
   Used by: PWR.

5. **alldatasheet page for XC6372E702DR**.
   URL: [html.alldatasheet.com](https://html.alldatasheet.com/html-pdf/178047/TOREX/XC6372E702DR/155/1/XC6372E702DR.html).
   *Aggregator page; Secondary; none; **not read** (aggregator search snippet).*
   Provides: "Oscillation frequency is also selectable from 50kHz, 100kHz, and 180kHz (accuracy: ±15%)"; PWM and PWM/PFM-switchable versions.
   Used by: CLK.

6. **Light-load (PFM) boost-converter noise** (three application notes and articles, all from search summaries).
   URLs: [TI, "Make a Boost Converter Quieter" (SSZT873)](https://www.ti.com/lit/pdf/sszt873); [TI SLVA890](https://www.ti.com/lit/an/slva890/slva890.pdf); [Embedded Computing Design, boost converters at light load](https://embeddedcomputing.com/technology/analog-and-power/analog-semicundoctors-sensors/boost-converter-design-for-sensitive-loads-at-light-load).
   *Application notes / article; Secondary; none; **not read** (search summaries).*
   Provides: in PFM, skipped pulses give a load-dependent sub-harmonic that can land in the audio band; vendors add circuits to keep it above about 20 kHz (TI measured about 53 kHz at no load with one such scheme).
   Used by: PWR.

### Reset detector (IC6, XC61AN2002MR)

7. **Torex XC61 listings on alldatasheet** (XC61 series search and the XC61C datasheet).
   URLs: [alldatasheet XC61 search](https://www.alldatasheet.com/view.jsp?Searchword=XC61&sField=2); [alldatasheet XC61CN4502MR-G](https://www.alldatasheet.net/datasheet-pdf/pdf/2238706/TOREX/XC61CN4502MR-G.html).
   *Aggregator pages; Secondary; none; **not read** in full (search summary).*
   Provides: "XC61A … Low Voltage Detectors (VDF = 0.8 V)" (a 6-page sheet, not obtained); the XC61C covers 0.8–1.5 V low-voltage and 1.6–6.0 V standard detectors, N-ch open drain or CMOS output.
   Used by: PWR.

8. **XC61C distributor listings** (LCSC, Mouser Indonesia, Farnell).
   URLs: [LCSC XC61CN2702MR-G](https://www.lcsc.com/product-detail/C112139.html); [LCSC XC61CN2502MR](https://www.lcsc.com/product-detail/C112138.html); [Mouser XC61x overview](https://www.mouser.co.id/new/torex-semiconductor/torex-xc61x-voltage-detectors); [Farnell XC61CN2002MR](https://uk.farnell.com/torex/xc61cn2002mr/volt-detector-nch-2-0v-sot-23/dp/3605486).
   *Distributor listings; Secondary; none; read (listing text).*
   Provides: the naming pattern XC61C N {VV}{x}{2} MR; XC61CN2002MR sold as "N-Channel Open Drain Output, 2.0Vdetect … SOT-23"; ≈ 0.7 µA standby at 1.5 V. The 2.0 V decode of the RR-DR60's XC61**A**N2002MR rests on this analogy; no XC61A text confirms it.
   Used by: PWR (LCSC, Mouser), SMR (Farnell), RPT.

### Speaker amplifier (IC9, NJM2113) and its equivalent

9. **JRC NJM2113 "Low Voltage Audio Power Amplifier" datasheet, Ver. 2014-03-17**.
   URL: [endrich.com PDF](https://www.endrich.com/Datenbl%C3%A4tter/Aktive%20Komponenten/NJM2113_E.pdf).
   *Datasheet; Primary; **S-006**; read (text and rendered PSRR curves).*
   Provides: CD active < 0.8 V, standby > 2.0 V; R_CD 50/75/175 kΩ at V_CD = 16 V; mute 70 dB; power-down 72 µA at 3 V; ICC1 2.7 mA at 3 V; PO1 ≥ 55 mW at 3 V into 16 Ω; output DC level 1.00/1.18/1.25 V at 3 V; **VOH = V+ − 1.1 V, VOL = 0.21 V at 75 mA**; ±250 mA absolute peak; R+IN 100/150/220 kΩ, RREF 18/25/40 kΩ; output offset ±30 mV; PSRR2 12 dB (C1 0.1 µF, C2 0), PSRR3 52 dB (1.0 / 5.0 µF); **curves for C1 = 1 µF, C2 = 0: ≈ 27–28 dB at 20–300 Hz, ≈ 30 dB at 1 kHz, ≈ 40 dB at 5–10 kHz**; note 3: "C1 and C2 make slow power rise up". The basis of the V+ − 1.6 V ceiling law and the speaker-mute reading in record.
   Used by: PWR, SEQ, RPT.

10. **Motorola MC34119 low-power audio amplifier datasheet** (the NJM2113's pin- and specification-equivalent).
    URL: [rigpix.com PDF](https://rigpix.com/components/mc34119.pdf).
    *Datasheet; Primary (for the MC34119), Secondary as a proxy; **proposed S-015**; read (Figure 1 not digitized; Figure 20 not read).*
    Provides: pin 1 input impedance about 90 kΩ, open = logic 0; when disabled FC1/FC2 do not change and the outputs go high impedance; CD turn-off < 2.0 µs, **turn-on 12–15 ms**, "independent of C1, C2, and VCC"; power-up turn-on through internal 50 k and 125 kΩ, "≈60% longer for VCC = 3.0 V" (Figure 1 to 360 ms); FC1 1.0 µF with FC2 5.0 µF gives typically 52 dB PSRR; "C2 is somewhat dominant at low frequencies, while C1 is dominant at high frequencies"; output DC level 1.0/1.15/1.25 V at 3 V.
    Used by: PWR, SEQ, RPT.

### Transistors

11. **SeCoS 2SB1218A datasheet** (alldatasheet).
    URL: [alldatasheet.net](https://www.alldatasheet.net/datasheet-pdf/pdf/419422/SECOS/2SB1218A.html).
    *Datasheet (second source); Secondary; none; partly read.*
    Provides: PNP "complementary to 2SD1819A" (Q20, Q2 and Q6 in the RR-DR60).
    Used by: SEQ.

12. **Panasonic 2SD1819ASL listing** (GlobalSpec).
    URL: [datasheets.globalspec.com](https://datasheets.globalspec.com/ds/panasonic/2sd1819asl/cfe57e9b-914e-41e5-9d69-f6d3dfb8a2ca).
    *Listing; Secondary; none; partly read (the register notes the PDF itself was not read).*
    Provides: NPN, 50 V, 100 mA, 150 mW, SMini3. The register's open item gives hFE ranks 160–460 from an aggregator. No datasheet was found for the **2SB1295** (Q9, Q10, Q16); its PNP polarity rests on the schematic symbol and the "2SB" prefix.
    Used by: SEQ (also cited in the register's "To find" list).

## 10. Batteries

1. **Energizer E92 (alkaline AAA) datasheet**.
   URL: [batteryjunction.com PDF](https://files.batteryjunction.com/frontend/files/energizer-e92bp-24.pdf).
   *Datasheet; Primary (for the E92), General for the device; **proposed S-017**; read (curves read from renders).*
   Provides: IR 150–300 mΩ fresh; −18 … 55 °C; capacity to 0.8 V at 21 °C ≈ 1150 / 940 / 670 / 440 mAh at 25 / 100 / 250 / 500 mA; the 100 mW constant-power curve (≈ 1.42 V at 0.5 h … ≈ 1.11 V at 10 h, 0.8 V at ≈ 11.7 h); 250 mW to 0.8 V at ≈ 2.9 h; "Tape-Game-Digital Audio" 100 mA 1 h/day curve; 250 mA at 21 / 0 / −20 °C ≈ 2.6 / 1.3 / 0.55 h. Used to infer ≈ 200 mW (record) / 230 mW (play) from the manual's 10 h / 8 h, a 2.96 → 2.22 V pair trajectory and a firmware cutoff near a 2.1–2.2 V pair.
   Used by: PWR, RPT.

2. **Energizer EN92 datasheet**.
   URL: [batteryjunction.com PDF](https://files.batteryjunction.com/frontend/files/EN92.pdf).
   *Datasheet; Primary; none; partly read.*
   Provides: the same 150–300 mΩ fresh IR figure.
   Used by: PWR.

3. **Energizer Alkaline Handbook / application manual**.
   URL: [data.energizer.com PDF](https://data.energizer.com/pdfs/alkaline_appman.pdf).
   *Application manual; Primary (general); **proposed S-017**; read.*
   Provides: the sloping discharge comes from internal-resistance rise; cold raises Ri ("No capacity is actually lost due to cold temperatures"); electronic resistance acts in the first milliseconds, ionic (polarization) after; "the 1000 Hz impedance value will be less than the total effective resistance"; a dual-pulse example, 0.214 Ω (AA).
   Used by: PWR, RPT.

4. **Wikipedia, "Internal resistance"** (citing Energizer).
   URL: [en.wikipedia.org/wiki/Internal_resistance](https://en.wikipedia.org/wiki/Internal_resistance).
   *Encyclopedia; Secondary; none; **not read** (search summary).*
   Provides: a fresh AA (E91) at ≈ 0.9 Ω at −40 °C, ≈ 0.15 Ω at room temperature, ≈ 0.1 Ω at 40 °C (AA, not AAA).
   Used by: PWR.

5. **Panasonic NiMH AAA datasheets: HHR-70AAA and HHR-60AAAH** (modern parts).
   URLs: [HHR-70AAA (cellpacksolutions)](https://cellpacksolutions.co.uk/wp-content/uploads/2016/07/HHR-70AAAB8.pdf); [HHR-60AAAH (DigiKey)](https://mm.digikey.com/Volume0/opasdata/d220001/medias/docus/7181/HHR-60AAAHB2-Datasheet.pdf).
   *Datasheets; Primary (for those cells); none; **not read** in full (search summaries).*
   Provides: HHR-70AAA 1.2 V, 730 mAh typ / 700 min, ≈ 30 mΩ at 1 kHz charged; HHR-60AAAH 550 / 500 mAh, ≈ 35 mΩ.
   Used by: PWR.

## 11. Clock parts

1. **Murata CERALOCK Application Manual P17E** (Murata, 2018).
   URL: [DigiKey media PDF](https://mm.digikey.com/Volume0/opasdata/d220001/medias/docus/7176/490_p17e.ashx.pdf).
   *Application manual; Primary (for current CERALOCK series), General for the device; **proposed S-016**; read.*
   Provides: "Temperature stability is ±0.3 to ±0.5% … within -20 to +80 °C. The initial frequency precision is ±0.5%"; the frequency "may shift when used in the actual IC circuit" (§3.1); CSTLS□G53/56 (3.40–10.00 MHz) ±0.5 % / ±0.2 % / aging ±0.2 %, CSTCE□G (8.00–13.99 MHz) ±0.5 / ±0.2 / ±0.1 %; standard circuit TC4069UBP or TC74HCU04, Rf 1 MΩ, **Rd 680 Ω** (pp. 13–14; the RR-DR60's TC7WU04 with R69 680 Ω is close to it); load capacitance ±10 % moves the frequency ≈ ±0.05 %, supply ≤ ±0.05 % (§5.1); ceramic ±0.5 % vs crystal ±0.001 % vs LC/CR ±2.0 % (pp. 2, 12).
   Used by: CLK, RPT.

2. **Murata CERALOCK catalogue P16E-13** (Murata, 2002).
   URL: [media.elv.com PDF](https://media.elv.com/file/03522_keramikschwinger_data.pdf).
   *Catalogue; Primary; **proposed S-016**; read (the "±" extracts as "–"; the CSA□MG rows were unreadable in this copy).*
   Provides: "Aging (10 years)" column: CSTLS_G 2.00–10.00 MHz ±0.5 % initial, ±0.2 % temperature, **±0.2 % aging**; CSTCE_G 8.00–12.50 MHz **±0.1 %**; CSTCC_G 2–4 MHz ±0.3 %.
   Used by: CLK, RPT.

3. **Kyocera KBR ceramic resonator datasheet (discontinued, 2003)**.
   URL: [ele.kyocera.com PDF](https://ele.kyocera.com/sites/default/files/assets/static/discontinued/kbr_2003_e.pdf).
   *Datasheet; Primary; none; read.*
   Provides: MSB series 3.52–8.00 MHz ±0.5 % tolerance, ±0.3 % temperature (−20 … 80 °C); MSE ±0.5 / ±0.5 %; MKC/MKD/MKE (to 8.00 MHz) ±0.5 / ±0.5 %; no aging figure.
   Used by: CLK.

The RR-DR60's X1 is Panasonic service part RSXY8M00M07T ("CERAMIC OSC", 3-terminal, built-in capacitors); its vendor part is not printed. No datasheet was found for 1990s parts of the likely types (CSA8.00MG, CSA8.00MTZ, CST8.00MTW, TDK FCR8.0M).

## 12. Psychoacoustics (pitch discrimination)

1. **Wikipedia, "Cent (music)"**.
   URL: [en.wikipedia.org/wiki/Cent_(music)](https://en.wikipedia.org/wiki/Cent_(music)).
   *Encyclopedia; Secondary (citing a thesis); none; read.*
   Provides: "humans can distinguish a difference in pitch of about 5–6 cents".
   Used by: CLK, RPT.

2. **E. Buss, C. N. Taylor, L. J. Leibold (2014), J. Speech Lang. Hear. Res. 57(5)**.
   URL: [PMC4192033](https://pmc.ncbi.nlm.nih.gov/articles/PMC4192033).
   *Paper; Secondary; none; partly read.*
   Provides: adult frequency-discrimination thresholds for a 1000 Hz standard about 0.5 % (citing Olsho 1984; Sinnott & Aslin 1985).
   Used by: CLK, RPT.

3. **CCRMA demonstration page "Frequency Difference Limen or JND"** (Stanford CCRMA, correlogram demonstration texts).
   URL: [ccrma.stanford.edu](https://ccrma.stanford.edu/~malcolm/correlograms/text/33%20Frequency%20Difference%20Limen%20Or%20Jnd.html).
   *Demonstration page; Secondary; none; read.*
   Provides: the JND is "about 1/30 of the critical bandwidth".
   Used by: CLK.

4. **W. Wier, W. Jesteadt, D. Green (1977), JASA 61, 178–184** (frequency discrimination). No URL recorded. *Paper; Primary for the measurement; none; **not read**.* Its 1 kHz values are not quoted anywhere in the notes. Used by: CLK (as a lead).

## 13. Other recorders of the period, and later products used for comparison

1. **OM System (Olympus) dictation history page, "Our legacy: audio capture since 1969"**.
   URL: [dictation.omsystem.com](https://dictation.omsystem.com/our-legacy-audio-capture-since-1969).
   *Manufacturer history page; Secondary; none; partly read (search summary gave no noise feature).*
   Provides: the Olympus D1000 (1997) as Olympus's "first fully fledged digital voice recorder" using DSS, with "a high data compression rate while maintaining good sound quality".
   Used by: NRD.

2. **GCN review of the Olympus D1000** (Michael Cheek, 1998-11-09; now on Route Fifty).
   URL: [route-fifty.com](https://www.route-fifty.com/digital-government/1998/11/olympus-digital-recorder-a-good-idea-that-gets-lost-in-translation/291789/).
   *Review; Secondary; none; read.*
   Provides: the 2M flash card "recorded at most 30 minutes in long-play mode", complex controls, poor speech recognition; no noise, filter or playback-speed feature mentioned.
   Used by: NRD, SPD.

3. **GeneralManual, Olympus D1000 spec listing**.
   URL: [generalmanual.com](https://www.generalmanual.com/Recorders/Olympus-D1000-Digital-Voice-Recorder.htm).
   *Spec listing; Secondary; none; read.*
   Provides: "Standard mode: 12kHz Long mode: 8kHz"; nothing on speed control.
   Used by: SPD.

4. **MacTech, "Olympus introduces USB digital voice recorder"** (2002-03-08; the DS-330).
   URL: [mactech.com](https://www.mactech.com/2002/03/08/olympus-introduces-usb-digital-voice-recorder/).
   *News article; Secondary; none; read.*
   Provides: "noise cancelling technology for removing background noise and hiss": the earliest consumer IC-recorder noise-cancel claim found.
   Used by: NRD, RPT.

5. **TechCrunch on the Olympus DS-30 / DS-40 / DS-50** (2007-01-04).
   URL: [techcrunch.com/?p=359377](https://techcrunch.com/?p=359377).
   *News article; Secondary; none; read.*
   Provides: "three types of noise cancellation, too (low-cut filter, voice filter, and noise cancel)".
   Used by: NRD.

6. **TechCrunch, "Sanyo unveils three super-advanced IC recorders"** (2008-11-13).
   URL: [techcrunch.com](https://techcrunch.com/2008/11/13/sanyo-unveils-three-super-advanced-ic-recorders/).
   *News article; Secondary; none; read.*
   Provides: the Sanyo ICR-PS603RM's "noise cancelation" (2008).
   Used by: NRT.

7. **Sony IC recorder Help Guides** (three guides, modern products).
   URLs: [ICD-UX532/UX543 series "Recording Filter"](https://helpguide.sony.net/icd/u53/v1/gb/contents/TP0000003663.html) (about 2012–2014); [low-cut and "NCF (Noise Cut)" filters](https://helpguide.sony.net/icd/b14/v1/en/contents/TP0000340730.html); [troubleshooting contents list](https://helpguide.sony.net/icd/t65/v1/en1/contents/contentslist02.html).
   *Manufacturer help pages; Primary (for those models); none; read.*
   Provides: "Noise Cut" for high-frequency noise and "Low Cut" for rumble, off by default; the NCF filter as a named "noise cut"; playback noise, buzzing/humming and short battery life listed as separate symptoms without linking them.
   Used by: NRD (u53), NRT (b14), PWR (t65).

8. **manualsnet, Sony ICD-50 (1996)**.
   URL: [manualsnet.com/sony/icd-50](https://manualsnet.com/sony/icd-50).
   *Manual aggregator; Anecdotal, low quality; none; read.*
   Provides: lists no noise feature, but its specification text looks generic ("32 MB … ADPCM") and is not trustworthy.
   Used by: NRD.

9. **Kaden Watch reader column on a Sony ICD-70 bought around 1998**.
   URL: [kaden.watch.impress.co.jp](https://kaden.watch.impress.co.jp/docs/readers/memories/1122712.html).
   *Reader column; Anecdotal; none; read.*
   Provides: bought for its per-second time stamps; no noise feature mentioned.
   Used by: NRD.

10. **Olympus Asia, WS-852 features** **[later]**.
    URL: [asia.olympus-imaging.com](https://asia.olympus-imaging.com/product/audio/ws852/feature2.html).
    *Product page; Primary (for that model); none; read.*
    Provides: "the speed change is processed digitally, thus the pitch is not altered", ×0.5 … ×2.
    Used by: SPD.

11. **Sony DPC (Digital Pitch Control)** **[later]**: support article, ICD-PX470 manual, 2009 press release.
    URLs: [Sony Thailand support](https://www.sony.co.th/en/electronics/support/digital-voice-recorders-icd-series/articles/S500019524); [ICD-PX470 manual p. 73 (ManualsLib)](https://www.manualslib.com/manual/1357038/Sony-Icd-Px470.html?page=73); [Sony press release 2009-09](https://www.sony.jp/CorporateCruise/Press/200909/09-0901/).
    *Support page, manual, press release; Primary (for those models); none; read.*
    Provides: DPC changes "the playback speed without changing the pitch", 0.50–2.00× on some models, 0.25–3.00× on the ICD-PX470; marketed in 2009 for language study.
    Used by: SPD.

12. **world-topics.com blog on 早聞き (fast listening) IC recorders** **[later]**.
    URL: [world-topics.com](https://www.world-topics.com/archives/hayakiki/ic-recorder/).
    *Blog; Anecdotal; none; read.*
    Provides: on some recorders 「速度を速くすると音程も高くなってしまう」 (raising the speed also raises the pitch); advises checking the pitch statement; no model or year given.
    Used by: SPD.

## 14. User reports of battery artifacts on other devices

All anecdotal and none about the RR-DR60. PWR used them only to show the generic pattern.

1. **All About Circuits forum, ISD1820 voice-recorder board distortion**. URL: [forum.allaboutcircuits.com](https://forum.allaboutcircuits.com/threads/isd1820-distortion-put-sound-on-device.174133/post-1568401). *Forum; Anecdotal; none; read.* Provides: distorted playback on a 3 V CR2032, clean on 5 V USB; "The CR2032 battery is too small to produce 1/2W of audio power". Used by: PWR.
2. **Tascam forums, "DR-05X whine when recording"**. URL: [tascamforums.com](https://www.tascamforums.com/threads/dr-05x-whine-when-recording.7987). *Forum; Anecdotal; none; read.* Provides: a whine while recording on battery, attributed to grounding or a defective unit, not charge. Used by: PWR.
3. **MPC-Forums post on an MPC500 sampler**. URL: [mpc-forums.com](https://www.mpc-forums.com/viewtopic.php?p=1598841). *Forum; Anecdotal; none; read.* Provides: a user had read that a whine occurs with low battery charge (unverified by the poster). Used by: PWR.

## 15. EVP-community and other public writing about the RR-DR60

Listed neutrally, for what each source says. The notes describe the claims without endorsing or debunking them.

1. **Higgypop, "Why The Panasonic RR-DR60 Has Been Given Legendary EVP Status"** (Steve Higgins; published 2020-02-13, updated December 2020). The register lists it as "Panasonic RR-DR60: A Legendary EVP Recorder".
   URLs: [higgypop.com](https://www.higgypop.com/news/panasonic-rr-dr-60/); first capture [Wayback 2020-02-18](https://web.archive.org/web/20200218211242/https://www.higgypop.com/news/panasonic-rr-dr-60/).
   *Article; Secondary / Anecdotal; **S-004**; read.*
   Provides: the earliest dated instance found that attributes "onboard noise reduction and auto-gain functions" to the RR-DR60, already present in the 2020-02-18 capture; no citation for it; the article's own test (a minute in a silent room gave a short recording, "a scratchy, muffled mess of noises"); a Panasonic USA reply via Facebook on the recall question; claims "mid-1990s … 1996" with no source; contains errors ("sample the sound six times per second", an "about 18kHz" upper limit).
   Used by: NRD, RPT.

2. **Paranormal New England, "Panasonic RR-DR60 IC Recorder EVP Legend: Longer…"** (blog by joerainone, 2014-12-18; replies by Michael J. Baker).
   URL: [wordpress.com](https://paranormalnewengland796925647.wordpress.com/2014/12/18/panasonic-rr-dr60-ic-recorder-evp-legend-longer/).
   *Blog with comments; Anecdotal; none; read in full.*
   Provides: the post does not mention noise reduction. Comment by Michelle Jones (2014-12-18): "auto-gain, noise reduction (yuck)" describes an unnamed more expensive recorder, not the DR60. Comment by "jonas" (2015-02-09): investigators like the DR60 for the "lack of noise filters". Baker's reply (2015-02-09): "below 18 Khz", "artificial noise caused by bad components", a 6 kHz sample rate (the earliest "18 kHz" figure found). A source who "purchased hers back in 1998 for a mere $20". The author's own impression of sounding "under water" and of "static noise (sound artifacts) generated from the recorder itself".
   Used by: NRD, RPT.

3. **ITC Bridge forum thread "Panasonic RR DR60"** (March–July 2013).
   URLs: [itcbridge.com, with forum_id](http://www.itcbridge.com/forum/view_topic.php?id=2062&forum_id=5); alternate [https form without forum_id](https://www.itcbridge.com/forum/view_topic.php?id=2062) (the same thread).
   *Forum; Anecdotal; none; read in full.*
   Provides: users describe audible hiss and VAS behavior and never a noise-reduction feature; "the volume control DOES increase the 'white noise' -- hissing sound coming out of the speaker during record mode" (clockdryve, 2013-03-30); unit-to-unit differences ("some DR60s will continue to record in complete silence and others will not"); "I really wish there was a way to disable the voice activation" (scott39); one member's post-processing plug-ins; no mention of the speed control or of slow playback.
   Used by: NRD, SPD, RPT.

4. **huntdogman.com, "Panasonic RR-DR60: a legendary EVP recorder"** (2023-05-13).
   URL: [huntdogman.com](https://huntdogman.com/panasonic-rr-dr60-a-legendary-evp-recorder/).
   *Blog; Anecdotal; none; read.*
   Provides: reproduces Higgypop's noise-reduction sentences verbatim with a "Source link" to Higgypop and hotlinked images.
   Used by: NRD, RPT.

5. **Skeptical Inquirer, "The Holy Grail of EVP: Panasonic DR60"** (Kenny Biddle, 2023-05-09).
   URL: [skepticalinquirer.org](https://skepticalinquirer.org/exclusive/the-holy-grail-of-evp-panasonic-dr60/).
   *Article; Secondary; none; read.*
   Provides: "released in 1998 as one of the first digital Dictaphones"; no mention of noise reduction, filters, hiss, VAS or AGC.
   Used by: NRD.

6. **John Costella, "Hearing ghosts? EVP and the DR60"** (May 2022, updates 2022–2025).
   URL: [johncostella.com/ghosts](https://johncostella.com/ghosts/).
   *Article; Secondary; none; read.*
   Provides: no mention of noise reduction or playback speed; reports aliasing of high audio frequencies; 6 kHz from the service manual (found via transkommunikation.ch); a claim that "every 160 such samples are compressed using the CELP algorithm into 14 bytes", unverified in these notes.
   Used by: NRD, SPD.

7. **noct-ml, "panasonic-rr-dr60-emulator"** (GitHub, Python).
   URL: [github.com/noct-ml/panasonic-rr-dr60-emulator](https://github.com/noct-ml/panasonic-rr-dr60-emulator).
   *Software project; Anecdotal; none; read.*
   Provides: a VAS gate on low-RMS frames, a 300–2800 Hz band-pass and added hiss (`--snr_db`); no noise-reduction stage; Higgypop is not among its four references (which include a Reddit thread and the Buried Secrets podcast).
   Used by: NRD.

8. **VICE, "Ghouls in the Machine"** (2009-04-01).
   URL: [vice.com](https://www.vice.com/en/article/4w4vnn/ghouls-in-the-machine-824-v16n4).
   *Magazine article; Anecdotal; none; read.*
   Provides: nothing on the DR60 or noise reduction (a negative result).
   Used by: NRD.

9. **GhostTube, "An expert's breakdown of the Panasonic DR60"**.
   URL: [ghosttube.com](https://ghosttube.com/blogs/ghosttube/an-experts-breakdown-of-the-panasonic-dr60).
   *Blog; Anecdotal; none; read.*
   Provides: no speed or pitch content.
   Used by: SPD.

10. **White Light Paranormal, review of the DR60** (2026).
    URL: [whitelightparanormal.com](https://www.whitelightparanormal.com/blog/ldmlghhgbs3p6h7ek5w9c78mlc9s2d).
    *Blog; Anecdotal; none; read.*
    Provides: no speed or pitch content.
    Used by: SPD.

11. **YouTube playlist of DR60 / QR-series overview videos**.
    URL: [youtube.com playlist](https://www.youtube.com/playlist?list=PLXArOUNzRps_damvWEKFyYs53ZgkIwsZa).
    *Video playlist; Anecdotal; none; partly read (titles only).*
    Provides: no speed test identified.
    Used by: SPD.

12. **Google Play, "EVP Tape Recorder (DR60 Style)"** (third-party Android app).
    URL: [play.google.com](https://play.google.com/store/apps/details?id=com.spottedghosts.EVPTapeRecorder&hl=en_US).
    *App listing; Anecdotal; none; **not read** (description not retrievable by fetch; known from a search snippet).*
    Provides: advertises variable-speed playback (an emulator's design choice, not device evidence).
    Used by: SPD.

13. **eBay listings** (two items).
    URLs: [item 206447360785](https://www.ebay.com/itm/206447360785) ("Japanese ver" / "Made in Japan" in the title); [item 204135153389](https://www.ebay.com/itm/204135153389) (gives the speed feature as "2 Speed", conflicting with the manual's five levels).
    *Seller listings; Anecdotal, low reliability; none; **not read** (item pages answered 403; titles from search results, not re-verified).*
    Used by: NRD (206447360785), SPD (204135153389).

14. **Medium, "The hype around the Panasonic RR-DR60"** (Chris Amandier). No URL recorded. *Blog; Anecdotal; none; **not read** (403; a search snippet only: "plenty of people have claimed that the device adds weird sound artifacts").* Used by: NRD.

15. **Reddit thread "Why all the love for the Panasonic RR-DR60?"** and **the Buried Secrets podcast** (both cited by noct-ml). No URL recorded. *Forum / podcast; Anecdotal; none; **not read**.* Used by: NRD.

## Searched without result

Negative results the notes record, grouped by question. "Searched" means web search unless another place is named. Repeat a search only with a new method or a new place.

### Did Panasonic ever document noise reduction for the RR-DR60? (NRD, NRT)

- The owner's manual OCR, for `noise`, `nois`, `filter`, `reduc`, `clear`, `quality`, `cut`, `hiss`, `quiet`, `cancel`: no feature hit (one troubleshooting "noise", hearing-safety "reduce" and "clearly", "cancel" only in erase/lock modes).
- The Japanese spec sheet, for ノイズリダクション, ノイズキャンセル, ノイズカット, 雑音, ローカット: none.
- The 1999–2000 US product page and the 1999 category page: no noise, filter or AGC claim.
- A Panasonic press release for the launch (US or JP): none found.
- Wayback CDX regex search for "DR60" on panasonic.com, panasonic.co.jp, panasonic.ca: nothing (API intermittently refusing); no 1997–2000 panasonic.co.jp captures.
- Japanese press coverage 1997–2000 (AV Watch began 1997): nothing for the RR-DR60.
- A Panasonic/National IC recorder before the RR-DR60: none identified.
- 37 model names on the partner spec-sheet path: only RR-DR60 and RR-QR240 exist; no RR-DR50/55/70/80/100 anywhere.

### Where did the community's "noise reduction" claim come from? (NRD)

- `"RR-DR60" "noise reduction"`, `DR60 EVP "noise filter"`, `"auto gain"`, `"automatic gain"`, and `DR60 EVP "noise reduction" forum 2008–2011`: only Higgypop, its copy, the 2014 blog and listings. **No pre-2020 page attributes noise reduction to the DR60**, and none cites a Panasonic source.
- Skeptical Inquirer (2023), Costella (2022–2025), VICE (2009) and the noct-ml emulator: no noise-reduction statement.
- A published measurement of the RR-DR60's noise floor, idle noise or silence behavior: none found anywhere.

### Did contemporary recorders advertise noise reduction? (NRD, NRT)

- Olympus DS-150 "noise cancel"; Sony ICD-50 / ICD-70 noise cancel; Sharp, Sanyo, Aiwa, Philips IC recorders 1996–2000; Japanese 「ICレコーダー ノイズキャンセル 初搭載 2000–2002」: nothing dated before 2002.
- 1997–1998 IC recorders with a "noise cancel" feature (Olympus, Sony, Sanyo): only modern products.
- 1993–1999 IC recorders or TAD chipsets with a documented spectral noise suppressor: none found.

### What is the MN1931712BB, and could it run a suppressor? (NRT, CLK, SMR)

- "MN1931712", "MN19317", "MN1931 DSP", "MN1900 series DSP speech": no datasheet, data book or article.
- bitsavers' Panasonic directory: only the 1990-91 distributor catalog and the two 1997 selection guides for this era.
- The 1997 selection guide's DSP table: MN1931712 absent.
- S-002: no DSP core-frequency or PLL-multiplier annotation anywhere.
- MIPS or memory figures for the IS-127 suppressor or for Matsushita's canceller: none found.
- A period description of a noise expander inside a speech DSP: none found.

### Matsushita patents on recorder silence handling (NRT)

- Matsushita patents on recorder playback squelch, comfort noise for recorders, or silence compression in IC recorders: none found (not exhaustive).
- Japanese-language searches for ICレコーダ / 雑音 / 松下, 1996–1999: nothing specific. J-PlatPat itself was not searched.

### Does the speed control change pitch, and by what ratios? (SPD, NRD)

- The owner's manual OCR for "pitch", "tone", "natural", "%", "percent", "transcri", "dictat": no hit in the speed passages ("%" only in dimensions). The web-summary claim that the manual calls the feature "a transcription aid" is not in the text.
- No ratio or percentage for levels 1, 2, 4, 5 in either manual. The only figure anywhere is the US page's "+/-40%".
- First-hand descriptions of DR60 slow/fast playback (forums, ITC Bridge thread, Costella, GhostTube, White Light Paranormal, YouTube metadata): none.
- Reddit, Amazon, Epinions (indirectly, via search): nothing on the question.
- 1995–2000 reviews or manuals stating whether Olympus D1000/D1500/DS-150 or Sony ICD speed control changed pitch: none.
- When Panasonic first marketed natural-pitch speed control, or whether any 1990s Panasonic recorder was so described: not determined.

### The clock and the resonator (CLK, SMR)

- Datasheets for the 1990s resonator types (CSA8.00MG, CSA8.00MTZ, CST8.00MTW, TDK FCR8.0M): not found. The CSA□MG rows in the P16E-13 copy were unreadable.
- Aging beyond 10 years, population distribution within ±0.5 %, phase noise of 8 MHz CERALOCK oscillators: no source found.
- A source stating the RR-DR60's exact sample rate: none. X1's vendor part number: not printed.
- In the MSM7702 datasheet (all 17 pages): no PLL multiplication ratio, switched-capacitor clock rate, PLL lock range or time, or behavior below 6.0 kHz; no statement that the sync must be an integer number of BCLKs.

### MSM7702 power and start-up figures (MSM, SEQ, PWR)

Searched within S-003 pp. 1–17 and not present: SG source impedance, SG noise, SG settling or start-up time, PLL lock or wake-up time after PDN, any start-up mute, op-amp open-loop gain, bandwidth, slew rate or noise, PSRR versus frequency, full-scale clipping or overload recovery, the internal filter clock. The S-003 lead for A-050 (SG noise) is exhausted.

### Power parts (PWR, SMR, SEQ)

- XC61A datasheet text, and its hysteresis: not found (the 2.0 V decode rests on XC61C naming).
- 2SB1295 datasheet: not found.
- MN101C16 manual: not found (the MN101C51F family manual was the proxy).
- Ripple and efficiency curves for the XC6372 **D** (external-transistor) type at 3.3 V: not found online. Torex publishes no PFM↔PWM current threshold.
- L1's inductance (parts list gives only "RL09U028T-T COIL"), the back-up cell's part number and chemistry: not printed.
- An AAA-specific curve of internal resistance against depth of discharge or temperature: not found. Discharge data for 1998-era Panasonic alkaline AAA: not found.
- A 1990s voice-recorder teardown or application note quantifying converter ripple in recordings: not found.
- Any source on RR-DR60 or comparable recorder firmware blanking at record start: not found.
- RR-DR60-specific reports of weak-battery artifacts: none (EVP forums not searched in depth).

### The original service manual (SEQ, SMR)

- Web search for "AD9807107C1": no matching document.
- Internet Archive full-text search for "AD9807107C1": 0 results.
- TSN service CD-ROM volumes (2000–2004) on the Internet Archive: no RR-DR60.

## Open leads

Sources identified but not obtained. Each is a candidate next step for anyone continuing this work.

### Panasonic documents

- **The original service manual, Order No. AD9807107C1** (for the RR-DR60P-S; S-002's cover says to use the supplement "together with" it). It may hold a block diagram, IC pin-function tables for the MN101C16 and MN1931712, adjustment notes and perhaps a mode table. Not checked: paid service-manual sellers, Japanese-language sources (Panasonic Japan model numbers), Panasonic's dealer-login service portal.
- **The Japanese owner's manual (取扱説明書)** for the RR-DR60: not online as far as found; panasonic.jp's manual database is JavaScript-rendered. Try a headless browser on the database and on the RR-DR60 support page.
- **A retail box or blister card** (the main remaining primary channel for feature wording), and **1998–2000 Panasonic printed catalogs** (archive.org text search and worldradiohistory.com not reached).
- **A Panasonic press release** for the launch (US or JP), and archived panasonic.co.jp pages of 1997–2000.
- Higgypop's "unnamed recall list" and "copies of dated instruction manuals found online" (its sources, not identified).

### Chips

- **MN1931712BB datasheet or data book:** the PDF behind the SeekIC listing; the 1997 *Microcomputer Family* Selection Guide on bitsavers (retry the download); Panasonic's 1998–1999 selection guides (not online); Nuvoton (Panasonic Semiconductor's successor) for legacy data.
- **MN101C16 LSI manual** (the RR-DR60's microcontroller).
- **OKI family documents:** MSM7508B/7509B (same transmission clocks), the full MSM7578/7579 sheet, MSM7716, OKI application notes for the MSM75xx/77xx family (PLL lock, SG start-up); a request to Lapis/ROHM (OKI's successor) for E2U0018-28-81. The OKI 1987 Voice Synthesis LSI Data Book (bitsavers; cited, not read).
- **Torex XC61A datasheet text** (detect-voltage code, hysteresis, the SOT-23 marking table).
- **2SB1295 datasheet**; the original Panasonic 2SD1819A and 2SB1218A sheets (hFE ranks; also an open item in the register).
- **MC34119 Figure 1** (turn-on time for C1 = 1 µF, C2 = 0) and **Figure 20** (CD input resistance), to digitize; the NJM2113's CD input resistance at 3 V.
- Murata's 1990s CSA□MG catalogue rows in a readable copy.

### Patents

- **JP 2955247 B2** ("Speech speed conversion method and apparatus") and **JPH08147874A** ("Speech speed conversion device", 1996): full texts and assignees.
- The full text of **US 5,774,846**'s FIG. 17 embodiment, and the **1996 Japanese priority text** of the Yasunaga / Morii / Ehara family (to confirm the noise canceller was in it, not added in the 2001 continuation).
- **US 7,054,808 B2** (Matsushita, Yoshida): priority date and content.
- **J-PlatPat searches:** Matsushita applications filed 1995–1998 with FI G10L21/02 (noise reduction) and the keywords 録音再生 / 半導体メモリ / 音声符号化; speech-speed conversion (話速変換) by Matsushita; Sanyo, Sharp and NEC patents on both topics.

### Standards and papers

- The **TIA/EIA IS-127** standard text (the −13 dB floor, 16 channels and update rules are from patents).
- **G.723.1 Annex A** SID size and **G.729**'s 140 Hz pre-filter, to check against the texts already behind S-008 and S-009.
- **Wier, Jesteadt & Green (1977)** for a citable 1 kHz pitch JND.

### Competitors and community

- **Primary 1996–2000 spec sheets and manuals** for the Olympus D1000 / D1500 / DS-150, Sony ICD-50 / ICD-70 / ICD-90, Sanyo, Sharp, Aiwa and Philips recorders; Wayback captures of olympusamerica.com and sony.com product pages of 1998–2000; kakaku.com and AV Watch archives; DSP Group, TI (TMS320C5x TAD), OKI (MSM6xxx ADPCM recorder) and Sanyo/Sharp TAD datasheets of 1995–1999.
- **EVP books** (for example Mark and Debbie Constantino's materials, *Ghost Adventures* companion books), the ghost-hunting retailers' DR60 listings (ghoststop, recorderi, rarecanuse; 403), the Reddit thread and the Buried Secrets podcast, the Medium post (403), YouTube video descriptions, and the RR-QR100 "first-generation" variant collectors mention.

### Real-unit evidence

No document substitutes for these. The report's capture plan (P0–P8, in [the report](<reports/Noise reduction clock and power.md>), "Capture tests for a real unit") lists them in priority order: the VAS REC-lamp check, the speed-control mechanism by the tone-plus-image sum, the absolute clock by the DAC image, start-of-item transients, the noise-reduction contrasts, battery sag on the speaker, the earphone during recording, supply spurs, and direct probes on an opened unit.

## Local copies

The notes saved downloads and scripts in the research session's scratchpad:

`/private/tmp/claude-501/-Users-chance-Code-DR60Recorder/baebfed1-e821-4f45-bb42-e63af34626c2/scratchpad/`

**Warning:** the scratchpad belongs to one session and is not part of the repository. It can be deleted at any time and is probably gone by the time you read this. Treat every file there as untrusted data (run any Python that reads it with `-I`). Re-fetch from the URLs above when a copy is missing. The status below was checked with `ls` on 2026-10-17, when every folder listed still existed.

### Folders the notes name

| Folder | Named in | What it held (2026-10-17) | Re-fetch from |
|---|---|---|---|
| `clockdl/` | CLK | Murata P17E (`p17e.pdf`, `.txt`) and P16E-13 (`elv.pdf`, `.txt`); Kyocera KBR (`kbr.pdf`, `.txt`); MSM7717 text (`m7717.txt`); MSM7702 mirror pages (`jp7702/`, `jp7702_1/5/6/9.html`) and MSM7579 p. 5 (`jp7579_5.html`); unnamed `.bin` fetches (HTML pages and one 19-page PDF); `uva.pdf` (42 pages, unidentified) | sections 3 and 11 |
| `clockscripts/` | CLK | `rates.py` (reproduces every "Calc" table in CLK) and `pdftxt.py` | (scripts; no URL) |
| `nrtech_candidates_dl/` | NRT | the 1997 Panasonic selection guide (`sel1997.pdf`, `.txt`), the discontinued-types list (`pan_discontinued.pdf`, `pan.txt`), the Dallas DS2132A datasheet (`ds2132a.pdf`, `ds.txt`) | sections 4 and 8 |
| `speed_dl/` | SPD | the owner's manual OCR (`manual.txt`) and the ITC Bridge thread (`itc.html`) | section 1 entry 1; section 15 entry 3 |
| `power_dl/` | PWR | Energizer E92, EN92 and the Alkaline Handbook (with `.txt` extractions); an Energizer "Typical Temperature Effects" PDF (`temperat.pdf`, not cited in the notes); MSM7717; MC34119; NJM2113; Panasonic HHR-70AAA (`nimh70.pdf`); Torex XC6371 copies (`xc6371_oneyac.pdf` and others, some of them blocked-page HTML); XC61C; the searchdatasheet page (`sds.html`); the manual OCR (`manual.txt`, `manual_clean.txt`); unnamed `dl_*` fetches | sections 9 and 10 |
| `scripts/` | PWR | `calc.py` (every figure PWR marks "calc"), plus `transients.py`, `render.py`, `p2t.py`, `pdftxt.py`, `crop2.sh` (SEQ's transient tables used "a scratch script" at 192 kHz with the emulator's `RECORD_SOS`; it is probably `transients.py`) | (scripts; no URL) |
| `sm_pc_dl/` | SEQ (as "already in the session scratchpad") | `jp2.zip` (the Internet Archive's `RRDR60_SM_PANASONIC_EN_jp2.zip`, 600 dpi), `sm.pdf`, `djvu.txt`, the extracted images in `ex/` | section 1 entry 2 |
| `msm7702_render/` | MSM ("local renders are in the session scratchpad") | `page01` … `page17` as `.html`, `.png` and `.txt`: the readable page renders | section 3 entry 1, with the Access tips recipe |

### Other folders present but not named in the notes

These held working material from the same session: `msm7702_dl/` (the 17 page HTMLs, the Referer-fetched `img_NN.svg`, `style.css` and the 240 × 310 thumbnails `pg1` … `pg18.jpg`), `msm7702_scripts/` (`build.py`, `sym.py`: the render builder and the Symbol-font mapping), `sm_pc_img/` and `sm_pc_thumb/` (the 14 service-manual page images and thumbnails), `sm_pc_crop/` and `seq_crop/` (the schematic crops behind SMR and SEQ), `sm_pc_tools/` (`crop.sh`), `seq_dl/` (the MN101C51F manual), `ds_xc6372/` (Torex and Mouser fetch attempts, mostly blocked-page HTML), `nr_dl/` (the Wayback captures of the US pages, the two Japanese spec sheets in `jp/`, the Higgypop page and its first capture, the ITC Bridge and Paranormal New England pages, the manual OCR, CDX results), `nr_img/` (renders of the two Japanese spec sheets), `nr_scripts/` (`extract.py`), `render/` (rendered pages of the E92, NJM2113 and XC6371 datasheets), `bench/` (a checker script and fetched pages), and `report_backup.md` (a backup of the report draft).
