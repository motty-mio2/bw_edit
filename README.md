# bw_edit

`bw_edit` は、chezmoi 等で展開されたローカルの設定ファイルを直接編集した後に、ワンコマンドで Bitwarden 上のアイテムメモ（`notes`）に逆同期（Push）するための CLI ツールです。

## 特徴

- **ファイルヘッダーの自動認識**: ファイル先頭付近のコメント（`# bw_id: <UUID>`, `// bw_id: <UUID>`, `-- bw_id: <UUID>` 等）から Bitwarden アイテム UUID を自動抽出。
- **ファイル全文（ヘッダー含む）の保存**: ヘッダーごと Bitwarden の `.notes` に保存されるため、chezmoi で何度再展開してもヘッダーが失われません。
- **対話的自動アンロック**: Vault がロック中の場合、マスターパスワードの入力を求めて自動で一時アンロックします（`BW_SESSION` が設定済みの場合はそのまま使用）。
- **自動バックアップ**: 上書き直前の Bitwarden 側のメモ内容を `~/.local/state/bw_edit/backups/` に自動退避。
- **Diff & Dry-run**: 変更差分の確認や、実際には書き込まずにテストするフラグを用意。
- **chezmoi template構文対応**: `{{ (bitwarden "item" "<UUID>").notes }}` 構文からも UUID を自動抽出可能。

---

## 使い方

### 1. ファイルの準備

同期したい設定ファイルの先頭付近に、コメントで `bw_id: <UUID>` を記載しておきます。

**SSH Config (`~/.ssh/config.d/hostLN.conf`):**
```sshconfig
# bw_id: e71b158e-c584-4de3-8ae1-b003011c67d4
Host myserver
    HostName 192.168.1.100
    User admin
```

**JSON / JSONC:**
```json
// bw_id: 418f2f48-2d8e-495b-bc44-b17500e9aa1b
{
  "api_key": "secret-key-here"
}
```

**Lua (`~/.config/nvim/...`):**
```lua
-- bw_id: 7540fa95-a318-49a3-b140-b17500e9b87e
return { ... }
```

### 2. 同期コマンドの実行

ファイルを普段のエディタで編集・保存後、以下を実行します。

```bash
bw_edit sync ~/.ssh/config.d/hostLN.conf
```

- Vault がロック中の場合はパスワードが求められ、自動アンロック後に同期されます。
- 同期成功後、直前のメモは `~/.local/state/bw_edit/backups/<UUID>_<timestamp>.bak` に保存されます。

---

## オプション一覧

```bash
bw_edit sync [OPTIONS] <FILE>
```

| オプション | 説明 |
| :--- | :--- |
| `--diff` | 反映前にリモートのメモとローカルファイルの差分（Unified Diff）を表示 |
| `--dry-run` | 実際の書き込みを行わず、差分確認のみ実行 |
| `--id <UUID>` | ファイル内のヘッダーを使わず、明示的にアイテム UUID を指定 |
| `--no-backup` | 上書き前の自動バックアップ作成をスキップ |

### 例

**差分だけ確認する:**
```bash
bw_edit sync ~/.ssh/config.d/hostLN.conf --diff
```

**Dry Run（書き込まずに確認）:**
```bash
bw_edit sync ~/.ssh/config.d/hostLN.conf --dry-run
```

**UUID を直接指定して同期する:**
```bash
bw_edit sync ~/my-config.txt --id e71b158e-c584-4de3-8ae1-b003011c67d4
```

---

## インストール / 更新

```bash
cargo install --path .
```
バイナリは `~/.cargo/bin/bw_edit`（または `~/.local/share/cargo/bin/bw_edit`）に配置されます。
