# Changelog

This is the curated public development history of Keith Agent. It is generated from the polished Git history; internal analysis, local execution traces, private paths, and unpublished working notes are intentionally excluded.

The original `spec/keith-agent` delivery is traceable through explicit task and wave markers for every completed wave from 1 through 23. Waves 24 through 30 remain pending in the specification and are not represented as completed work.

## Unreleased

- Configure authenticated multi-architecture GHCR publication with protected credentials.
- Pin the Ed25519 release trust key and stop publication when the signing seed, protected public-key variable, and packaged key disagree.

## 2026-09-02

- [`dbd2fb187920`](https://github.com/Sidiora-Labs/keith-agent/commit/dbd2fb1879201100eae7915eee9c9e7d7241984c) Finalize the Sidiora Labs public repository boundary
- [`879183f5c97e`](https://github.com/Sidiora-Labs/keith-agent/commit/879183f5c97e3676b3b91a047da3a6d28cc4cb88) Publish agent context and remove editor-specific configuration
- [`481fdd453d83`](https://github.com/Sidiora-Labs/keith-agent/commit/481fdd453d83bdc9f6e3292dc892460169641804) Document Keith's supported feature surface
- [`7a73f12ef184`](https://github.com/Sidiora-Labs/keith-agent/commit/7a73f12ef184f269b576f2fba4e0f27466711715) Publish community, security, and operator documentation
- [`4c8fcb66b634`](https://github.com/Sidiora-Labs/keith-agent/commit/4c8fcb66b634aef4b20b654867ca5e4486d2a4d9) Add multi-cloud deployment and release automation
- [`fc31fdabb9a3`](https://github.com/Sidiora-Labs/keith-agent/commit/fc31fdabb9a3438540455c71129d55356674d89c) Harden the authority matrix and release packaging
- [`0d4572341eda`](https://github.com/Sidiora-Labs/keith-agent/commit/0d4572341eda44987f99e75ca791c5e0b83d8024) Qualify cross-surface runtime and recovery boundaries

## 2026-09-01

- [`09f66c5ef40c`](https://github.com/Sidiora-Labs/keith-agent/commit/09f66c5ef40c0fdbb6fbd32e136c78d3abaa38f7) Unify service orchestration across daemon, web, and terminal clients
- [`22d10867904e`](https://github.com/Sidiora-Labs/keith-agent/commit/22d10867904e9731a4035c2b9cb42fe6be4d6570) Connect supervised integrations and operator surfaces
- [`8b8d3f206c24`](https://github.com/Sidiora-Labs/keith-agent/commit/8b8d3f206c245b5638e47bf0bed0a66362dcfb04) Add channel, ACP, plugin, Composio, CUA, and teaching runtimes
- [`65e46bc5eee1`](https://github.com/Sidiora-Labs/keith-agent/commit/65e46bc5eee16de4df83283a3052623f8372bc66) Establish the Keith Everywhere platform contract
- [`77db7560cbb2`](https://github.com/Sidiora-Labs/keith-agent/commit/77db7560cbb2c0230973d893c0dd0a977a8e5338) Plan competitive parity remediation
- [`9d702a1f1fe6`](https://github.com/Sidiora-Labs/keith-agent/commit/9d702a1f1fe667b65ec953c48e05947ae29082c1) Add guarded self-evolution and release hardening

## 2026-08-22

- [`5497026a3a4e`](https://github.com/Sidiora-Labs/keith-agent/commit/5497026a3a4e95020a13414fd174a9a0e91a5cc3) Integrate the Next.js client and unified memory runtime [spec:keith-agent/7.13 wave:20]

## 2026-08-18

- [`ba0348a49eb0`](https://github.com/Sidiora-Labs/keith-agent/commit/ba0348a49eb02502ac4a9281e01f3d32cf411138) Connect Keith web conversation and recovery [spec:keith-personal-intelligence/3.3]
- [`950f1707b667`](https://github.com/Sidiora-Labs/keith-agent/commit/950f1707b6671475556d15afea27532144da0725) Qualify terminal behavior and restoration [spec:keith-personal-intelligence/2.3]
- [`716fdac21d3d`](https://github.com/Sidiora-Labs/keith-agent/commit/716fdac21d3d6f4ca172c9b2a455327e948688b1) Build Keith consumer personal intelligence shell [spec:keith-personal-intelligence/3.2]
- [`e16bc3a45a95`](https://github.com/Sidiora-Labs/keith-agent/commit/e16bc3a45a95459047c4f700f034c5ec47ad5934) Redesign the terminal around conversation [spec:keith-personal-intelligence/2.2]
- [`cdcb570548ec`](https://github.com/Sidiora-Labs/keith-agent/commit/cdcb570548eccee06fd684846634974d07d172a9) Build Keith personal intelligence web foundation [spec:keith-personal-intelligence/3.1]
- [`bceaca03512d`](https://github.com/Sidiora-Labs/keith-agent/commit/bceaca03512dc0c6d460a3f02ed5625d33cc446b) Stream TUI events continuously [spec:keith-personal-intelligence/2.1]
- [`ea44053d3d13`](https://github.com/Sidiora-Labs/keith-agent/commit/ea44053d3d133ac87d5b7d268ae3d5882d9bb004) Define Keith personal intelligence client projections [spec:keith-personal-intelligence/1.1]

## 2026-08-17

- [`fc906f3c0356`](https://github.com/Sidiora-Labs/keith-agent/commit/fc906f3c0356ec3d368ca8332164436a1edba5f4) Add durable first meeting and relationship continuity [spec:keith-agent/7.12 wave:19]
- [`dbb8b27de94d`](https://github.com/Sidiora-Labs/keith-agent/commit/dbb8b27de94dc40bca00967b4dad5a0800d55158) Give Keith a stable machine-native personality [spec:keith-agent/7.11 wave:18]
- [`631cbaae52b2`](https://github.com/Sidiora-Labs/keith-agent/commit/631cbaae52b27aafb194c6b4e65574c25af6bf8d) Add bounded evidence-grounded reflex memory activation [spec:keith-agent/7.10 wave:17]
- [`d12286dec9e5`](https://github.com/Sidiora-Labs/keith-agent/commit/d12286dec9e51fe6dd1e4146137541a851ad334c) Add bounded recursive memory scouts and recall capsules [spec:keith-agent/7.9 wave:16]
- [`0c757fb6939d`](https://github.com/Sidiora-Labs/keith-agent/commit/0c757fb6939df91456224cd8acca7ca8fca15aca) Add revision-bound MemoryWorld kernel access [spec:keith-agent/7.8 wave:15]
- [`35509359910e`](https://github.com/Sidiora-Labs/keith-agent/commit/35509359910e0ff93160cfedcf908623b2d4cb40) Correct memory atlas task graph scope [spec:keith-agent/7.7 wave:14]
- [`bc271d0e3061`](https://github.com/Sidiora-Labs/keith-agent/commit/bc271d0e306136b06fbc6c41bc51042471cc323c) Build authoritative memory evidence vault and atlas [spec:keith-agent/7.7 wave:14]
- [`c6df046b659a`](https://github.com/Sidiora-Labs/keith-agent/commit/c6df046b659a768e24beb7ed538a1a384e6fba9a) Implement durable compaction and terminal turn authority [spec:keith-agent/13.6 wave:23]
- [`47eab86bfd7b`](https://github.com/Sidiora-Labs/keith-agent/commit/47eab86bfd7b4f9424487744b94ff274a6aca009) Stream Keith lifecycle events end to end [spec:keith-agent/13.6 wave:23]

## 2026-08-16

- [`0e5d9722075f`](https://github.com/Sidiora-Labs/keith-agent/commit/0e5d9722075fe2fb586b04bc1a4f46030ff26ce7) Accept advisory OpenWebUI function catalogs [spec:keith-agent/13.4 wave:23]
- [`63d43b8ce815`](https://github.com/Sidiora-Labs/keith-agent/commit/63d43b8ce8151812dfd7e7a6d23c9fe11331bfd6) Wire persistent RLM bridge and recursive child runtime [spec:keith-agent/7.13 wave:20]
- [`f270c166e34f`](https://github.com/Sidiora-Labs/keith-agent/commit/f270c166e34f5c303a16f2ecfb7c10be6e26d0ab) Add OpenAI-compatible application interface [spec:keith-agent/13.4 wave:23]
- [`ceb349d87bf7`](https://github.com/Sidiora-Labs/keith-agent/commit/ceb349d87bf7587c934c2a338f29319c8adb4175) Finish signed release lifecycle qualification [spec:keith-agent/13.3 wave:23]
- [`679a13a79598`](https://github.com/Sidiora-Labs/keith-agent/commit/679a13a7959892bfaab812c16bfdc3755346d7ea) Qualify real worker execution and compaction recovery [spec:keith-agent/13.3 wave:23]
- [`ee3f59f06f66`](https://github.com/Sidiora-Labs/keith-agent/commit/ee3f59f06f66b598dfdafe6f4b80f4cb5dbfcdc4) Route active cancellation through the leased worker [spec:keith-agent/13.3 wave:23]
- [`4753807e946e`](https://github.com/Sidiora-Labs/keith-agent/commit/4753807e946e520daccaa3e278f161b268385509) Wire the complete headless runtime and signed release lifecycle [spec:keith-agent/13.3 wave:23]
- [`ede26c887c7a`](https://github.com/Sidiora-Labs/keith-agent/commit/ede26c887c7a407fb1b2bfa2f3eb12515c78a40d) Harden signed release and lifecycle verification groundwork [spec:keith-agent/13.3 wave:23]
- [`950d3043786c`](https://github.com/Sidiora-Labs/keith-agent/commit/950d3043786c387ec76e4cd99f8be2013b8019d7) Add the packaged web server and release entrypoint [spec:keith-agent/13.3 wave:23]
- [`dbab7d5e29fd`](https://github.com/Sidiora-Labs/keith-agent/commit/dbab7d5e29fdd6ca4638d1961551b119b6d838bf) Add native Linux macOS and Windows platform backends [spec:keith-agent/13.2 wave:22]
- [`72af4c24b35b`](https://github.com/Sidiora-Labs/keith-agent/commit/72af4c24b35bf2bcab7a1edf9dfce2411453f6f7) Add a release-blocking adversarial security gate [spec:keith-agent/12.5 wave:22]

## 2026-08-15

- [`29e61476f567`](https://github.com/Sidiora-Labs/keith-agent/commit/29e61476f56721677d93a9a6cf6f7ca42d55a2c7) Run the complete migration compatibility matrix [spec:keith-agent/13.1 wave:21]
- [`aa6e6e29d2d5`](https://github.com/Sidiora-Labs/keith-agent/commit/aa6e6e29d2d5376dd154864045ec587efcabea89) Add complete portable data control lifecycle [spec:keith-agent/12.3 wave:21]
- [`9a636bf8fb68`](https://github.com/Sidiora-Labs/keith-agent/commit/9a636bf8fb6879a33c327e319ad1ab7518541ef3) Reconcile interrupted work across durable boundaries [spec:keith-agent/12.2 wave:21]
- [`71b3497c9fb3`](https://github.com/Sidiora-Labs/keith-agent/commit/71b3497c9fb3b31c6b438c6133126131a1675730) Add privacy-preserving local observability [spec:keith-agent/12.4 wave:20]
- [`74cb76ad92cb`](https://github.com/Sidiora-Labs/keith-agent/commit/74cb76ad92cba279750599c5df175cc07f709cfb) Bound hierarchical load and cancellation [spec:keith-agent/12.1 wave:20]
- [`e3885d227ed8`](https://github.com/Sidiora-Labs/keith-agent/commit/e3885d227ed8ed304dcc98ed0b73e1d7ff71a804) Unify accessible web and TUI operator surfaces [spec:keith-agent/11.5 wave:20]
- [`8c699218501e`](https://github.com/Sidiora-Labs/keith-agent/commit/8c699218501e2f7950947a4560b654329d3a7212) Build the Rust desktop lifecycle shell [spec:keith-agent/11.4 wave:19]
- [`5771d6227f21`](https://github.com/Sidiora-Labs/keith-agent/commit/5771d6227f21f3039778840ea521cbb4c02e4bbd) Add validation-gated candidate skill synthesis [spec:keith-agent/10.4 wave:19]
- [`23280c8dcf2c`](https://github.com/Sidiora-Labs/keith-agent/commit/23280c8dcf2c67cef02d446853d033cbfc825896) Build the authenticated Rust web application [spec:keith-agent/11.3 wave:18]
- [`29d1b2576c36`](https://github.com/Sidiora-Labs/keith-agent/commit/29d1b2576c3686f091d6e28b24d9eb03e611ce6d) Build the Ratatui terminal client [spec:keith-agent/11.2 wave:18]
- [`4a40f66a5399`](https://github.com/Sidiora-Labs/keith-agent/commit/4a40f66a53998b1a49c5ddd646a83fcc0e2016ba) Add guarded reversible refinement transactions [spec:keith-agent/10.3 wave:18]
- [`7c5639e3a4c1`](https://github.com/Sidiora-Labs/keith-agent/commit/7c5639e3a4c1226423a97e7c638aa1c339186ef9) Add shared UI projections and generation-aware reducers [spec:keith-agent/11.1 wave:17]
- [`be63fbbc4d54`](https://github.com/Sidiora-Labs/keith-agent/commit/be63fbbc4d543c92614978109f9df140504a87c1) Add durable multi-profile channel routing [spec:keith-agent/9.4 wave:17]
- [`eefb78b4a018`](https://github.com/Sidiora-Labs/keith-agent/commit/eefb78b4a018dd1505e655e4672db118fbfd24d8) Add production Discord channel adapter [spec:keith-agent/9.3 wave:17]
- [`1b12bbb3b7d7`](https://github.com/Sidiora-Labs/keith-agent/commit/1b12bbb3b7d7a98f90ef08d39c318c2fee096ce8) Add transactional delivery outbox recovery [spec:keith-agent/9.2 wave:16]
- [`9cb492bd40a3`](https://github.com/Sidiora-Labs/keith-agent/commit/9cb492bd40a3a9de47d6941a0663a23250c2f380) Add truthful presence and progress projection [spec:keith-agent/8.5 wave:16]
- [`355f45117c33`](https://github.com/Sidiora-Labs/keith-agent/commit/355f45117c3310f44cb317980278f869743fd27a) Add bounded explainable attention policy [spec:keith-agent/8.4 wave:16]
- [`df8cb94f7b07`](https://github.com/Sidiora-Labs/keith-agent/commit/df8cb94f7b07268c97b219b75a9f33819e4f36ce) Add managed MCP lifecycle and schema projection [spec:keith-agent/10.2 wave:15]
- [`6b94b3cf2caa`](https://github.com/Sidiora-Labs/keith-agent/commit/6b94b3cf2caabfb37b5f756a30c3cb8a4743fedb) Add isolated Wasm plugin lifecycle [spec:keith-agent/10.1 wave:15]
- [`cb34438dfd13`](https://github.com/Sidiora-Labs/keith-agent/commit/cb34438dfd13813578a93a02e999893f303df46c) Add isolated channel gateway and ordered queues [spec:keith-agent/9.1 wave:15]
- [`6c6eb0589d17`](https://github.com/Sidiora-Labs/keith-agent/commit/6c6eb0589d172dc43f3e346d1d9ec51e3178a95b) Add durable bounded awareness projections [spec:keith-agent/8.3 wave:15]
- [`c83f1269ee15`](https://github.com/Sidiora-Labs/keith-agent/commit/c83f1269ee15a849444a3c20048f799424305658) Add durable commitments and event-driven waits [spec:keith-agent/8.2 wave:14]
- [`751c09633aab`](https://github.com/Sidiora-Labs/keith-agent/commit/751c09633aab5a9e9fd98773a6362fb0f904beca) Add declarative skill discovery and lifecycle [spec:keith-agent/7.6 wave:14]
- [`3caaf8e1e81e`](https://github.com/Sidiora-Labs/keith-agent/commit/3caaf8e1e81e7758fa319d3dc28cf728c45af762) Add transactional linked Markdown knowledge [spec:keith-agent/7.5 wave:14]
- [`409717cfd476`](https://github.com/Sidiora-Labs/keith-agent/commit/409717cfd47691c61174784a1e81d01b409ae17a) Add durable timezone-aware scheduling [spec:keith-agent/8.1 wave:13]
- [`2f32b9cdbf7a`](https://github.com/Sidiora-Labs/keith-agent/commit/2f32b9cdbf7aff54fc6dc6c21e9e47ebc6585db2) Add resilient hybrid retrieval [spec:keith-agent/7.4 wave:13]
- [`296f640527c9`](https://github.com/Sidiora-Labs/keith-agent/commit/296f640527c9a81394160e8fb1f0e8ad994924d9) Add policy-separated memory consolidation [spec:keith-agent/7.3 wave:13]
- [`2dc66a7558e0`](https://github.com/Sidiora-Labs/keith-agent/commit/2dc66a7558e02cda192068933cf4050353662a3d) Add versioned live personal workspaces [spec:keith-agent/7.2 wave:12]
- [`fec7b0239d9a`](https://github.com/Sidiora-Labs/keith-agent/commit/fec7b0239d9aa32efe40ca7053cfd3373a172c88) Add durable hierarchical resource governance [spec:keith-agent/6.3 wave:12]
- [`7100fcb9b5d7`](https://github.com/Sidiora-Labs/keith-agent/commit/7100fcb9b5d761121d4272ee12644d0c48e667e4) Add durable profile routing and session snapshots [spec:keith-agent/7.1 wave:11]
- [`a2491c5a0803`](https://github.com/Sidiora-Labs/keith-agent/commit/a2491c5a0803e433edaf97760e519d0cd832c61c) Add durable recursive child sessions [spec:keith-agent/6.2 wave:11]
- [`81a6dd0d8978`](https://github.com/Sidiora-Labs/keith-agent/commit/81a6dd0d89782b9a8e47e4564d77f65df95070f6) Add persistent isolated guest kernel broker [spec:keith-agent/5.6 wave:11]
- [`b69fabc443b4`](https://github.com/Sidiora-Labs/keith-agent/commit/b69fabc443b49ddc934505dcaab2a5d252bd9c52) Implement durable bounded autonomous goals [spec:keith-agent/6.1 wave:10]
- [`56310cc77fb3`](https://github.com/Sidiora-Labs/keith-agent/commit/56310cc77fb35210df25cf70a5fe40dafc29f250) Add safe web access and isolated browser profiles [spec:keith-agent/5.3 wave:10]
- [`6beb3742c534`](https://github.com/Sidiora-Labs/keith-agent/commit/6beb3742c534f5d8476a2eb5a5d9d7fd0b7caa79) Add bounded adaptive routing experience [spec:keith-agent/4.5 wave:10]
- [`e1ce0ec16407`](https://github.com/Sidiora-Labs/keith-agent/commit/e1ce0ec1640787b279e9e0b67da111b6f56303b4) Add durable scoped artifacts and bounded output spill [spec:keith-agent/5.5 wave:9]
- [`ab2c7c60dfc4`](https://github.com/Sidiora-Labs/keith-agent/commit/ab2c7c60dfc4ec1132cefd1dd0ce037318eab8da) Add encrypted scoped credentials and leak filtering [spec:keith-agent/5.4 wave:9]
- [`103367982d17`](https://github.com/Sidiora-Labs/keith-agent/commit/103367982d17228ff1e9533f133e35b89cd86c68) Add capability-rooted files and restricted process execution [spec:keith-agent/5.2 wave:9]
- [`ba5a059d736c`](https://github.com/Sidiora-Labs/keith-agent/commit/ba5a059d736cc485289b4db7cfd21447bd411100) Add deterministic checks and bounded review passes [spec:keith-agent/4.4 wave:9]
- [`44bf843a15ea`](https://github.com/Sidiora-Labs/keith-agent/commit/44bf843a15ea383f8f56bdb19b82b10556d05f25) Add durable plans and deterministic task routing [spec:keith-agent/4.3 wave:9]
- [`27ecb3ce8f5a`](https://github.com/Sidiora-Labs/keith-agent/commit/27ecb3ce8f5a7fd23b09f88134e0fced6b3514f9) Add typed tool discovery and execution management [spec:keith-agent/5.1 wave:8]
- [`6ae873468718`](https://github.com/Sidiora-Labs/keith-agent/commit/6ae873468718acb8d1b21500cc9b8bd86c41049a) Build the streaming tool-using agent loop [spec:keith-agent/4.2 wave:8]
- [`e9aca699cca3`](https://github.com/Sidiora-Labs/keith-agent/commit/e9aca699cca3f23fcb81068c96fbe528c91aa48f) Add normalized providers and profile model routing [spec:keith-agent/4.1 wave:7]
- [`ee07ef33ad1d`](https://github.com/Sidiora-Labs/keith-agent/commit/ee07ef33ad1d71c473f4c32d5e494486d23d71c8) Add branch-safe compaction and context rebuilding [spec:keith-agent/3.4 wave:7]
- [`d3dfa3bdc82c`](https://github.com/Sidiora-Labs/keith-agent/commit/d3dfa3bdc82c43a94e7286e22cf96766ba39ee4f) Implement append-only branched session storage [spec:keith-agent/3.3 wave:6]
- [`3a6e50959c8d`](https://github.com/Sidiora-Labs/keith-agent/commit/3a6e50959c8defdc5aaa2f3ac35f4c403837721c) Add the durable session action inbox and pump [spec:keith-agent/3.2 wave:6]
- [`edbc2b76cb02`](https://github.com/Sidiora-Labs/keith-agent/commit/edbc2b76cb024664dfcdc1cbf00dca636de89cc2) Implement durable AgentSession actor state machine [spec:keith-agent/3.1 wave:5]
- [`3f7f6c05e478`](https://github.com/Sidiora-Labs/keith-agent/commit/3f7f6c05e4781962ac636c18e810f331851ea644) Add bounded event replay and reconnect recovery [spec:keith-agent/2.4 wave:5]
- [`7223153fc7c6`](https://github.com/Sidiora-Labs/keith-agent/commit/7223153fc7c6769e53b85dc714ac76649a695fe2) Enforce leased worker ownership and private control [spec:keith-agent/2.3 wave:4]
- [`57de8771d4cc`](https://github.com/Sidiora-Labs/keith-agent/commit/57de8771d4ccfadd1834b155059be0f069b17526) Build lazy daemon catalog and worker supervisor [spec:keith-agent/2.2 wave:4]
- [`e9ecba38d59a`](https://github.com/Sidiora-Labs/keith-agent/commit/e9ecba38d59ad4a9fbfbeb6d2704415f00d6be8c) Define AgentConnection protocol and transport conformance [spec:keith-agent/2.1 wave:3]
- [`578a4388c01f`](https://github.com/Sidiora-Labs/keith-agent/commit/578a4388c01f2a44d96a0edf9a631490bcc010ba) Add transactional state repositories and SQLite recovery [spec:keith-agent/1.4 wave:2]
- [`b81c25eef174`](https://github.com/Sidiora-Labs/keith-agent/commit/b81c25eef1745e30d181c79414d99bff4ba03d06) Implement validated layered configuration and profiles [spec:keith-agent/1.3 wave:2]
- [`eb6734e47db1`](https://github.com/Sidiora-Labs/keith-agent/commit/eb6734e47db11522d829b2b5f31596841b03eb47) Add versioned common types and canonical schemas [spec:keith-agent/1.2 wave:1]
- [`b852367cb615`](https://github.com/Sidiora-Labs/keith-agent/commit/b852367cb615ca61d390063162b8f8c6a063dd0e) Establish the Keith Rust workspace and dependency guardrails [spec:keith-agent/1.1 wave:1]
- [`8768ff44006f`](https://github.com/Sidiora-Labs/keith-agent/commit/8768ff44006ff803fbb26f99e903ea2870782ed1) Initialize Keith Agent
