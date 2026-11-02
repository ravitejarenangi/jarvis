# JARVIS — Design Document

Date: 2026-11-03
Status: Draft, awaiting owner approval

---

## 1. Intent

Build a real-life JARVIS, the AI from *Iron Man*: a voice-first assistant you hold
a natural spoken conversation with. It lives in a full-screen holographic HUD,
controls the computer, runs coding agents, connects to your life (mail, calendar,
messages, home), and has a "God's-eye view": a live intelligence globe plus a
full OSINT suite covering everything from cyber recon to competitor analysis.

It should **look and feel like the film**, not like a chatbot with a voice
bolted on. The magic comes from three things: speed (it answers almost
instantly), presence (a HUD and orb that react to speech), and competence (it
does things, not just talks about them).

The owner is not a Rust developer. Claude builds; the owner tests and steers.

## 2. Goals

1. **Natural conversation.** Always-in-conversation mode: once a session
   starts, JARVIS keeps listening, you can interrupt it mid-sentence (barge-in),
   and it replies in a low-latency streaming voice.
2. **Film-grade visuals.** A full-screen holographic HUD is the home screen;
   every app launches from it. When another app takes focus, JARVIS shrinks to a
   floating **arc-reactor orb** that pulses with its voice, and it expands back
   into the HUD on command.
3. **Pluggable brains.** Many LLM providers, switchable in settings, in the
   spirit of Hermes Agent: Anthropic, OpenAI, Google Gemini, OpenRouter, xAI,
   Groq, DeepSeek, Mistral, Sarvam, Ollama, LM Studio, and any OpenAI-compatible
   endpoint. You can choose a model per task (fast model for chit-chat, strong
   model for reasoning and tools).
4. **Pluggable voices.** Text-to-speech and speech-to-text providers are switchable:
   ElevenLabs (streaming), **VoiceStudio** (https://github.com/debpalash/VoiceStudio,
   local, via its local API/MCP), OpenAI, Sarvam (Indian languages), and local
   engines. Voice cloning is allowed only for voices the owner has rights to.
5. **Languages.** English plus Indian languages (Telugu, Hindi at minimum),
   including code-mixed speech (e.g. Telugu–English in one sentence).
6. **Customizable persona.** Ships as film JARVIS (calm, dry British wit,
   addresses the owner as "sir", concise). The name, wake phrase, voice,
   personality prompt and form of address are all editable (e.g. FRIDAY).
7. **Computer control.** Launch and switch apps, run shell commands, read and
   write files, click and type, manage windows. Anything **risky** (delete,
   send, pay, post, install, run a scan, run an unknown command) requires spoken
   or tapped confirmation. Everything else runs freely.
8. **Long-term memory.** Remembers everything (conversations, preferences, people,
   projects), stored **locally and encrypted**. Recalls relevant memories
   automatically. The owner can view, edit and delete any memory, and can say
   "forget that".
9. **Coding agents.** Drive almost all agentic coding harnesses (Claude Code,
   Codex CLI, Gemini CLI, OpenCode, Aider, Cursor CLI, Hermes, Goose, and
   others), preferring the Agent Client Protocol (ACP) and falling back to each
   tool's non-interactive CLI mode. Progress streams into a HUD panel.
10. **Connections.** Email + calendar (Gmail/Google Calendar, Microsoft 365, IMAP/CalDAV),
    messaging (WhatsApp, Telegram, Slack, Discord), smart home (Home Assistant,
    which also bridges HomeKit/Matter), web browsing and automation, GitHub.
    JARVIS acts as an **MCP client**, so any MCP server adds new powers, and as
    an **MCP server**, so other agents can use JARVIS.
11. **Phone remote.** iPhone + Android app that connects securely to the
    desktop core from anywhere: talk to JARVIS, see its HUD panels, approve
    risky actions, get notifications.
12. **God's-eye globe.** A **full Rust port** of God's Eye View
    (https://github.com/bilawalsidhu/gods-eye-view, MIT): live aircraft, ships,
    satellites, earthquakes, fires, weather, traffic, public cameras, launches,
    on a 3D globe, including the photorealistic 3D-tiles mode, all voice-controllable.
13. **OSINT suite.** Covers everything:
    - Cyber intel: subdomains, DNS, ports, services, tech stack, certificates,
      known vulnerabilities, leaked-credential exposure, threat feeds.
    - Company and competitor intel: news, filings, hiring, products, pricing,
      web changes, social mentions, tech stack.
    - Topic and event tracking: monitored keywords and regions, alerts.
    - People lookups: public profiles and username presence across sites.
    - Findings link together on a graph (entities and relationships), are tied
      to **cases**, and export as reports.
14. **Desktop platforms.** macOS first; Windows and Linux are also required.

## 3. Non-goals (for now)

- Running the full JARVIS core on the phone. The phone is a remote only.
- A cloud-hosted JARVIS server. The core runs on the owner's computer.
- Multi-user/team mode in v1. The OSINT data model must allow cases, users
  and shared reports to be added later without a rewrite, but only the
  single-owner flow gets built now.
- Training our own AI models.
- Cloning a real person's voice (e.g. Paul Bettany) without their consent.

## 4. Anti-goals (failure even if it "works")

- **Lag.** If JARVIS takes 2–3 seconds to start replying, the illusion dies.
  Target: first spoken audio within **~800 ms** of the owner finishing a
  sentence when a cloud voice is used, ~1.2 s with fully local voice.
- **Talking over itself or hearing itself.** In always-listening mode on
  speakers, JARVIS must not react to its own voice (echo cancellation is
  mandatory).
- **Acting destructively on a mishearing.** No delete, send, pay, post or scan
  without explicit confirmation.
- **Leaking private data.** Memory, keys, OSINT cases and audio stay local
  and encrypted; nothing is sent anywhere except the chosen AI or voice provider
  for the request at hand.
- **Illegal or unauthorized recon.** Active scanning (port scans, vuln scans,
  brute-forcing) is only allowed against targets on an explicit authorized-scope
  list; every OSINT action is logged.
- **Cheesy UI.** It must look like a film HUD, not a game-jam mockup or a
  generic dashboard.

## 5. Constraints

- **Language:** every line JARVIS's own codebase contains is Rust: core, UI,
  phone app, globe, OSINT. Rust crates that wrap proven C/C++ libraries
  (e.g. WebRTC echo cancellation, whisper.cpp, ONNX Runtime) and OS frameworks
  (Core Audio, AVFoundation, etc.) are allowed, as are the minimal platform
  shims an OS forces (e.g. Android service/manifest glue, iOS entitlements),
  each kept as small as possible. External programs JARVIS *talks to*
  (VoiceStudio is Python; coding agents are Node/Go; Home Assistant is Python)
  are fine because they are separate apps reached over APIs/protocols.
  VoiceStudio is AGPL-3.0, so JARVIS must only talk to it over its API and
  never copy its code.
- **Hardware (owner's main machine):** Mac mini, Apple M4, 10 cores, 16 GB RAM,
  1080p display, macOS 27. Always plugged in, so it doubles as the always-on home
  base for the phone. 16 GB is tight for running a local LLM, local STT/TTS,
  VoiceStudio and the Bevy globe at once, so cloud providers are the default and
  local models are small fallbacks. Windows/Linux minimum: 16 GB RAM, a GPU with
  Vulkan/DX12 support.
- **Open source:** public on GitHub (`ravitejarenangi/jarvis`), dual-licensed MIT OR Apache-2.0. Dependencies must be permissive (no GPL/AGPL) so App Store release stays possible. Secrets never enter the repo.
- **Distribution:** public release.
  - iOS App Store and Google Play: the phone app is a *remote*, so all OSINT,
    scanning and computer control runs on the desktop, which keeps
    store review risk down. Needs privacy disclosures and Apple/Google
    developer accounts.
  - macOS: Developer ID–signed and notarized direct download, **not** the Mac App
    Store (its sandbox forbids shell commands, accessibility control and
    controlling other apps, which JARVIS needs).
  - Windows: signed installer. Linux: AppImage/Flatpak.
- **Data licensing:** start with free/personal-use data sources, but every
  data source (globe feeds, OSINT APIs, map tiles) sits behind a switchable
  source setting with its license recorded, so commercial-licensed sources can
  replace them before any paid client work. Sources marked non-commercial
  are flagged in the HUD.
- **UI technology:** a Rust GPU engine, **Bevy** (on wgpu), for the HUD, orb,
  globe and phone app (owner's choice: "Rust + GPU engine").
- **Budget:** no real limit on paid services. Use the best provider for each
  job, but every paid service must have a free/local fallback configured, so
  JARVIS still works offline or if a key lapses.
- **Platforms:** macOS (Apple Silicon) first, then Windows and Linux; iOS and
  Android for the phone remote.
- **Toolchain:** Rust is not installed yet on the owner's Mac; setup is step 0.

## 6. Decisions the owner made

| Topic | Decision |
|---|---|
| Platforms | Desktop (macOS + Windows + Linux) + phone (iOS + Android) |
| Phone role | Remote to the desktop core, works away from home |
| AI brain | Many providers, switchable, like Hermes Agent |
| Talking | Always-in-conversation mode |
| Voice | Pluggable, including ElevenLabs and open-source VoiceStudio |
| Languages | English + Indian languages |
| Persona | Customizable, film JARVIS by default |
| UI | Rust GPU engine; full-screen HUD launches everything; shrinks to arc-reactor orb when other apps are in use |
| Computer control | Full, with confirmation for risky actions |
| Memory | Everything, stored locally |
| Connections | Email + calendar, smart home, messaging, web + coding, and nearly all coding-agent harnesses |
| God's Eye | Full Rust port of gods-eye-view, including photoreal mode |
| OSINT scope | Everything: cyber, company/competitor, topic, people, live world map |
| OSINT users | Owner now (personal + authorized client work); team later |
| Builder | Claude builds, owner reviews |
| Budget | No real limit |
| First milestone | Talking JARVIS + HUD |
| Rust rule | All our code is Rust; native C/C++ libs, OS frameworks and minimal platform shims allowed underneath |
| Phone link | End-to-end encrypted public relay + Apple/Google push (they only see ciphertext) |
| Commercial use | Start personal-use data sources; keep every source swappable for commercial-licensed ones later |
| Distribution | Public: App Store + Play Store for the phone; signed direct download for desktop |

## 7. How it fits together

```
                ┌────────────────── Desktop app (one Rust binary) ──────────────────┐
 mic ──► Audio in (echo cancel, VAD, wake/turn detection)                          │
                │                                                                  │
                ▼                                                                  │
          Speech-to-text (pluggable) ──► Conversation brain ◄──► Memory (encrypted)│
                                          │   (LLM router,     │                   │
                                          │    persona, tools) │                   │
                                          ▼                    ▼                   │
                                    Tool layer ──► OS control · coding agents (ACP)│
                                          │        · MCP servers · integrations    │
                                          │        · God's-eye feeds · OSINT       │
                                          ▼                                        │
 speakers ◄── Text-to-speech (pluggable, streaming)                               │
                │                                                                  │
          HUD / Orb / Globe (Bevy) ◄── event bus ──► Phone gateway (P2P, encrypted)│
                └──────────────────────────────────────────────────────────────────┘
                                                              ▲
                                                  iPhone / Android remote (Bevy)
```

### 7.1 Parts

1. **Audio pipeline.** Captures the mic, cancels echo from JARVIS's own voice,
   detects speech start/stop (voice activity detection), and detects end of turn
   (the owner finished speaking vs. just paused). Barge-in: when the owner starts
   speaking, JARVIS's speech stops within ~150 ms.
2. **Speech-to-text (pluggable).** Streaming cloud options (e.g. Deepgram,
   ElevenLabs Scribe, Sarvam for Indian languages, OpenAI) and a local option
   (Whisper-family models). Language is auto-detected; code-mixed speech
   is supported where the provider handles it.
3. **Conversation brain.** Holds the session, builds the prompt (persona +
   relevant memories + live context like time, active app and calendar), and
   streams the reply sentence by sentence into text-to-speech so speech starts
   before the full answer is written. Routes between models: a fast model for
   quick replies, a strong model for tools and reasoning.
4. **LLM router (pluggable providers).** One internal interface; adapters for
   each provider family (Anthropic, OpenAI-compatible, Gemini, Ollama).
   Configured in a settings file and in the HUD settings panel. Supports tool
   calling, streaming, and per-task model selection. Optional speech-to-speech
   realtime models (OpenAI Realtime, Gemini Live) as an alternate mode.
5. **Text-to-speech (pluggable).** ElevenLabs streaming (WebSocket), VoiceStudio
   (local API), OpenAI, Sarvam, plus a bundled local engine as the offline
   fallback. Emits audio levels so the orb/HUD visualizer animates with the voice.
6. **Tool layer.** Every capability is a tool with a name, description, input
   schema, and a **risk level** (safe / confirm / forbidden). The confirm gate is
   enforced here, so no individual tool can bypass it. All tool calls are logged.
7. **OS control.** Launch/quit/switch apps, window management, shell commands,
   files, clipboard, keyboard/mouse, notifications, volume/brightness, media.
   One implementation per OS (macOS, Windows, Linux).
8. **Coding-agent bridge.** Starts and talks to coding agents over ACP (or
   their CLI as a fallback), streams their output to a HUD panel, forwards their
   permission requests to the owner by voice ("Claude Code wants to run the
   migrations, sir. Proceed?").
9. **MCP.** Client: connect any MCP server (including VoiceStudio's).
   Server: expose JARVIS's tools to other agents.
10. **Integrations.** Email/calendar, messaging, Home Assistant, GitHub, web
    browsing/automation. Each is a group of tools with its own risk levels
    (reading mail = safe, sending = confirm).
11. **Memory.** Local encrypted database: conversation log, extracted facts
    (people, preferences, projects), and vector search for recall. A HUD panel
    lets the owner browse, edit and delete memories.
12. **HUD, orb and globe (Bevy).** Described in §8.
13. **Phone gateway.** Encrypted peer-to-peer link to the phone that works
    across networks without port forwarding, with device pairing by QR code.
    Streams audio both ways, HUD panel state, notifications and confirmation
    prompts.
14. **God's-eye module.** Described in §9.
15. **OSINT module.** Described in §10.

## 8. The HUD experience

- **HUD mode (home).** Full-screen, transparent-feeling holographic interface:
  central arc-reactor/voice visualizer, an app launcher ring, live panels
  (time, weather, calendar, system stats, notifications, active tasks/agents),
  the globe, and result panels that JARVIS opens while talking ("Here are the
  flights over Hyderabad, sir"). Every app is launched from here, by voice or click.
- **Orb mode.** When the owner switches to another app, the HUD collapses into a
  small always-on-top arc-reactor orb (draggable, click-through when idle). It
  glows and pulses with listening and speaking. Saying the name, clicking the orb,
  or a hotkey brings the HUD back.
- **Visual language.** Cyan/amber holographic palette, thin-line geometry,
  glow/bloom, scanlines, subtle parallax, animated transitions where panels
  "assemble" in. Owner reviews look-and-feel early via screenshots/recordings.
- **Accessibility basics.** Every voice action also has a click/keyboard path;
  text captions of what JARVIS says; animation intensity is adjustable.
- **Settings panel.** Providers and keys, voice, persona, languages,
  wake behavior, risk rules, authorized OSINT scope, memory browser.

## 9. God's-eye globe (full Rust port)

Port God's Eye View's features to Rust/Bevy, layer by layer, using its source
and `DATA_SOURCES.md` as the reference. Code is MIT, so porting is allowed with
attribution; bundled datasets keep their own licenses (e.g. TeleGeography
cables are non-commercial and must be excluded or licensed if JARVIS is ever
used commercially).

Order inside this module:
1. Globe rendering: a stylized holographic globe (the Iron Man look) with
   satellite imagery and terrain.
2. Live feeds ported as Rust data services: aircraft, military aircraft, ships,
   satellites (orbital propagation), earthquakes, fires, weather/radar, launches,
   public cameras, traffic.
3. Interactions: click-to-track, contacts roster, trails, follow camera,
   search/fly-to, sensor looks (FLIR/NVG/CRT), detection overlay, scene director,
   share links.
4. **Photorealistic 3D tiles** (Google Photorealistic 3D Tiles / Cesium ion).
   This is the hardest part: no mature Rust 3D Tiles streaming renderer
   exists, so we build one (tile streaming, level-of-detail, glTF decoding, caching).
   It is scheduled last within this module so everything else ships first.
5. Voice control of everything through JARVIS's tool layer, replacing the
   original's built-in agent.

## 10. OSINT suite

- **Workspace model.** Every lookup belongs to a **case**. Findings are
  entities (domain, IP, company, person, username, email, location, vessel,
  aircraft…) and relationships, stored locally and shown as a graph in the HUD
  and on the globe where they have coordinates. The data model includes owner,
  case and user IDs so team mode can be added later.
- **Passive vs. active.** Passive collection (public records, search, DNS,
  certificate logs, news, social, APIs) is allowed on any target. **Active**
  collection (port scanning, vuln scanning, crawling/fuzzing a target)
  requires the target to be on the **authorized-scope list**, plus confirmation.
- **Modules:**
  - Cyber: subdomain enumeration, DNS, certificate transparency, port/service
    scan, tech fingerprinting, vulnerability templates, Shodan/Censys/
    VirusTotal/breach-exposure lookups, threat feeds.
  - Company/competitor: news (GDELT and news APIs), filings (e.g. SEC EDGAR,
    MCA India), hiring signals, website change monitoring, product/pricing
    tracking, social mentions, tech stack.
  - Topic/event monitoring: saved watches with alerts to the HUD and phone.
  - People: username presence across sites, public profiles, email/phone
    footprint from public sources. Every people lookup records a stated purpose
    in the audit log. Respect privacy law (India DPDP Act, GDPR where
    relevant); no stalking/doxxing use.
- **Reports.** Export a case as Markdown/PDF with sources and timestamps.
- **Tools in Rust.** Prefer Rust implementations or crates; where the best tool
  is non-Rust (e.g. some Go recon tools), the default is a native Rust
  re-implementation. Wrapping an external binary is a fallback that needs
  the owner's sign-off per tool.

## 11. Security and privacy

- API keys in the OS keychain (macOS Keychain / Windows Credential Manager /
  Linux Secret Service), never in plain files.
- Memory, OSINT cases and logs encrypted at rest.
- Phone pairing by QR code; all phone traffic end-to-end encrypted; a lost
  phone can be revoked from the HUD. Traffic may pass through a public relay
  (for NAT traversal) and Apple/Google push services (to wake the phone), but
  these only ever see encrypted bytes. Push payloads contain no readable content.
- Audit log of every tool call and OSINT action, viewable in the HUD.
- Always-in-conversation sessions show a clear "listening" indicator; a
  hotkey/phrase mutes instantly.

## 12. Build order

Each phase ends with something the owner can run and judge.

| Phase | Delivers |
|---|---|
| 0. Setup | Install Rust toolchain, repo, workspace skeleton, CI |
| 1. **Talking JARVIS + HUD (first milestone)** | Audio pipeline with echo cancellation and barge-in, pluggable STT/LLM/TTS (ElevenLabs + VoiceStudio + one local fallback each), persona, HUD + orb mode, app launching by voice, basic memory, settings panel, macOS |
| 2. Hands | Tool layer with risk gate, full OS control, MCP client/server, web browsing, coding-agent bridge (ACP) |
| 3. Life | Email/calendar, messaging, Home Assistant, GitHub, morning briefing, proactive notifications |
| 4. Phone | iOS + Android remote, pairing, voice from phone, approvals, notifications |
| 5. God's-eye | Rust globe and live feeds → interactions → photoreal 3D tiles |
| 6. OSINT | Cases/graph, cyber, company/competitor, topic monitoring, people, reports |
| 7. Reach | Windows + Linux parity, Indian-language polish (Telugu/Hindi code-mixing), performance |

Phases 1–2 must run on macOS; Windows/Linux differences are kept behind the
OS-control boundary from day one so Phase 7 is porting, not rewriting.

## 13. Success looks like

- The owner says "Jarvis", has a five-minute back-and-forth conversation with
  interruptions, and it feels like talking to a person, not a phone tree.
- "Jarvis, open VS Code and have Claude Code fix the failing test": the app opens,
  the agent runs in a HUD panel, and JARVIS asks before risky steps.
- "Show me every aircraft near Hyderabad": the globe flies there and tracks them.
- "Run recon on example.com" (in scope): a case fills with subdomains, services
  and exposures, and a report is ready.
- From the phone, away from home: "Turn off the AC and read my last three emails."

## 14. Left to the builder

- Specific crates, versions, and internal module layout.
- Specific default models per provider and the fast/strong routing rule.
- Choice of local STT/TTS/VAD/turn-detection models (must support English +
  Telugu/Hindi where feasible).
- P2P/networking library for the phone link.
- Database and vector-search engine for memory.
- Exact visual design of the HUD (owner reviews by screenshots/recordings).
- Which messaging platforms use official APIs versus bridges, within platform
  terms of service.

## 15. Calls the designer made (owner: please check these)

These were not decided by the owner; they are sensible defaults that can be changed.

1. **Linux display servers:** full support on X11; Wayland is best-effort, with
   per-desktop workarounds (GNOME/KDE portals) for the orb's always-on-top,
   global hotkeys and simulated input. Some features may be limited on Wayland.
2. **Memory stores transcripts, not raw audio.** Raw audio is discarded after
   transcription unless the owner turns on audio retention. "Forget that"
   deletes the transcript, extracted facts and their search index entries.
3. **Desktop availability for the phone:** a "stay awake for the phone"
   setting (on by default, since the Mac mini is always plugged in).
   Wake-on-LAN is deferred.
4. **Mac App Store is skipped** for the desktop (see Distribution).
5. **Cloud-first, local-fallback** for STT/LLM/TTS because of 16 GB RAM.
6. **Phone store compliance:** the phone app exposes OSINT/recon only as
   viewing results and approving actions that run on the desktop.
7. **OSINT tools:** native Rust re-implementations by default; wrapping
   an external binary needs owner sign-off per tool (§10).
