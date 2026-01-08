# Claude Code 設定

このドキュメントでは、qni-webgpu プロジェクトでの Claude Code の設定について説明します。

## 設定ファイル構成

Claude Code の設定は複数のレベルで管理されています：

| ファイル | スコープ | 用途 |
|---------|---------|------|
| `~/.claude/settings.json` | グローバル | ユーザー全体の設定（プラグイン有効化など） |
| `.claude/settings.local.json` | プロジェクト | プロジェクト固有の設定（gitignore 推奨） |
| `.mcp.json` | プロジェクト | MCP サーバー定義（git 共有可能） |

## MCP サーバー設定

### .mcp.json

プロジェクトルートの `.mcp.json` で MCP サーバーを定義します：

```json
{
  "mcpServers": {
    "qni": {
      "type": "stdio",
      "command": "node",
      "args": ["/home/yasuhito/Work/qni-webgpu/apps/mcp-qni/src/index.js"],
      "env": {}
    },
    "playwright": {
      "command": "xvfb-run",
      "args": [
        "-d",
        "-s", "-screen 0 1920x1080x24",
        "npx",
        "@playwright/mcp@latest",
        "--config", "/path/to/qni-webgpu/.playwright-mcp/config.json"
      ]
    }
  }
}
```

#### 定義済みサーバー

- **qni**: 量子回路シミュレータ用 MCP サーバー
- **playwright**: ブラウザ自動化用（xvfb + WebGPU 対応）

### .playwright-mcp/config.json

Playwright MCP の設定ファイル。WebGPU を xvfb 上で動作させるための設定：

```json
{
  "browser": {
    "browserName": "chromium",
    "launchOptions": {
      "headless": false,
      "executablePath": "/usr/bin/chromium",
      "args": [
        "--ozone-platform=x11",
        "--enable-features=WebGPU,WebGPUDeveloperFeatures,WebGPUService,Vulkan",
        "--enable-unsafe-webgpu",
        "--enable-dawn-features=allow_unsafe_apis,enable_immediate_error_handling",
        "--ignore-gpu-blocklist",
        "--disable-gpu-sandbox",
        "--no-sandbox",
        "--use-gl=angle",
        "--use-angle=swiftshader",
        "--use-vulkan=swiftshader"
      ]
    }
  }
}
```

#### 重要なフラグ

| フラグ | 説明 |
|--------|------|
| `--ozone-platform=x11` | Wayland 環境でも xvfb の X11 に接続 |
| `--use-angle=swiftshader` | ソフトウェア WebGPU レンダリング |
| `--enable-unsafe-webgpu` | WebGPU を有効化 |
| `headless: false` | 真の headless では WebGPU が動作しないため |

#### なぜ xvfb が必要か

Chrome の WebGPU (ANGLE) は X11 接続が必要なため、`--headless=new` では動作しません。
`xvfb-run` で仮想 X11 サーバーを提供し、`headless: false` でブラウザを起動することで、
物理モニターにウィンドウを表示せずに WebGPU を使用できます。

### .claude/settings.local.json

プロジェクト固有の Claude 設定：

```json
{
  "permissions": {
    "allow": [
      "Bash(git add:*)",
      "Bash(git commit:*)",
      ...
    ]
  },
  "enableAllProjectMcpServers": true,
  "enabledMcpjsonServers": ["qni", "playwright"],
  "enabledPlugins": {
    "playwright@claude-plugins-official": false
  }
}
```

## プラグインの無効化

Claude Code の公式プラグインを無効化するには、`enabledPlugins` で `false` を指定します。

### 重要: プラグイン名の形式

プラグイン名は `プラグイン名@組織名` の形式で指定する必要があります：

```json
{
  "enabledPlugins": {
    "playwright@claude-plugins-official": false
  }
}
```

**よくある間違い:**
```json
{
  "enabledPlugins": {
    "playwright": false  // ← 効かない！
  }
}
```

### プラグイン名の確認方法

グローバル設定で正しいプラグイン名を確認できます：

```bash
cat ~/.claude/settings.json | jq '.enabledPlugins'
```

出力例：
```json
{
  "playwright@claude-plugins-official": true,
  "greptile@greptile": true
}
```

## Ralph での注意事項

### Playwright の重複問題

公式プラグインの Playwright と `.mcp.json` で定義したプロジェクト版 Playwright が同時に動作すると問題が発生します：

1. 公式プラグインは Chrome のインストールを試みる（`sudo` が必要）
2. プロジェクト版はシステムの Chromium を使用

**解決策:** 公式プラグインを無効化し、プロジェクト版のみを使用：

```json
{
  "enabledPlugins": {
    "playwright@claude-plugins-official": false
  },
  "enabledMcpjsonServers": ["qni", "playwright"]
}
```

### MCP サーバーの確認

現在接続されている MCP サーバーを確認：

```bash
claude mcp list
```

期待される出力（公式プラグインが無効化されている場合）：
```
playwright: npx @playwright/mcp@latest --browser chromium --executable-path /usr/bin/chromium - ✓ Connected
qni: node /home/yasuhito/Work/qni-webgpu/apps/mcp-qni/src/index.js - ✓ Connected
```

## トラブルシューティング

### Claude が sudo を実行しようとする

**症状:** Ralph 実行中に `playwright install chrome` で sudo が要求される

**原因:** 公式 Playwright プラグインが有効になっている

**解決:**
1. `.claude/settings.local.json` で `"playwright@claude-plugins-official": false` を設定
2. Claude をリスタート
3. `claude mcp list` で Playwright が1つだけか確認

### MCP サーバーが接続されない

**確認事項:**
1. `.mcp.json` のパスが正しいか
2. `enableAllProjectMcpServers: true` または `enabledMcpjsonServers` にサーバー名があるか
3. コマンドが正しくインストールされているか
