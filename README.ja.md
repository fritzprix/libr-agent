# 🤖 LibrAgent

> **状態保持が可能な、軽量自律型AIエージェントプラットフォーム。**

[English](./README.md) | [한국어](./README.ko.md) | [简体中文](./README.zh.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgentは、対話の合間でもコンテキストを維持するように設計されたローカルファーストのエージェント実行環境です。従来のステートレスなクライアントとは異なり、ブラウザのタブやターミナルセッションを維持できるため、エージェントは一貫したワークスペース内でより柔軟に動作できます。

また、**MCP (Model Context Protocol)** や **Skills** といったオープンスタンダードをサポートし、高いモジュール性と拡張性を備えています。

---

## なぜLibrAgentを作ったのか？

このプロジェクトの目標は、自律型エージェントを誰もが使えるようにすることです。既存のツールの多くは、ターミナルコマンドや複雑なJSON設定に依存しており、多くの潜在的なユーザーにとって高いハードルとなっています。LibrAgentは、開発者でなくても誰もがローカル環境で自分だけのエージェントを構築・管理できる環境を提供することで、この技術的格差を解消することを目指しています。

---

## 🎬 デモ

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

_ブラウザの自動操作とシェル実行を、単一のステートフルなワークフローで実現。_

---

## 主な機能

### 1. 永続的なワークスペース (Persistent Workspace)

エージェントは毎回の対話で新しいプロセスを起動するのではなく、長期的な環境内で動作します。

- **Live Webview**: Tauri webviewsを使用したリアルタイムのブラウザ自動操作。セッションとクッキーは対話をまたいで維持されます。
- **統合ターミナル**: ワークスペースと状態を共有し、サンドボックス化された永続シェル (Python/Node.js対応)。

### 2. マルチエージェント・オーケストレーション

LibrAgentは、エージェントが特定のタスクを専門のサブエージェントに委託することを可能にします。

- **アシスタント (Assistants)**: 独自のシステムプロンプトやツール設定を持つエージェントプロファイルの管理。
- **Swarm Intelligence**: 親エージェントがサブエージェントを生成、指示し、結果を待機することで複雑な問題を解決します。

### 3. 拡張性

コミュニティ標準を通じて拡張できるように設計されています。

- **拡張 (MCP)**: Model Context Protocolを完全サポート。あらゆるMCPサーバーに即座に接続可能。
- **ワンクリック・プリセット**: UIからGitHub、Brave Searchなどを直接インストールできるキュレートされたカタログ。
- **Skills & Playbooks**: 再利用可能な行動スニペットと定型化されたワークフローテンプレート。

### 4. 自律性とスケジューリング

- **YOLOモード**: 機密性の高いツールの実行を、手動承認なしで自律的に行うオプション。
- **スケジュールタスク**: Cronベースの自動化。指定されたワークスペースでの動作と、再起動後の自動復旧をサポート。

### 5. コンテキストとメトリクス

- **@メンション**: ファイル、スキル、プレイブックをチャットに直接挿入。
- **マルチモーダル**: OpenAI、Anthropic、Geminiモデルでの画像および音声の処理に対応。
- **オブザーバビリティ**: リアルタイムのTPSメトリクスとプロンプトキャッシュヒット率の表示。

---

## 📦 インストール

[リリースページ](https://github.com/fritzprix/libr-agent/releases/latest)からWindows、macOS、Linux用の最新バイナリをダウンロードしてください。

**ソースからビルド:**

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## 設計の選択

- **ローカルファースト**: データとAPIキーはすべてローカルマシン内に保持されます。
- **Tauri + Rust**: セキュリティ（メモリ安全性）、パフォーマンス、およびバイナリサイズの最適化のために選択されました。
- **SQLite (SeaORM)**: セッションや設定の堅牢なローカル永続化に使用。

---

## 貢献とライセンス

貢献を歓迎します。詳細は `CONTRIBUTING.md` を参照してください。

**ライセンス**: MIT
