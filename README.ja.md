<p align="center">
  <img src="./src-tauri/icons/128x128.png" width="96" alt="LibrAgent" />
</p>

# LibrAgent

> **ユーザー自身がコントロールするローカル AI エージェントデスクトップ環境です。**
> 任意の LLM を接続し、ワンクリックでツールを追加し、実証済みのマルチエージェント協調パターンでタスクを自動化します。
> _特定ベンダーへの依存や複雑な JSON 設定は不要です。すべての成果物はローカル PC に安全にファイルとして保存されます。_

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

![ヒーローデモ — アプリ感覚でツールを導入し、モデルを自在に選択。成果物は手元のファイルに。](./assets/hero-demo-60s.gif)

[HD WebM](./assets/hero-demo-60s.webm) · _アプリ感覚でツールを導入し、モデルを自在に選択。成果物は手元のファイルに。_

---

## LibrAgent が選ばれる理由

多くのエージェントフレームワークは、ユーザーがターミナルでの煩雑なコマンド操作や手動の JSON 構成に慣れており、オーケストレーションをコードで自作することを前提としています。また、特定クラウドベンダーのスタックに強く依存するものも少なくありません。

LibrAgent は、これらの作業を直感的に解決する **完成されたデスクトップ製品** です：

| 従来の課題 | LibrAgent のアプローチ |
| ---------- | ---------------------- |
| 複雑な MCP JSON 設定の手動編集 | **ワンクリック拡張機能** — GitHub、Brave Search、ファイルシステムなどのプリセット提供、および Cursor / VS Code / Claude Code からの設定インポート |
| コードでの組み立てが必要なマルチエージェント | **標準協調パターンを内蔵** — `pipeline`、`hub-spoke`、`divide-conquer`、`consensus-delegation` などのバンドルスキル |
| 特定ベンダーのモデル・ツールへの束縛 | **自由なスタック選択** — クラウド API からローカル [Ollama](https://ollama.com) まで自由に接続できる MIT ライセンス・ローカル優先設計 |

[最新リリースをダウンロード](https://github.com/fritzprix/libr-agent/releases/latest) · [5分オンボーディング](#5分オンボーディングの手順) · [デモ仕様書](docs/contributing/hero-demo-spec.md)

---

## 最初の 10 分で体験できること

### 1. ワンクリックでのツール導入と成果物生成

- **Extensions** メニューから GitHub などのプリセットをワンクリックで導入（JSON 設定不要）。
- **Workspace** で作業対象のローカルプロジェクトフォルダーを指定。
- プロンプトを実行：_"このリポジトリを分析し、新規コントリビューターが注意すべきリスクを `DELIVERABLE.md` にまとめて"_

### 2. 自動化ワークフローレシピの実行

- チャットホームまたは [スケジュールタスクガイド](docs/user/guides/scheduled-tasks.md) から **Morning Briefing** レシピを実行。
- Hacker News および Yahoo Finance の連携が構成され、毎朝 09:00 の定期実行がスケジュールされます。
- 毎朝、最新のテクノロジーおよび市況ブリーフィングが自動生成されます。

### 3. コード不要の協調パターン選択

- 複雑なコードを書くことなく、自然言語で協調パターンを指定：
  - _"@skill:pipeline — 調査、ドラフト作成、レビューの順に進め、最終レポートを 1 つ作成して"_
  - _"@skill:divide-conquer — このタスクを独立したサブタスクに分割して並列処理し、結果を統合して"_
- 実証済みの協調パターンがスキルとして組み込まれています。詳細は [サブエージェントとオーケストレーション](docs/user/guides/sub-agents.md) を参照してください。

### 4. 自由なモデル選択

- **クラウドモデル**: OpenAI、Anthropic、Gemini、Groq などの API キーを入力して即座に利用。
- **ローカルモデル**: `ollama pull qwen3:14b` を実行後、設定で Ollama を選択することで完全ローカル環境で動作。

---

## 製品の 3 つのコアバリュー

1. **セットアップ不要の洗練された UI** — GUI、ワンクリック拡張、自動化レシピ、アプリ内承認システムにより、設定ファイルの編集やターミナル操作なしで即座に利用可能。
2. **組み立て不要の内蔵オーケストレーション** — スキルを通じて Sequential、Hub-and-spoke、Swarm などのワークフローを選択し、長期的なチーム運用（`teamwork`）や定期実行（`schedule`）へスムーズに拡張。
3. **完全なモデルとデータの独立性** — オープン標準である MCP（Model Context Protocol）を基盤とし、ワークスペースやブラウザセッションのデータはすべて手元のローカル環境に保持。

**推奨ユーザー:** 設定に時間をかけず強力なエージェントを活用したい実務者、単一ベンダーロックインを回避したい開発者、ブラウザ自動化・ナレッジベース・定期タスクを統合環境で扱いたい研究者。

---

## 内蔵協調パターン（バンドルスキル）

タスクの特性に合わせた協調パターンを選択し、チャットから直接呼び出すことができます：

| スキル名 | 協調パターン | 推奨用途 |
| -------- | ------------ | -------- |
| `pipeline` | 順次ステージ実行 | 前段の出力を次段の入力として順次処理（調査 → ドラフト → レビュー） |
| `hub-spoke` | ハブ＆スポーク | 1 つの統合コーディネーターが複数の専門エージェントを総括管理 |
| `divide-conquer` | 並列分割処理 | 独立したサブタスクに分割して並列実行し、結果をマージ |
| `consensus-delegation` | 多角的比較分析 | 同一の課題を複数の専門エージェントに諮問し、合意を形成 |
| `gatekeeper` / `pair-programming` | レビーループ | 厳格な品質レビューや 2 エージェントによるペアプログラミング |
| `delegate` | 軽量タスク委任 | 単一の子セッションにタスクを委任し、系譜を追跡 |
| `teamwork` → `org` | 永続プロジェクトチーム | 共有憲章と組織 UI（Org UI）に基づくチーム協調 |
| `schedule` / `loop` / `call-me-back` | 時間・イベント駆動 | Cron スケジュール実行、セッション内遅延、外部イベントによる再開 |

選定ガイドライン: [フレームワーク選定基準](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · 詳細ガイド: [サブエージェント](docs/user/guides/sub-agents.md)

その他の初期スキル: `setup-wizard`、`tool-installer`、`playbook-creator` など — **[バンドルスキル一覧](docs/user/guides/skills.md)**

---

## 強力な MCP 実行インフラ

- **多彩なトランスポート**: stdio、HTTP、SSE、OAuth 2.1 をサポート
- **15 以上の内蔵サーバー**: Workspace、Shell、Browser、Knowledge、Planning、Scheduled Tasks などを標準装備
- **ワンクリックプリセットと対話型導入**: `tool-installer` によるエージェント支援型セットアップ
- **強固なセッション分離**: セッションごとのツール分離、パスおよびコマンド実行ガード、安全な自動化モード

### コア実行インフラ

| 実行領域 | 主要機能 |
| -------- | -------- |
| **Workspace** | 行単位の精密コード編集、複数ファイル一括操作、`@file` / `@skill` / `@playbook` コンテキスト注入 |
| **Shell** | 分離された永続シェル環境およびバックグラウンドプロセスの非同期監視 |
| **Browser** | 分離されたブラウザサイドカーおよび日常の Chrome セッションと連動する拡張ブリッジ |
| **Knowledge** | ナレッジグラフと高速 BM25 ハイブリッド検索 |
| **Export** | Markdown レポート出力および標準 ATIF セッション軌跡エクスポート（[セッションエクスポート](docs/user/guides/session-export.md)） |

コンテキストの自動圧縮、無限ループ検知、サーキットブレーカー、応答バリデーションガードにより、長時間のセッションでも安定したパフォーマンスを維持します。

---

## 実践的な活用シナリオ

### 業務オペレーター — 朝のブリーフィング自動生成

1. **Morning Briefing** レシピを実行（プリセット導入、アシスタント設定、毎朝 09:00 の定期実行予約）。
2. **Run now** で動作確認。
3. ターミナルを開くことなく、毎朝自動生成された最新レポートを確認。

### 個人開発者 — 設定ファイル不要の即時ツール連携

1. Extensions メニューから GitHub MCP プリセットを導入。
2. Workspace でローカルのリポジトリを接続。
3. セキュリティとコードレビューの Markdown レポート生成をエージェントに指示。

### パワーユーザー — 協調テンプレートによるタスク委任

1. 入力欄に `@skill:pipeline`（または `hub-spoke`、`divide-conquer`）を指定。
2. エージェント群がパターンに従って協調動作。
3. 外部ライブラリを保守することなく、統合された最終成果物をワークスペースに出力。

### プライバシー重視のチーム — 完全ローカル実行

1. `ollama pull qwen3:14b` を実行。
2. Workspace と Shell の全処理がローカル PC 完結で動作。
3. 必要に応じてクラウドモデルへの切り替えも可能で、操作フローはそのまま維持。

---

## ドキュメント

- **[ユーザーガイド](docs/user/README.md)** — インストール、チャット、モデル、スキル（[ドキュメントサイト](https://fritzprix.github.io/libr-agent/)）
- **[ヒーローデモ仕様](docs/contributing/hero-demo-spec.md)** — 製品デモ仕様書（EN/KO/ZH 字幕対応）
- **[製品メッセージングガイド](docs/contributing/product-messaging-guide.md)** — ポジショニングとコピーライティング
- **[レシピ](docs/user/guides/recipes.md)** · **[スケジュールタスク](docs/user/guides/scheduled-tasks.md)** · **[サブエージェント](docs/user/guides/sub-agents.md)** · **[スキル](docs/user/guides/skills.md)**
- **[HTTP API](docs/api/http_api.md)** — リモート制御とプログラマティック承認
- **[アーキテクチャ](docs/architecture/agent-workflow-architecture.md)** — セッション分離と Think-Act-Observe ループ

---

## はじめに

**[Releases ページ](https://github.com/fritzprix/libr-agent/releases/latest)**から最新のインストーラーをダウンロードしてください。

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.23_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.23/LibrAgent_0.9.23_x64-setup.exe) · [`LibrAgent_0.9.23_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.23/LibrAgent_0.9.23_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.23_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.23/LibrAgent_0.9.23_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.23_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.23/LibrAgent_0.9.23_amd64.AppImage) · [`LibrAgent_0.9.23_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.23/LibrAgent_0.9.23_amd64.deb) · [`LibrAgent-0.9.23-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.23/LibrAgent-0.9.23-1.x86_64.rpm)
- **すべてのリリース資産:** [リリースページ](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.23)
<!-- RELEASE_DOWNLOADS_END -->

### 5分オンボーディングの手順

**ステップ 1 — モデルの接続**（設定 → LLM Providers）

- クラウド: OpenAI、Anthropic、Gemini、Groq などの API キーを入力。
- ローカル: `ollama pull qwen3:14b` を実行し、設定で Ollama を選択。

**ステップ 2 — ツールの追加（JSON 編集不要）**

- Extensions メニューから GitHub などのプリセットをインストール、または
- エージェントに指示：_"Cursor で使っている MCP サーバー設定をインポートして"_

**ステップ 3 — ワークスペースの接続と成果物の生成**

- Workspace で作業対象のローカルプロジェクトフォルダーを指定。
- エージェントに指示：_"このワークスペースを分析して、結果を `DELIVERABLE.md` にまとめて"_

**次のステップ — 協調と自動化の拡張**

- _"@skill:pipeline — 調査、ドラフト、レビューを進め、最終レポートを 1 つ作成して"_
- _"このリポジトリ用の teamwork ワークスペースを準備して"_
- _"毎朝 7 時に競合動向をまとめるスケジュールタスクを作成して"_（または Morning Briefing レシピを実行）

### すぐに使えるサンプルプロンプト

- _"Cursor で使っている MCP サーバー設定をインポートして、何が追加されたか確認して。"_
- _"GitHub MCP プリセットを導入して、コーディングエージェントに接続して。"_
- _"このワークスペースをレビューして、改善点を `DELIVERABLE.md` にまとめて。"_
- _"@skill:pipeline — このトピックを深く調査し、サマリーを作成してレビューの上、最終レポートとして保存して。"_
- _"毎朝 7 時に市況ブリーフィングを作成するスケジュールを設定して。"_

### 開発者向けセットアップ

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## 設計思想

- **組み立てキットではなく完成品**: 面倒なコード配線なしに、導入直後から実務で活用可能。
- **スキルとして定義されたオーケストレーション**: 協調パターンが名前付きの仕様として体系化されています。
- **自由な技術スタック**: モデルやツールはユーザーが自由に選択でき、特定の AI ベンダーを強制しません。
- **ローカル優先（Local First）**: ワークスペース、セッション、ブラウザ状態はすべてユーザーの PC 内で安全に管理されます。
- **モデルを支える実行環境の重視**: 優れたツール統合、セッション状態管理、適切なガバナンスが、単一モデルの性能以上に安定した成果を生み出します。
- **機能拡張よりシステムの安定性**: コンテキスト分離、自動圧縮、ループ防止など、基盤の安定性を最優先に検証。
- **オープンスタンダードの遵守**: MIT ライセンスを掲げ、MCP（Model Context Protocol）を相互運用の基盤標準として採用。

---

## コントリビューションとライセンス

LibrAgent は MIT ライセンスのもとでオープンに開発されています。バンドルスキル、MCP 連携、バグ修正、アーキテクチャの改善など、あらゆる貢献を歓迎します。

- [コントリビューションガイド (Contributing Guide)](CONTRIBUTING.md)
- [Issue トラッカー](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- [ディスカッション (Discussions)](https://github.com/fritzprix/libr-agent/discussions)
- ベンチマーク (Harbor / Terminal-Bench): [Harbor ガイド](benchmarks/harbor/README.md)（`pnpm bench:diverse`、`pnpm bench:terminal` など）

**License**: MIT
