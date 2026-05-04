# 🤖 LibrAgent

> **自律型知能の時代のためのエージェントハーネス。**
> _単なるチャットアプリではありません。エージェントが働き、協力し、スケールする実行基盤です。_

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgentはTauri + Rust + React上に構築された**ローカルファーストのエージェントOS**です。チャットインターフェースをはるかに超え、安全な実行基盤、MCPネイティブのツールエコシステム、単一エージェントを協調クラスターに拡張する再帰的委任アーキテクチャを提供します。

任意のLLM（クラウドまたはOllama経由のローカル）に接続し、任意のMCPサーバーで拡張し、エージェントに実際の作業をさせましょう：ファイル編集、シェル実行、ウェブブラウジング、知識管理——自律的に、必要なだけ長く。

---

## なぜLibrAgentなのか？

AI業界の焦点が変わりました。2026年の最近のベンチマーク分析は、**同じモデルでも、その周囲のハーネスによって二桁のタスク成功率の差が生じる**ことを示しました。モデルはエンジンです——しかしハーネスがどこまで到達できるかを決めます。

現在のすべての選択肢はまだトレードオフを強いています：

| プラットフォーム | 問題点 |
|---|---|
| **OpenClaw** | 高い柔軟性のオープンエコシステムだが、2026年初頭の分析は露出インスタンス、平文秘密鍵処理、コミュニティスキルのプロンプトインジェクションリスクを指摘。 |
| **Claude Cowork** | 強力なローカルUXだが、複雑な自律タスクでは依然限界あり。クローズドエコシステム。拡張不可。 |
| **Claude Code / Cursor** | 開発者専用。ターミナルの習熟が必要。汎用ではない。 |
| **Google Mariner** | 作業がGoogleのクラウドVMで実行される。データを制御できない。 |
| **LangGraph / CrewAI** | 強力なフレームワークだが、すべて自分で組み立てる必要がある。製品体験なし。 |

**LibrAgentはそのトレードオフを解消するために構築されました。** ローカルファーストのセキュリティ。MCPネイティブの拡張性。クラスター→組織のマルチエージェント調整。非開発者向けに洗練されたGUI。すべてひとつのオープンソースデスクトップアプリに。

### LibrAgentが対象とするユーザー

- **ソロ開発者**：実際にローカルで読み書き実行・ブラウズしてコンテキストを維持するエージェントが欲しい人
- **パワーユーザーとオペレーター**：ローカルモデル、APIプロバイダー、MCPサーバー、スケジュールワークフローから独自スタックを構築したい人
- **研究者・アナリスト**：ブラウザ自動化、知識取得、反復可能なプレイブック、長時間セッションが必要な人
- **プライバシー重視のチーム**：ローカル実行、明示的なガバナンス、単一エージェントから調整された組織へのパスが欲しいチーム

---

## 🎬 プラットフォーム動作

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

_単一エージェントから協調クラスターまで——再帰的委任、MCPツール、持続的ワークスペースが一つの統合基盤に。_

---

## コアピラー

### 1. 🔐 ローカルファーストセキュリティ——データはあなたのマシンに

LibrAgentはセキュリティをファーストクラスのアーキテクチャ上の関心事として扱います：

- **セッション分離**：すべてのエージェントセッションが専用の`MCPServiceProxy`インスタンスを持つ——クロスセッションのデータ漏洩ゼロ
- **内蔵SecurityValidator**：パストラバーサル攻撃とコマンドインジェクションをシステムレベルでブロック
- **クラウド基盤不要**：すべての実行はローカルで発生；LLM APIコールのみがマシンを離れる
- **完全オフラインサポート**：[Ollama](https://ollama.ai)とペアでエアギャップされたエージェントスタック

#### ローカルに留まるものと離れるもの

- **常にローカル**：ワークスペース、ローカルファイル、バンドルスキル、セッション状態、MCPサーバー設定、ブラウザ状態、ローカルツール実行
- **選択した時のみ離れる**：クラウドLLMプロバイダーまたはリモートMCP/HTTPサービスへのリクエスト——明示的に設定した場合のみ
- **完全オフラインモード**：OllamaやローカルMCPサーバーでエアギャップされたワークフロー

### 2. 🧩 MCPネイティブエコシステム——設計による無限の拡張性

MCP（Model Context Protocol）は2026年にLinux Foundation標準となりました。LibrAgentはこれを機能としてではなくアーキテクチャの骨幹として扱います：

- **完全なトランスポートサポート**：stdio、HTTP、SSE、OAuth 2.1——完全な仕様
- **12以上の内蔵サーバー**：Planning、Knowledge(RAG)、Browser Automation、Workspace、Shell Execution、Content Storeなど
- **プリセットカタログ**：GitHub、Brave Search、Filesystemなど人気サーバーをワンクリックでインストール
- **セッション分離インスタンス**：各エージェントセッションが独立したMCPサーバー状態を持つ——並列エージェント間で干渉なし
- **どこからでもインポート**：Cursor、VS Code、Claude Code、WindsurfからMCP設定を自動移行

### 3. 🦾 プロダクショングレードの実行基盤

ほとんどのAIツールはデモでは印象的ですが、プロダクションでは脆弱です。LibrAgentは長時間の実際の作業のために徹底的にエンジニアリングされています：

| 基盤 | 機能 |
|---|---|
| **Workspace** | 行単位の精密編集、マルチファイル操作、統合検索、`@file`/`@skill`/`@playbook`コンテキスト注入 |
| **Shell** | 分離実行 AND 持続シェル——非同期プロセス監視(`poll`、`read output`、`list`) |
| **Browser** | キャッシュ一貫性保証付きPlaywrightスタイルツール(`goto`、`click`、`fill`、`screenshot`) |
| **Knowledge** | エンティティ/関係抽出(v2)、BM25全文検索付きグラフベース知識管理 |

**信頼性エンジニアリング込み**：コンテキスト圧縮、ループ防止、サーキットブレーカー、陳腐化レスポンスガードで数時間に及ぶセッションでもエージェントを生産的に保ちます。

### 4. 🤝 クラスター→チーム→組織：あらゆる規模のマルチエージェント

LibrAgentはソロ実行から明示的な組織調整まで一貫したマルチエージェントのストーリーを持ちます：

- **`delegate`**：親エージェントが明示的系譜追跡で子セッションを生成、ブリーフィング、監視
- **`teamwork`**：ワンコマンドで完全なタスクフォースワークスペース(agents.md、MISSION.md、KANBAN.md)をスキャフォールド
- **`org`**：持続的組織アイデンティティ、ルートセッション再開、org-visibleメンバー階層でチームを正式化
- **`schedule`**：CRONベースの自動化——エージェントが無人で、スケジュール通りに、ワークスペース憲法に従って実行
- **Concurrency Gate**：並列セッションとシェルプロセスのハードリミットでデッドロックとコスト暴走を防止

### 5. ⚡ バンドルスキル——空のインストールから動作するクラスターへの最速ルート

LibrAgentは成長し続ける**バンドルスキル**ライブラリを同梱します。ランダムなプロンプトではなく——任意のエージェントが名前で呼び出せる再利用可能な操作手順です。

最重要のday-oneスキル：

| スキル | 機能 |
|---|---|
| `system-setup` | すべてのプラットフォームで不足しているランタイム(Python、Node.js、uv)を検出・インストール |
| `mcp-installer` | npmパッケージ、GitHub URL、JSON設定ブロックからMCPサーバーを登録 |
| `mcp-importer` | Cursor、VS Code、Windsurf等から既存MCP設定をインポート |
| `specialist-creator` | ロール説明から完全なエージェント設定(システムプロンプト、モデル、ツール)を設計 |
| `crew-constructor` | 利用可能なツールをスキャンし、マッチしたスペシャリストチームを自動バッチ作成 |
| `agent-tooling` | エージェントを監査し、能力の不一致を検出し、ツール割り当てを動的に再バランス |
| `delegate` | 明示的なコンテキスト転送と系譜追跡付きで親→子セッションの引き継ぎを案内 |
| `teamwork` | 調整されたマルチエージェント作業のための共有ワークスペース憲法をスキャフォールド |
| `org` | 持続的な組織アイデンティティとorg-visibleメンバー階層を正式化 |
| `schedule` | 無人自動化のための定期スケジュールタスクグループを作成・管理 |
| `soul-awakening` | エージェントを`SOUL.md`ペルソナに固定——トーン、スタンス、アイデンティティ |

これはオペレーターレイヤーだけです。LibrAgentはドメインスキルも提供します：

- **知識と研究**：`deep-research-report`、`knowledge-distiller`
- **ドキュメントワークフロー**：`document-to-markdown`、`docx`、`pptx`
- **スキルとワークフロー作成**：`skill-creator`、`skill-deployer`、`playbook-creator`、`mcp-builder`
- **特殊操作**：`computer-diagnosis`および他の専門ヘルパー

_重要：`bootstrap`はこれらのスキルと並行して使用される内蔵機能です。バンドルスキルは再利用可能な手順であり、内蔵機能とMCPツールはその下の実行基盤です。_

---

## 🌍 実世界のシナリオ

### ソロ開発者——自動コードレビュー
1. WorkspaceツールでローカルリポジトリをConnect
2. GitHub MCPプリセットをインストール（ワンクリック）
3. 依頼：_「PR #42のセキュリティ問題を見つけてMarkdownレポートを作成」_
4. エージェントがコードを読み、分析を実行し、発見事項をKnowledgeサーバーに保存

### マーケター——競合情報をオートパイロットに
1. Browserツールで5つの競合ブログを設定
2. エージェントに伝える：_「毎朝7時に競合ブリーフを予約して」_——エージェントが`schedule`スキルで定期タスクグループを設定
3. エージェントがブラウズ、要約し、Knowledge storeに追記
4. いつでも聞く：_「先週の競合の動向を要約して」_

### エンジニアリングチーム——オフラインエージェントスタック
1. `ollama pull qwen3:14b`——APIキー不要、クラウド不要
2. Workspace + ShellツールをコードベースにConnect
3. 機密IPがマシンから外に出ない
4. エージェントが読み、修正し、テストし、コミット——完全ローカル

### パワーユーザー——マルチエージェント研究パイプライン
1. `crew-constructor`で自動生成：Researcher×3、Analyst×1、Writer×1
2. オーケストレーターが`delegate`スキルで並列委任
3. 結果がContent Storeの単一構造化レポートにマージ
4. `schedule`でワークフロー全体を週次予約

---

## 📖 ドキュメントとガイド

- **[ナビゲーションガイド](docs/guides/navigation-guide.md)**：Command & Controlハブ——`/assistants`(ロール定義)と`/playbooks`(ワークフロー設計図)。
- **[アーキテクチャガイド](docs/architecture/agent-workflow-architecture.md)**：セッション分離、オーケストレーションエンジン、RustドリブンのThink-Act-Observeループ。
- **[内蔵ツールガイド](docs/guides/builtin_tool_bp.md)**：ツール設計標準とMCPレスポンスパターン。

---

## 📦 はじめに

**[リリースページ](https://github.com/fritzprix/libr-agent/releases/latest)**からプラットフォーム別の最新インストーラーをダウンロード。

```
Windows  →  LibrAgent_x.x.x_x64-setup.exe
macOS    →  LibrAgent_x.x.x_aarch64.dmg
Linux    →  libragent_x.x.x_amd64.AppImage
```

**開発者セットアップ：**

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

### 5分オンボーディングパス

**ステップ1——モデルを接続** (Settings → LLM Providers)
- クラウド：OpenAI / Anthropic / Gemini / Groq APIキーを貼り付け
- ローカル：`ollama pull qwen3:14b`後にSettingsでOllamaを選択
- CursorやVS Codeを使用中？任意のエージェントに：_「CursorからMCPサーバーをインポートして」_ → `mcp-importer`が対応

**ステップ2——MCPツールを追加** (Extensionsサイドバー)
- プリセットカタログを閲覧してInstallをクリック、または
- エージェントに：_「Install @modelcontextprotocol/server-everything」_ → `mcp-installer`が自動登録

**ステップ3——最初のエージェントを作成**
- _「競合情報のためのresearcherエージェントを作成して」_ → `specialist-creator`が完全な設定を設計
- _「現在のツールでresearchチームを構築して」_ → `crew-constructor`がバッチ作成
- _「すべてのエージェントのツール割り当てを最適化して」_ → `agent-tooling`が監査・再バランス

**ステップ4——`delegate`で並列作業**
- 任意のエージェントに子セッションへのサブタスク委任を依頼
- `delegate`スキルがコンテキスト引き継ぎ、系譜追跡、結果マージを管理

**ステップ5——持続的チームを構築**
- `teamwork` → `agents.md`、`MISSION.md`、`KANBAN.md`で共有ワークスペースをスキャフォールド
- `org` → 持続的アイデンティティとorg-rootセッション管理でチームを正式化
- `schedule` → 無人CRONベースの自動化をエージェントが作成・管理

### コピーペーストできる最初のプロンプト

- _「CursorからMCPサーバーをインポートして何が追加されたか見せて。」_
- _「現在のツールで競合情報のためのresearcherエージェントを作成して。」_
- _「GitHub MCPプリセットをインストールしてcodingエージェントに接続して。」_
- _「リポジトリ分析を子セッションに委任して要約を返して。」_
- _「このリポジトリのteamworkワークスペースを準備して、org-readyなspecialistチームを作成して。」_
- _「毎朝7時に予約された日次競合ブリーフを設定して、shared teamworkワークスペースに保持して。」_

---

## LibrAgentの比較

```
                    Privacy/Local  MCP Ecosystem  Non-Dev UX  Multi-Agent  Open Source
LibrAgent              ★★★★★          ★★★★★         ★★★★☆       ★★★★★           ✅
OpenClaw               ★★☆☆☆          ★★★★☆         ★★★☆☆       ★★★☆☆           ✅
Claude Cowork          ★★★★☆          ★★☆☆☆         ★★★★★       ★★☆☆☆           ❌
Claude Code            ★★★★☆          ★★★☆☆         ★☆☆☆☆       ★★★☆☆           ❌
Google Mariner         ★★☆☆☆          ★★★☆☆         ★★★★☆       ★★★★☆           ❌
LangGraph / CrewAI     ★★★☆☆          ★★★☆☆         ★★☆☆☆       ★★★☆☆           ✅
```

---

## 設計哲学

- **ローカルファースト**：データ、キー、エージェントの「魂」はあなたの完全な管理下に。クラウド基盤不要。
- **ハーネス優先**：実行環境——ツール、セッション状態、委任、ガバナンス——が個々のモデルより重要。LibrAgentはどのモデルでも最大限のパフォーマンスを発揮できるようエンジニアリング。
- **安定性優先**：CHANGELOGはランタイムの正確性への強迫的なフォーカスを反映——セッション分離、圧縮、ループ防止、陳腐化レスポンスガード。
- **インフラとしてのMCP**：プラグインシステムではなく。ツールエコシステム全体がMCPを主要な相互運用性レイヤーとして構成。
- **オープンスタンダード**：MITライセンス。MCP、オープンソースの相互運用性、ユーザーデータ主権に完全にコミット。

---

## 貢献とライセンス

LibrAgentはMITライセンスで公開されたオープンソースです。バンドルスキル、MCP統合、バグ修正、アーキテクチャ改善など、あらゆる貢献を歓迎します。

- 📖 [貢献ガイド](CONTRIBUTING.md)
- 🐛 [イシュートラッカー](https://github.com/fritzprix/libr-agent/issues)
- 💬 [ディスカッション](https://github.com/fritzprix/libr-agent/discussions)

**ライセンス**: MIT
