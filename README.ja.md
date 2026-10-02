<p align="center">
  <img src="./src-tauri/icons/128x128.png" width="96" alt="LibrAgent" />
</p>

# LibrAgent

> **自分で動かすエージェント実行環境 — モデルを選び、ツールはワンクリック、協調パターンを選ぶ。**
> _ベンダー製ハーネスなし。JSON 宿題なし。成果はマシン上のファイルとして残る。_

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

![ヒーローデモ — ツールはアプリのようにインストール。モデルは自分で選ぶ。成果はファイルに残る。](./assets/hero-demo-60s.gif)

[HD WebM](./assets/hero-demo-60s.webm) · _ツールはアプリのようにインストール。モデルは自分で選ぶ。成果はファイルに残る。_

---

## LibrAgent が違う理由

多くのエージェントハーネスは、MCP JSON を編集し、ターミナルに住み、オーケストレーションをコードで組み立てると仮定します（または単一ベンダーのスタックに閉じ込めます）。

LibrAgent は同じ仕事のための **デスクトップ製品** です：

| …の代わりに | 得られるもの |
| ----------- | ------------ |
| MCP 設定の手編集 | **Extensions** — ワンクリックプリセット（GitHub、Brave Search、Filesystem、…）と Cursor / VS Code / Claude Code / Windsurf からのインポート |
| 「マルチエージェントがあります」 | バンドルスキルとしての **名前付き協調パターン** — `pipeline`、`hub-spoke`、`divide-conquer`、`consensus-delegation`、… |
| ある提供者のモデル + ツール | **自分の** LLM（API キーまたは [Ollama](https://ollama.com)）と **自分の** MCP スタック — MIT、ローカル優先 |

[最新リリースをダウンロード](https://github.com/fritzprix/libr-agent/releases/latest) · [5分オンボーディング](#5分オンボーディングの道筋) · [ヒーローデモ仕様](docs/contributing/hero-demo-spec.md)

---

## 最初の 10 分でできること

### 1. ワンクリックツール、そして成果物

- **Extensions** を開き、プリセットをインストール（例: GitHub）— JSON 不要
- **Workspace** を実際のフォルダに向ける
- 依頼：_"このリポジトリで新規貢献者にとって最大のリスクをレビューし、`DELIVERABLE.md` に保存して"_

### 2. ワンクリックワークフローレシピをデプロイ

- Chat ホームまたは [Scheduled Tasks](docs/user/guides/scheduled-tasks.md) から **Morning Briefing** レシピを起動
- Hacker News + Yahoo Finance プリセットをインストールし、アシスタントを設定、毎日午前 9 時の実行をスケジュール
- 起きると技術・市場のブリーフィングが用意されている — 無人実行

### 3. 協調パターンを選ぶ（フレームワーク組み立てなし）

- 仕事の形を言うか、スキル名で添付：
  - _"@skill:pipeline — 調査、下書き、レビューの順；最終レポートを一つ"_
  - _"@skill:divide-conquer — 独立した断片に分けて結果をマージ"_
- パターンは製品化されたスキルです — 自分で配線する SDK ではありません。[Sub-agents & orchestration](docs/user/guides/sub-agents.md) を参照。

### 4. モデルの自由を保つ

- クラウド：OpenAI / Anthropic / Gemini / Groq API キーを貼る
- ローカル：`ollama pull qwen3:14b` して Ollama を選択 — ハーネスは同じ

---

## 三つの製品約束

1. **ハーネス宿題なしで表面に立つ** — GUI、Extensions ワンクリック、レシピ、アプリ内承認、`@skill:` — 「まず設定とシェルを開け」ではない。
2. **オーケストレーションを製品として** — スキルで Sequential / Hub-and-spoke / Swarm 風フローを選択；耐久チームや cron が必要なら `teamwork` → `org` と `schedule` へ — それでも LangGraph/CrewAI を自分で組まない。
3. **プロバイダとスタックの自由** — 対応 LLM どれでも、MCP をインフラとして、既存 IDE MCP 設定のインポート、MIT、デフォルトでローカルワークスペースとブラウザ状態。

**最適：** JSON なしでハーネスの深さを欲するオペレーターとパワーユーザー；単一ベンダーのエージェントスタックを拒否する開発者；ブラウザ + 知識 + スケジュールを一製品で要する研究者。

---

## 協調パターン（バンドルスキル）

**仕事の形** からモデルを選び、チャットから実行します：

| スキル | パターン | 使うとき |
| ------ | -------- | -------- |
| `pipeline` | 順次ステージ | 出力が次の入力になる（調査 → 下書き → レビュー） |
| `hub-spoke` | ハブアンドスポーク | 一人のコーディネータが多数のワーカーを統合 |
| `divide-conquer` | 並列分割 | 独立した断片、その後マージ |
| `consensus-delegation` | 多視点 | 同じ質問を複数の専門家へ、その後調整 |
| `gatekeeper` / `pair-programming` | レビューループ | 厳格レビューまたは二人エージェントのコーディング |
| `delegate` | 軽量ハンドオフ | 子セッション一つ、系譜を追跡 |
| `teamwork` → `org` | 耐久チーム | 共有憲章 + Org UI |
| `schedule` / `loop` / `call-me-back` | 時間とイベント | Cron、セッション内遅延、またはプロセス/webhook で再開 |

選択ヒューリスティック：[framework-selection](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · 完全ガイド：[Sub-agents](docs/user/guides/sub-agents.md)。

その他のデイワンスキル：`setup-wizard`、`tool-installer`、`playbook-creator` など — **[Bundled Skills](docs/user/guides/skills.md)**。

---

## MCP プラットフォーム（パワーユーザーにも耐える）

- トランスポート：stdio、HTTP、SSE、OAuth 2.1
- 15+ 組み込みサーバー（Workspace、Shell、Browser、Knowledge、Planning、Scheduled Tasks、…）
- ワンクリックプリセット + エージェント支援インストール（`tool-installer`）
- セッションごとのツール分離；パス/コマンドガード；自動化向け YOLO / unsafe モード

### 実行基盤

| 基盤 | 能力 |
| ---- | ---- |
| **Workspace** | 行単位の精密編集、マルチファイル操作、`@file` / `@skill` / `@playbook` コンテキスト |
| **Shell** | 隔離・永続シェルと非同期プロセス監視 |
| **Browser** | 隔離ブラウザサイドカー；任意の保存ログインプロファイル |
| **Knowledge** | グラフ知識 + BM25 検索 |
| **Export** | Markdown レポートと ATIF 軌跡エクスポート（[session export](docs/user/guides/session-export.md)） |

長いセッションはコンテキスト圧縮、ループ防止、サーキットブレーカー、stale-response ガードで生産性を保ちます。

---

## 実世界のシナリオ

### オペレーター — 空のアプリから毎日のブリーフィングへ

1. **Morning Briefing** レシピを実行（プリセット + アシスタント + 午前 9 時スケジュール）
2. **Run now** を一度クリックして確認
3. 放置 — ターミナルを開かずにレポートが届く

### ソロ開発者 — 設定ファイルではなくプリセット

1. Extensions → GitHub MCP プリセットをインストール
2. Workspace でローカルリポジトリを接続
3. ディスクに残る Markdown のセキュリティ/レビューレポートを依頼

### フレームワークなしのパワーユーザー — 名前付きオーケストレーション

1. 仕事の形に `@skill:pipeline`（または `hub-spoke` / `divide-conquer`）
2. エージェントがそのパターンで協調
3. ワークスペースにマージされた成果物一つ — 維持すべきオーケストレーションライブラリなし

### プライバシー重視チーム — 同じ製品、ローカルモデル

1. `ollama pull qwen3:14b`
2. Workspace + Shell はマシン上に留まる
3. 後からクラウドキーに切り替えても — ハーネスがベンダーを強制しない

---

## ドキュメント

- **[User Guide](docs/user/README.md)** — インストール、最初のチャット、モデル、スキル（[docs site](https://fritzprix.github.io/libr-agent/)）
- **[Hero Demo Spec](docs/contributing/hero-demo-spec.md)** — 正規の製品デモ（EN/KO/ZH 字幕）
- **[Product Messaging Guide](docs/contributing/product-messaging-guide.md)** — ポジショニングとコピー
- **[Recipes](docs/user/guides/recipes.md)** · **[Scheduled Tasks](docs/user/guides/scheduled-tasks.md)** · **[Sub-agents](docs/user/guides/sub-agents.md)** · **[Skills](docs/user/guides/skills.md)**
- **[HTTP API](docs/api/http_api.md)** — リモート制御とプログラマティック承認
- **[Architecture](docs/architecture/agent-workflow-architecture.md)** — セッション分離と Think-Act-Observe

---

## はじめに

**[Releases ページ](https://github.com/fritzprix/libr-agent/releases/latest)** から最新インストーラーをダウンロードしてください。

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.22_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_x64-setup.exe) · [`LibrAgent_0.9.22_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.22_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.22_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_amd64.AppImage) · [`LibrAgent_0.9.22_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_amd64.deb) · [`LibrAgent-0.9.22-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent-0.9.22-1.x86_64.rpm)
- **すべてのリリース資産:** [リリースページ](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.22)
<!-- RELEASE_DOWNLOADS_END -->

### 5分オンボーディングの道筋

**ステップ 1 — モデルを接続**（Settings → LLM Providers）

- クラウド：OpenAI / Anthropic / Gemini / Groq API キーを貼る
- ローカル：`ollama pull qwen3:14b`、その後 Settings で Ollama を選択

**ステップ 2 — JSON なしでツールを追加**

- Extensions → プリセットをインストール（例: GitHub）、**または**
- エージェントに：_"Cursor から MCP サーバーをインポートして"_

**ステップ 3 — ワークスペースを接続し、残すファイルを依頼**

- Workspace を実際のプロジェクトフォルダに
- _"このワークスペースをレビューし、発見を `DELIVERABLE.md` に書いて。"_

**次に — 協調と自動化**

- _"@skill:pipeline — 調査、下書き、レビュー；最終レポート一つ。"_
- _"このリポジトリ用の teamwork ワークスペースを準備して。"_
- _"毎朝 7 時の競合ブリーフをスケジュールして。"_（または Morning Briefing レシピ）

### コピー＆ペースト用の最初のプロンプト

- _"Cursor から MCP サーバーをインポートし、何が追加されたか見せて。"_
- _"GitHub MCP プリセットをインストールし、コーディングエージェントに付けて。"_
- _"このワークスペースをレビューし、発見を `DELIVERABLE.md` に書いて。"_
- _"@skill:pipeline — このトピックを調査し、要約を下書きし、レビュー；最終レポートを保存。"_
- _"毎朝 7 時の競合ブリーフをスケジュールして。"_

### 開発者セットアップ

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## LibrAgent が最も合う場所

| 欲しいもの | LibrAgent が強い理由 |
| ---------- | -------------------- |
| **ハーネスの深さ、宿題なし** | Extensions プリセット、レシピ、`@skill:` パターン、承認 — JSON 優先のオンボーディングではない |
| **フレームワークを組み立てないオーケストレーション** | `pipeline`、`hub-spoke`、`divide-conquer`、`consensus-delegation`、`teamwork` / `org`、`schedule` が製品として提供 |
| **ベンダーエージェントスタックからの自由** | モデルと MCP ツールは自分で；MIT；デフォルトでローカル優先 |
| **本物の実行基盤** | Workspace、shell、browser、knowledge、playbooks、長時間セッションガード |
| **MCP ネイティブなデスクトップ製品** | プリセット、インポート、15+ 組み込み — 薄いチャットラッパーではない |

---

## 設計思想

- **キットより製品**：組み立てなくてもハーネスが使える。
- **オーケストレーションはスキル**：協調パターンは名前があり、選択可能で文書化されている — サンプルリポジトリに埋まっていない。
- **スタックの自由**：モデルとツールはユーザーの選択；製品は特定の AI ベンダーを要求しない。
- **ローカル優先**：ワークスペース、セッション、スキル、ブラウザ状態はあなたの管理下。クラウド LLM / リモート MCP はオプトイン時のみ。
- **モデルよりハーネス**：ツール、セッション状態、委譲、ガバナンスが単一モデルより重要。
- **機能より安定**：分離、圧縮、ループ防止 — 機能追いの前に。
- **オープンスタンダード**：MIT。相互運用レイヤーとしての MCP。

---

## 貢献とライセンス

LibrAgent は MIT ライセンスでオープンに構築されています。バンドルスキル、MCP 統合、バグ修正、アーキテクチャ改善を歓迎します。

- [Contributing Guide](CONTRIBUTING.md)
- [Issue Tracker](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- [Discussions](https://github.com/fritzprix/libr-agent/discussions)
- ベンチマーク（Harbor / Terminal-Bench）：[Harbor guide](benchmarks/harbor/README.md) を参照（`pnpm bench:diverse`、`pnpm bench:terminal`、…）

**License**: MIT
