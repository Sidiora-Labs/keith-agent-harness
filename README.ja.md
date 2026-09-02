<h1 align="center">Keith</h1>

<p align="center">
  <strong>プロンプトの小手先ではなく、自身の実行基盤（harness）を変更・テスト・安全昇格させることで真に進化する、初めてのエージェント。</strong>
</p>

<p align="center">
  Keith は現実の経験を、推論・ツール選択・コンテキスト管理・仕事の進め方を決める
  仕組みそのものへの変更へと転換します。プロンプトに追記するメモではありません。
  新しいバージョンは隔離環境で構築され、現在の Keith と比較テストされ、安全境界を
  越えずに優れていると証明された場合にのみ採用されます。
</p>

<p align="center">
  <a href="https://github.com/Sidiora-Labs/keith-agent/actions/workflows/ci.yml"><img src="https://github.com/Sidiora-Labs/keith-agent/actions/workflows/ci.yml/badge.svg" alt="CI ステータス"></a>
  <a href="https://github.com/Sidiora-Labs/keith-agent/releases/latest"><img src="https://img.shields.io/github/v/release/Sidiora-Labs/keith-agent?display_name=tag" alt="最新リリース"></a>
  <a href="https://github.com/Sidiora-Labs/keith-agent/stargazers"><img src="https://img.shields.io/github/stars/Sidiora-Labs/keith-agent?style=flat" alt="GitHub スター"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-green" alt="ライセンス: Apache-2.0"></a>
  <a href="https://github.com/Sidiora-Labs/keith-agent/pkgs/container/keith-agent"><img src="https://img.shields.io/badge/container-GHCR-blue" alt="GHCR イメージ"></a>
</p>

<p align="center">
  <a href="docs/installation.md">はじめに</a> ·
  <a href="docs/deployment.md">デプロイ</a> ·
  <a href="docs/crate-guide.md">アーキテクチャ</a> ·
  <a href="CONTRIBUTING.md">コントリビュート</a> ·
  <a href="SECURITY.md">セキュリティ</a>
</p>

<p align="center">
  <img src="docs/assets/keith-harness-repair.png" alt="自身の harness への修復を隔離環境でテストする Keith" width="1100">
</p>

<p align="center"><sub>実作業から学び、より良い harness を作り、本番投入前に改善を証明する。</sub></p>

> [!IMPORTANT]
> Keith はプレリリースソフトウェアです。コアシステムは動作していますが、
> インターフェース・ストレージ・パッケージは 1.0 までに変更される可能性があります。

## 試してみる

最短経路は Docker です:

```bash
cp .env.example .env
# .env に KEITH_WEB_LOGIN_SECRET とモデルプロバイダーのキーを 1 つ設定してください。
docker compose up --build
```

<http://localhost:7341> を開いてください。Keith は状態を `keith-data` ボリュームに
保持し、現在のチェックアウトの `/workspace` 内で作業できます。

ソースから開発する場合はこちら:

```bash
./keith doctor
./keith setup
./keith dev
```

TUI、プロバイダー設定、アップグレード、バックアップ、サービス管理については
[インストールガイド](docs/installation.md) を参照してください。

## Keith はどう進化するのか

各実行は単なるログ以上のものを Keith に与えます。エージェント全体の働きに関する
証拠を生成します:推論経路、ツール選択、コンテキスト使用、レイテンシ、コスト、
回復過程、最終結果です。

致命的な失敗は進化の糸口になり得ますが、過剰なツール呼び出し、遅い回復、より
良いはずだった結果もまた同様です。

Keith は最も強い機会を検証可能な仮説に変換し、稼働中のシステムから離れた場所で
複数の候補 harness を構築し、提案者に見えていなかったタスクで現行バージョンと
比較します。候補は、測定可能な改善をもたらし、かつ退行を伴わない場合のみ先に
進めます。

```text
経験 → 機会 → 検証可能な仮説 → 候補 harness
     → ホールドアウト評価 → canary → 観察 → 採用 or 巻き戻し
```

これが Keith の中核的な賭けです:エージェントは仕事をするだけでなく、その仕事を
より上手くするための安全で監査可能な手段を持つべきです。

### 動作中に修正する

Keith Computer は可視化された隔離デスクトップです。実行をライブで観察し、
ワンアクションでキーボードとポインタを奪い、問題を修正し、制御を戻せます。
排他的な制御リースにより、あなたと Keith が同じ画面を取り合うことを防ぎます。

### 仕事そのものを教える ― 別のプロンプトではなく

あなたがタスクを実演すると、Keith はその背後にある有用な構造を記録します:
画面状態、キーボードとポインタの操作、UI ターゲット、アプリコンテキスト、
タイミング、ナレーション、ファイル、クリップボード活動、すべての制御の受け渡し。
これらの証拠を入力・チェックポイント・承認ステップ・回復手順・バージョン・ロール
バックを備えた編集可能な **TaskRecipe** に変換します。

あなたは単に画面録画を渡しているのではありません。再現し、改善できる仕事の
一片を見せているのです。

### ルールではなく harness を修復させる

自己修復が意味を持つのは、候補がゴールポストを動かせない場合だけです。Keith の
修復候補は、判定者、ホールドアウトされたテストケース、資格情報、Keith が記憶・
公開してよい範囲、承認ルール、リリース検査、昇格ゲート、ロールバック経路を編集
できません。

Keith の到達範囲を選んでください:

- **アドバイスのみ**   提案された修復を説明し、待ちます。
- **シャドーテスト**   候補を構築・テストしますが、昇格はしません。
- **自律修復**   設定した範囲内で canary・観察・巻き戻しを行います。

どのモードでも、保護されたルールは修復候補の届かない場所にあります。

### ボットだらけではなく、1 つの Keith

Web UI、TUI、OpenAI 互換 API、ネイティブ API、ACP クライアント、メッセージング
チャネル、接続アプリ、そしてコンピュータは、すべてデーモンが所有する同じ
エージェントに接続します。特化型のワーカーは裏側で調査・コーディング・ツール操作
を行えますが、そのために製品を管理すべき人格だらけのダッシュボードにはしません。

セッションはクライアント切断後も存続します。コミットメント、待機、予定作業、
ゴール、実行中の処理は再起動後に復旧できます。ターミナルで始め、Slack で
確認し、ブラウザで完了する ― 互いに無関係な 3 つのアシスタントを作ることなく。

## Keith でできること

| やりたいこと | Keith のできること |
| --- | --- |
| ブラウザやデスクトップのタスクを任せたい | 画面あり/なしのコンピュータで作業し、観察・一時停止・介入が可能 |
| 反復可能なワークフローを教えたい | ライブ実演を、編集・再生可能な TaskRecipe に変換 |
| 同じエージェントの失敗の繰り返しを止めたい | harness を診断し、複数の修復をテストし、勝者を canary で展開、巻き戻し可能 |
| 自分のモデルを使いたい | OpenAI、Anthropic、OpenRouter、Ollama その他の対応プロバイダー経由でプロファイルをルーティング |
| どこからでも同じエージェントにアクセスしたい | Web、TUI、ACP、API、Discord、Slack、Telegram、WhatsApp、Teams、Google Chat、メール、Matrix クライアントを提供 |
| 実サービスを接続したい | 承認付き接続アプリ、Composio、MCP サーバー、能力スコープ付きの WASI プラグインを利用 |
| Keith の上に構築したい | OpenAI 互換の `/v1` API、または型付けされたネイティブ `/platform/v1` API を使用 |
| 自社のインフラで運用したい | Docker、Kubernetes、Railway、Fly.io、DigitalOcean、Azure、AWS、Google Cloud でデプロイ |

## 1 つのランタイム、多くの入り口

```text
Web · TUI · OpenAI API · Platform API · ACP · チャネル
                         │
                      agentd
              セッション · ポリシー · リカバリー
                         │
                リースされたエージェントワーカー
                         │
       モデル · ツール · プラグイン · CUA · 接続アプリ
```

`agentd` が真実を持ちます。クライアントは独自の状態を発明するのではなく、
そのセッションとライフサイクルを描画するだけです。ワーカーはリース下で
ターンを実行し、ドメインクレートがポリシーを外部アダプタから分離します。

システム全体の地図については[クレートガイド](docs/crate-guide.md) と
[依存関係の境界](docs/architecture/dependency-boundaries.md) を参照してください。

## API と拡張

Keith は 2 つの HTTP サーフェスを公開します:

- **OpenAI 互換の `/v1`** ― Open WebUI のような既存 SDK とツール向け。
- **ネイティブ `/platform/v1`** ― Keith のセッション、ライフサイクル、承認、
  アーティファクト、ライブイベントを必要とする信頼されたクライアント向け。

拡張は、能力スコープ付きの WASI コンポーネント、MCP サーバー、スキル、
または承認付き接続アプリとして動作できます。ACP クライアントは同梱の stdio
サーバーを介して接続できます。[OpenAI 互換性](docs/openai-compatibility.md) と
[プラットフォーム統合](docs/platform-integration.md) を参照してください。

## セキュリティ

> [!WARNING]
> Keith はコマンド実行、ファイル変更、ブラウザ制御、そしてあなたが与えた権限で
> 外部サービス呼び出しを行うことができます。検査と復元が可能なワークスペースを
> 使ってください。モデル出力、チャネルメッセージ、取得ページ、プラグイン、
> スキル、MCP サーバー、修復候補は信頼できないものとして扱ってください。

TLS、強い認証、明示的なネットワークポリシーを追加しない限り、Web UI と API は
ループバックに留めてください。Web ログイン、API、モデルプロバイダー、リリース署名
には異なるシークレットを使用してください。資格情報や未編集のトレースを公開
Issue に投稿しないでください。

脆弱性は GitHub の
[セキュリティアドバイザリフォーム](https://github.com/Sidiora-Labs/keith-agent/security/advisories/new)
から非公開で報告してください。信頼モデル、対象範囲、報告ルールについては
[SECURITY.md](SECURITY.md) を読んでください。

## デプロイ

Keith は単一のステートフル OCI イメージとして配布され、Docker Compose、
Helm 付き Kubernetes、Railway、Fly.io、DigitalOcean Kubernetes、Azure
Kubernetes Service、Amazon EKS、Google Kubernetes Engine Autopilot をサポート
しています。

```bash
./keith deploy kubernetes --render
./keith deploy railway
./keith deploy fly --app my-keith
./keith deploy aws --cluster keith --region us-east-1
```

クラウドコマンドは既定で計画を出力します。デプロイは `--execute` を渡し、
`KEITH_DEPLOY_APPROVED=YES` を設定したときにのみインフラを変更します。
ホスト外に Keith を公開する前に、完全な[デプロイガイド](docs/deployment.md) を
読んでください。

## 開発と拡張

`./keith` コマンドは、セットアップ、ローカルサービス、チェック、テスト、
リリースビルド、コンテナイメージ、スキャフォールド、デプロイ計画を行う
コントリビューターのエントリポイントです。Rust のビルド成果物はチェック
アウトの外に出力されます。

```bash
./keith check
./keith test
./keith image keith-agent:dev
./keith scaffold plugin my-plugin
./keith scaffold skill my-skill
```

Rust 1.93、Node.js 22.22、Corepack、Git が必要です。Pull Request を開く前に
[CONTRIBUTING.md](CONTRIBUTING.md) を確認し、実際に実行したコマンドと
ユーザーパスを報告してください。

## ドキュメント

| 目的 | まずはこちら |
| --- | --- |
| インストール、プロバイダー設定、TUI の実行 | [インストールとライフサイクル](docs/installation.md) |
| Docker またはクラウドプロバイダーで実行 | [デプロイガイド](docs/deployment.md) |
| OpenAI SDK または Open WebUI を接続 | [OpenAI 互換性](docs/openai-compatibility.md) |
| 信頼されたネイティブクライアントを統合 | [プラットフォーム統合](docs/platform-integration.md) |
| ワークスペースを理解する | [クレートガイド](docs/crate-guide.md) |
| リリースを認定する | [リリース認定](docs/release-qualification.md) |
| 質問・問題報告 | [サポート](SUPPORT.md) |

## コミュニティ

- 質問やアイデアの共有は
  [GitHub Discussions](https://github.com/Sidiora-Labs/keith-agent/discussions) で。
- 再現可能なバグは
  [Issue フォーム](https://github.com/Sidiora-Labs/keith-agent/issues/new/choose) から報告してください。
- セキュリティ問題は公開 Issue ではなく、非公開で報告してください。
- すべてのプロジェクトスペースで[行動規範](CODE_OF_CONDUCT.md) を尊重してください。

## ライセンス

Keith は [Apache License 2.0](LICENSE) の下で公開されています。
