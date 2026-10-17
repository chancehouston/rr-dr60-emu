# RR-DR60 playback speed control (SLOW/FAST, five steps): mechanism, ratios and artifacts

Researched 2026-10-17. Era focus 1995–2000; later material is flagged **[LATER]**. Evidence tags: **[PRIMARY]** = manufacturer document or patent full text; **[SECONDARY]**; **[ANECDOTAL]** = user/forum/seller; **[NEGATIVE]** = searched, nothing found. Downloads (untrusted) are in `/private/tmp/claude-501/-Users-chance-Code-DR60Recorder/baebfed1-e821-4f45-bb42-e63af34626c2/scratchpad/speed_dl/` (manual OCR `manual.txt`, ITC Bridge thread `itc.html`).

**Bottom line.** No source found states whether the RR-DR60's speed control changes pitch, or what its five ratios are. The question is still open and needs a capture from a real unit. The circumstantial evidence leans **weakly toward pitch-preserving (DSP time-scaling), at least for the SLOW steps**, for three reasons:
- The codec runs at 6 kHz, the bottom of its recommended sync range.
- The codec's maker (OKI) and the device's maker (Matsushita) both held 1994 patents on pitch-preserving speed change for IC-memory voice playback, OKI's done inside a CELP decoder.
- Panasonic's later five-step speed controls were marketed as natural-pitch.

None of this is direct evidence for this model.

## 1. What Panasonic documentation says

### Takeaway
The owner's manual (RQT4606-1P) describes how to operate the feature. It says nothing about pitch, gives no percentages or ratios, and does not explain the mechanism. The service manual summary in the project's S-002 row lists S6 SPEED CONTROL as a switch and records nothing else about speed.

### Cited Findings
- Controls list [PRIMARY]: "Speed control switch (SPEED CONTROL) • Leave in the [OFF] position unless changing playback speed." The display has two states: "(SPEED CONTROL OFF) Microphone sensitivity indicator" and "(SPEED CONTROL ON) Playback speed indicator". The same LCD bar seems to show either the SENS LEVEL or the speed level. — [Owner's manual OCR, Internet Archive](https://archive.org/stream/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual_djvu.txt) (OCR lines ~2793–2820)
- Summary page [PRIMARY]: "Changing the playback speed. There are five levels of playback speed. (The level is factory set to 3.) See page 10 for details on how to change the setting." — [same OCR](https://archive.org/stream/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual_djvu.txt) (~line 3390)
- Page 10 procedure [PRIMARY]: "There are five playback speeds. (The setting is 3 at time of purchase.)"
  1. "Slide [SPEED CONTROL] to the [ON] position while stopped. The playback speed indicator appears."
  2. "Press [PLAY/SEL/STOP] to playback an item. The playback speed indicator flashes."
  3. "Turn [PLAY/SEL/STOP] to select the required speed."

  The diagram reads "LEVEL: 1 2 3 4 5" with "SLOW" at 1 and "FAST" at 5 ("e.g. if the level 5 is selected"). The page ends: "Playback speed can only be changed during playback." — [same OCR](https://archive.org/stream/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual_djvu.txt) (~lines 3677–3739)
- Troubleshooting [PRIMARY]: symptom "Playback speed is too fast or slow", check "Has playback speed been changed? (See page 10 to set to the required speed.)". This also does not mention pitch. — [same OCR](https://archive.org/stream/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual_djvu.txt) (~line 3823); also [ManualsLib p. 6](https://www.manualslib.com/manual/1088379/Panasonic-Rr-Dr60.html?page=6)
- [NEGATIVE] I grepped the OCR for "pitch", "tone", "natural", "%", "percent", "transcri" and "dictat". There are no hits in the speed passages, and "%" appears only in dimension strings. One web-search summary claimed that the manual "presents this as a transcription aid". **That wording does not appear in the OCR text; treat that summary as wrong.**
- Separate fast-forward/rewind exists [PRIMARY]: "Turn [PLAY/SEL/STOP] rapidly during playback. The unit fast-forwards or rewinds in 10 second intervals." This is a skip, not variable-speed audio, so it is a different function from SPEED CONTROL. — [same OCR](https://archive.org/stream/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual_djvu.txt) (~line 3358)
- Service manual S-002 [PRIMARY, via the project register]: "S6 SPEED CONTROL" appears in the controls list. The DSP (MN1931712BB) has a PLL "(VCO IN / PC OUT) that generates the codec clocks" from an 8 MHz resonator. The codec's sync is marked T ≈ 170 µs (≈ 6 kHz) and BCLK T ≈ 2 µs. — project file `docs/hardware/sources.md`, S-002 row ([Internet Archive PDF](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN))
- A seller listing gives the speed feature as "2 Speed", which conflicts with the manual's five levels [ANECDOTAL, low reliability]. — eBay listing surfaced via search ([example](https://www.ebay.com/itm/204135153389)); not re-verified

### Inferences
- The manual says "speed" throughout and never warns that voices will sound higher or lower. Manuals for pitch-changing consumer gear often do mention that, so its absence is very weak evidence of anything.
- The indicator shares the SENS LEVEL bar's segments, and the setting changes only during playback. That fits a firmware parameter (microcontroller → DSP) more than a hardware clock switch, but both readings remain possible.

### Gaps
- No ratio or percentage for levels 1, 2, 4 and 5 appears in either manual.
- Whether IC4 (microcontroller) or IC2 (DSP) reads S6 is for the schematic researcher.

## 2. User reports (EVP and other users)

### Takeaway
I found no first-hand description of what RR-DR60 (or RR-DR50/55) SLOW/FAST playback sounds like: no deeper or higher voice, and no "just slower" either. The negative result covers forum threads, enthusiast articles and YouTube metadata.

### Cited Findings
- [NEGATIVE] ITC Bridge's long "Panasonic RR DR60" thread (fetched in full, 2013-era posts): it contains no mention of the speed control, slow playback or chipmunk/deeper voices. The only "pitch" hit is a 2013 member asking whether "communicators favor a specific frequency/pitch". — [ITC Bridge thread](https://www.itcbridge.com/forum/view_topic.php?id=2062)
- [NEGATIVE] John Costella's DR60 analysis does not discuss playback speed. It states that the sample rate is 6 kHz and that "every 160 such samples are compressed using the CELP algorithm into 14 bytes of encoded audio". That frame claim belongs to the codec researcher; it is unverified here. — [johncostella.com/ghosts](https://johncostella.com/ghosts/)
- [NEGATIVE] GhostTube's "expert's breakdown" and White Light Paranormal's 2026 review have no speed or pitch content. — [GhostTube](https://ghosttube.com/blogs/ghosttube/an-experts-breakdown-of-the-panasonic-dr60); [White Light Paranormal](https://www.whitelightparanormal.com/blog/ldmlghhgbs3p6h7ek5w9c78mlc9s2d)
- [NEGATIVE] Searches for YouTube speed tests of the RR-DR60 found only a playlist of general DR60/QR-series overview videos, with no speed test identified. — [YouTube playlist](https://www.youtube.com/playlist?list=PLXArOUNzRps_damvWEKFyYs53ZgkIwsZa)
- A third-party "EVP Tape Recorder (DR60 Style)" Android app advertises variable-speed playback. This is an emulator's design choice, not evidence about the device [ANECDOTAL]. — [Google Play listing](https://play.google.com/store/apps/details?id=com.spottedghosts.EVPTapeRecorder&hl=en_US) (description not retrievable by fetch; known only from a search snippet)

### Inferences
- Investigators commonly slow playback in desktop audio editors, so some "slowed" DR60 clips online were probably processed off-device. Those clips cannot tell us how the device's own SLOW steps sound.

### Gaps
- No Reddit, Amazon or Epinions/Wayback reports were found that address the question (searched indirectly). A targeted owner query or capture would settle it: record a steady 1 kHz tone or a sustained vowel, play it at each level, and measure duration and F0.

## 3. Patents 1993–2000 (Matsushita and competitors) and Panasonic's later marketing

### Takeaway
Pitch-preserving speed change for IC-memory speech playback was well patented in 1994–96. This includes **OKI, the codec maker, doing it inside a CELP decoder by deleting or repeating whole pitch cycles**, Lucent's excitation-domain method, Sony's parametric-decoder method, and Matsushita's own semiconductor-memory variable-speed playback. Panasonic marketed its five-step speed control as natural-pitch by 2005 **[LATER]**. I found no Panasonic statement about pitch for 1990s models.

### Cited Findings
- **OKI EP0714089B1** [PRIMARY], "Code-excited linear predictive coder and decoder". Inventors Aoyagi, Ariyama, Hosoda; priority JP 1994-11-22, filed 1995-11-16.
  - The decoder "deletes or interpolates a number of samples equal to an integer multiple (n) of the cycle count (cc)", in blocks of one pitch cycle, on the excitation signal. "The decoder in FIG. 8 can speed up or slow down the reproduced speech signal Sp without altering its pitch."
  - "The simplest way to select the cycles to be deleted is to delete the initial cycles." Disturbances "are moreover reduced by filtering".
  - Its stated application is telephone answering machines that replace cassette tape with IC memory. At a speed factor of 2/3, "the recording time is extended by fifty per cent".
  - — [Google Patents EP0714089B1](https://patents.google.com/patent/EP0714089B1/en)
- **Lucent US5717823A** [PRIMARY], W. B. Kleijn, "Speech-rate modification for linear-prediction based analysis-by-synthesis speech coders", priority 1994-04-14.
  - It inserts or removes pitch-period segments of the decoder's excitation before LPC synthesis, so speech is "shortened or lengthened without substantially altering the pitch".
  - Artifacts: changing the short-term correlation makes "pop and click sounds" audible; disturbing the quasi-periodic structure makes speech "sound buzzy or noisy"; slowing by more than 1.5× "often sounds annoying".
  - Cost: combining coding with rate change "is demanding of hardware resources", and reusing the coder's own pitch delay and LPC filter reduces that.
  - Applications: answering systems and stored messages; "machine dictation" for slower playback.
  - — [Google Patents US5717823A](https://patents.google.com/patent/US5717823A/en)
- **Sony US5873059A** [PRIMARY], Iijima, Nishiguchi, Matsumoto, Omori; priority 1995-10-26.
  - It time-compresses or expands the encoded parameters' output period and decodes at a fixed 20 ms frame interval, "so that speed may be adjusted over a wide range while maintaining a high sound quality".
  - It notes that "the contents of a solid-state recording can be reproduced at a speed, for example, twice the real-time speed".
  - Artifacts: abrupt LPC changes give "a foreign noise" (hence denser LSP interpolation); in slowed playback "the playback sound tends to become unrealistic since many identical parameters" are reused, and the patent proposes adding noise.
  - This is a harmonic/sinusoidal coder, not CELP, but it is the same parametric-domain idea.
  - — [Google Patents US5873059A](https://patents.google.com/patent/US5873059A/en)
- **Matsushita JPH0854895A** [PRIMARY], "Playback device", inventor Shuji Miyasaka; priority 1994-08-11, published 1996-02-27.
  - Speech stored in "a semiconductor memory" is read frame by frame. Each frame's playback speed is chosen automatically from a per-frame parameter: amplitude, level, voiced/unvoiced, formant change, or, for compressed data, scale factors or filter coefficients.
  - Silent frames play at about 2× and hard-to-hear frames slower, by "speed conversion processing" citing prior literature.
  - The pitch method is not stated in the text read.
  - — [Google Patents JPH0854895A](https://patents.google.com/patent/JPH0854895A/en)
- **Matsushita JPH01152499A** [PRIMARY, pre-era, 1987 filing], "double speed playback device": it restores pitch after fast playback with a frequency-conversion stage driven by the speed-control setting. This shows the company treated pitch correction as a design goal by the late 1980s; it is probably tape-oriented. — [Google Patents JPH01152499A](https://patents.google.com/patent/JPH01152499A/en)
- Other speech-speed-conversion patents surfaced but were **not read**: JP2955247B2 ("Speech speed conversion method and apparatus", assignee not checked) and JPH08147874A ("Speech speed conversion device", 1996, assignee not confirmed). — [JP2955247B2](https://patents.google.com/patent/JP2955247B2/en)
- **[LATER, 2005]** Matsushita RR-US090/US070/US050 family (announced 2005-01-17): 「自然な音程で再生速度を変更できる"スピードコントロール機能(早聞き/遅聞き)"」, i.e. a speed control that changes speed "at a natural pitch". — [ASCII.jp](https://ascii.jp/elem/000/000/346/346061/)
- **[LATER, 2005]** ITmedia review of the RR-US050: with the speed-control button ON, 「0.5～2倍の間で5段階の可変速再生」 ("five-step variable-speed playback between 0.5× and 2×"); the bundled PC software offers 0.7–2.8× in five steps. The review does not mention pitch. — [ITmedia](https://www.itmedia.co.jp/lifestyle/articles/0505/30/news062_2.html)
- **[LATER, 2008]** RR-QR280: 「自然な音程で再生速度を変えられる「5段階スピードコントロール機能」」 ("a five-step speed control that changes speed at a natural pitch"). The page's encoding was garbled and the fetch tool reconstructed the passage. — [kakaku.com news, 2008-06-05](https://news.kakaku.com/prdnews/cd=kaden/ctcd=2077/id=2812/)

### Inferences
- OKI made the RR-DR60's codec, and OKI had a CELP-decoder pitch-cycle method aimed at IC-memory answering machines with a 1994 priority. Matsushita had semiconductor-memory variable-speed patents from the same year. So a pitch-preserving method was both technically available and of commercial interest to both companies before the RR-DR60 (service manual © 1998). This makes a pitch-preserving implementation plausible. **It is not evidence that the MN1931712BB firmware does it.**
- Panasonic kept the same user-facing design in 2005: a SPEED CONTROL switch, five steps and normal in the middle. If the RR-DR60 is the direct ancestor, its ratios may resemble 0.5–2×, but that is speculation. A pitch-corrected 2× on a 4 kbit/s CELP decoder at 6 kHz would also have been demanding for the RR-DR60's DSP.

### Gaps
- I could not determine when Panasonic first marketed 早聞き/ゆっくり再生 with natural pitch, or whether any 1990s Panasonic IC recorder was described as natural-pitch. J-PlatPat was not searched directly.
- The full texts of JP2955247B2 and JPH08147874A were not read, and Sanyo, Sharp and NEC patents were not searched.

## 4. How contemporary recorders did it (Olympus, Sony)

### Takeaway
No 1995–2000 review or manual found states whether Olympus D1000/D1500/DS-150 or Sony ICD-series speed control changed pitch. Sources for later models (≥ 2005) say speed change was digital and pitch-preserving, under names such as Sony's "DPC" (Digital Pitch Control).

### Cited Findings
- [NEGATIVE] Route Fifty's 1998 review of the Olympus D1000 covers the flash card, voice recognition and capacity ("2M flash memory card recorded at most 30 minutes in long-play mode") but not playback speed. — [Route Fifty, 1998](https://www.route-fifty.com/digital-government/1998/11/olympus-digital-recorder-a-good-idea-that-gets-lost-in-translation/291789/)
- Olympus D1000 recording modes, from a spec listing [SECONDARY]: "Standard mode: 12kHz Long mode: 8kHz". It says nothing on speed control. — [GeneralManual D1000](https://www.generalmanual.com/Recorders/Olympus-D1000-Digital-Voice-Recorder.htm)
- **[LATER]** Olympus WS-852: "the speed change is processed digitally, thus the pitch is not altered", ×0.5 … ×2. — [Olympus Asia WS-852](https://asia.olympus-imaging.com/product/audio/ws852/feature2.html)
- **[LATER]** Sony DPC: changes "the playback speed without changing the pitch", 0.50–2.00× on some models and 0.25–3.00× on the ICD-PX470. A 2009 Sony press release markets DPC for language study. — [Sony Thailand support](https://www.sony.co.th/en/electronics/support/digital-voice-recorders-icd-series/articles/S500019524); [ICD-PX470 manual p. 73](https://www.manualslib.com/manual/1357038/Sony-Icd-Px470.html?page=73); [Sony press 2009-09](https://www.sony.jp/CorporateCruise/Press/200909/09-0901/)
- **[LATER]** A Japanese blog notes that on some recorders "速度を速くすると音程も高くなってしまう" ("raising the speed also raises the pitch"), and advises buyers to check the pitch statement. So pitch-changing speed control existed on some IC recorders, but no model or year is given. — [world-topics.com](https://www.world-topics.com/archives/hayakiki/ic-recorder/)
- In OKI's ADPCM speech-LSI family (MSM5205/MSM6388 class), the stored data carries no rate and the playback sampling clock is selectable, so changing the clock changes speed and pitch together. This is the low-cost pitch-changing route. The source is a search summary; the OKI 1987 Voice Synthesis LSI Data Book is the primary source to check. — [OKI 1987 Voice Synthesis LSI Data Book (bitsavers)](http://bitsavers.trailing-edge.com/components/oki/_dataBooks/1987_OKI_Voice_Synthesis_LSI_Data_Book.pdf) (not read)

### Inferences
- In the 1990s two approaches coexisted. Inexpensive ADPCM chip recorders changed the clock, which changes pitch. DSP-based coders could time-scale, and patents of 1994–96 from OKI, Lucent and Sony show that. Since the RR-DR60 has a programmable DSP running CELP, either is possible.

### Gaps
- I did not obtain the manuals for the Olympus D1000/D1500/DS-150 or the 1990s Sony ICD-50/ICD-70/ICD-90. Whether they had variable speed at all, and of which kind, is unknown.

## 5. Hardware constraints and candidate mechanisms (inference-heavy)

### Takeaway
A clock-based (pitch-changing) SLOW would drive the codec below its recommended 6.0 kHz minimum sync. FAST by clock would stay within range up to 10 kHz (1.67×). Designers who planned clock-based slowing would more likely have set "normal" mid-range, so the 6 kHz choice weakly favors DSP-domain slowing. Three mechanisms are plausible:
- **(a) codec clock change**: pitch and bandwidth scale with the ratio.
- **(b) DSP resampling at a fixed 6 kHz clock**: also pitch-changing.
- **(c) time-scale modification in or after the CELP decoder**: pitch-preserving, with characteristic pops, buzz or echo.

A hybrid (DSP-domain slowing with clock-based fast steps) is also possible.

### Cited Findings
- The codec family datasheet (MSM7717, proxy for the MSM7702) gives the sync frequency as "8 kHz ± 50 ppm to guarantee the AC characteristics". The device "can operate in the range of 8 kHz ± 2 kHz, but the electrical characteristics … are not guaranteed"; recommended sync is 6.0 min / 8.0 typ / 10 kHz max. — project `docs/hardware/sources.md` S-005 row ([mirror PDF](http://igorkov.org/revo/datasheets/M7717.pdf))
- The RR-DR60 runs the sync at 6 kHz ("Sampling frequency; 6 kHz"), and the codec clocks come from the DSP's PLL. — S-002 row ([Internet Archive](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN))
- Artifacts of excitation-domain pitch-cycle deletion or repetition: pops/clicks, buzzy/noisy speech, and "annoying" quality beyond 1.5× slowing — [US5717823A](https://patents.google.com/patent/US5717823A/en). Its disturbances are reduced by post-filtering — [EP0714089B1](https://patents.google.com/patent/EP0714089B1/en).
- Artifacts of parameter-domain stretching (reusing or interpolating decoder parameters): "unrealistic" sound with many identical parameters, and "foreign noise" from abrupt LPC changes. — [US5873059A](https://patents.google.com/patent/US5873059A/en)

### Inferences

**Arithmetic on the clock ratios** (my own):
- 8 MHz / 6 kHz = 1333.3 and 500 kHz / 6 kHz = 83.3, so 6 kHz is not an integer division of either the resonator or the marked BCLK. That fits S-002's note that a PLL makes the codec clocks. A PLL can in principle be set to other rates, so the hardware does not rule out clock-based speed change. (The T ≈ 2 µs and 170 µs marks are approximate, which weakens this.)
- With "normal" at 6.0 kHz (the recommended minimum):
  - Clock-based FAST steps up to 10 kHz could reach 1.67×, e.g. 1.25× = 7.5 kHz or 1.5× = 9 kHz.
  - Clock-based SLOW steps would need 4.5–5.4 kHz for 0.75–0.9×, below the recommended range. That is outside the datasheet's operating range, not only outside the guaranteed one, and the PLL might fail to lock.
  - A 0.5× step (3 kHz) would be far outside it.

**Mechanism (a), codec clock change.**
- Everything scales by the ratio r: F0, formants, the codec's switched-capacitor band edges and noise spectra.
- At 6 kHz the stored content sits in about 225–2550 Hz. At r = 1.5 it plays at about 340–3830 Hz, a thinner, higher "chipmunk" voice. At r = 0.75 it plays at about 170–1910 Hz, deeper and muffled.
- The receive filter scales with the clock, so no aliasing is introduced. Outside 8 kHz ± 2 kHz, filter accuracy and S/D are unspecified.
- The decoder must run r× real-time, and the DSP and codec interface must stay synchronized to the new rate.

**Mechanism (b), DSP resampling at a fixed 6 kHz codec rate.**
- The audible pitch and formant shifts are the same as (a).
- The DSP adds interpolation (polyphase cost of a few to a few tens of MACs per output sample) and decodes r× real-time. The codec stays within range.
- For r > 1, decoded content above 3000 Hz ÷ r must be low-passed before resampling, or it aliases. The shifted band can exceed Nyquist (2550 Hz × 1.5 = 3825 Hz > 3000 Hz), so either an anti-alias filter removes it (the top of the band is lost) or it folds back. This is a distinguishing artifact versus (a), where the whole band moves up intact.

**Mechanism (c), time-scale modification (pitch preserved).** Options from cheapest to costliest:
- **(c1) Pitch-cycle repetition or deletion in the excitation domain, using the decoder's own adaptive-codebook lag.**
  - This is the OKI and Lucent method. It needs no search, because the lag is in the bitstream.
  - The cost is buffer handling plus the normal LPC synthesis of r⁻¹× more (or fewer) output samples, about 10 MAC per sample for an order-10 filter, plus the postfilter.
  - Artifacts: on voiced speech, slight F0 "stepping" and occasional clicks at splice points. On unvoiced or noisy segments with no true period, repeated segments sound metallic or "buzzy" and give a flutter/echo texture. Plosives can double or smear.
  - Speed resolution is quantized to whole pitch periods per frame, so the realized ratio fluctuates frame to frame around the nominal value.
- **(c2) Stretching frame or subframe length at decode time.**
  - The adaptive codebook continues with the same lag, so pitch is preserved, while fixed-codebook pulses are spread over a longer subframe and LSPs are interpolated over a longer time.
  - Artifacts: consonants and transitions are lengthened uniformly. The excitation is thinned as pulses per ms fall, which sounds buzzier on slow steps and denser and noisier on fast ones. Repeated parameters sound "unrealistic" (Sony's word).
  - The cost is close to that of normal decoding.
- **(c3) SOLA/WSOLA on the decoded 6 kHz output.**
  - The cost is dominated by the similarity search. A ±90-sample search (15 ms max pitch period) over a ~120-sample window is about 10.8 k MAC per segment, roughly 1 M MAC/s at 100 segments per second, with no wrap-around concerns at 6 kHz. That was feasible in a 1990s 16-bit DSP alongside a CELP decoder, but it is not free.
  - Artifacts: a reverberant, phasey "doubling" on transients; slight tonal colouring.
  - (The operation counts are my own estimates, not sourced.)

**What a test would show.** A steady tone or sustained vowel recorded and played at levels 1–5 gives the answer directly:
- Duration changes alone mean (c).
- F0 and spectral centroid that scale with 1/duration mean (a) or (b).
- Bandwidth scaling that tracks the codec band edges separates (a) from (b).

**For the emulator.** Until evidence arrives, an assumption entry would be needed to cover:
- mechanism (pitch-preserving vs. pitch-shifting, possibly per side);
- the five ratios (unknown; 2005 Panasonic models used 0.5–2×, **[LATER]**);
- that the speed control is a playback-path option (stage 9/10, downstream of memory), selectable only during playback.

### Gaps
- No captures, measurements or firmware evidence exist for the five ratios or the mechanism.
- Whether S6 is read by IC4 or IC2, and whether the PLL is reprogrammed during playback (the sync frequency could be probed on PCB test points at each level), is for the schematic researcher or a unit owner.
- The MSM7702's own datasheet pages (S-003 lead) may state a different sync range than the MSM7717 proxy.
