# bw_edit

`bw_edit` は、chezmoi 等で展開されたローカルの設定ファイルを直接編集した後に、ワンコマンドで Bitwarden 上のアイテムメモ（`notes`）に逆同期（Push）するための CLI ツールです。

## 特徴

- **ヘッダー自動除去（無限増殖の防止）**:
  ファイル先頭の `bw_id: <UUID>` ヘッダーコメント行（`#`, `//`, `--` 等）を自動で抽出・除去し、**本文のみを Bitwarden の `.notes` に保存**します。
  これにより、chezmoi テンプレート側で `# bw_id: <UUID>` を付与して展開している場合でも、同期・展開を繰り返した際にヘッダーが二重・三重に増殖するのを完全に防ぎます。
- **ファイルヘッダーの自動認識**: ファイル先頭付近（15行以内）のコメントから Bitwarden アイテム UUID を自動抽出。
- **対話的自動アンロック**: Vault がロック中の場合、マスターパスワードの入力を求めて自動で一時アンロックします（`BW_SESSION` が環境変数にある場合は自動再利用）。
- **自動バックアップ**: 上書き直前の Bitwarden 側のメモ内容を `~/.local/state/bw_edit/backups/` に自動退避。
- **Diff & Dry-run**: 変更差分の確認や、実際には書き込まずにテストするフラグを用意。
- **chezmoi template構文対応**: `{{ (bitwarden "item" "<UUID>").notes }}` 構文からも UUID を自動抽出可能。

---

## おすすめの運用（chezmoi との組み合わせ）

### 1. chezmoi テンプレートにヘッダーを記述

chezmoi テンプレート（例: `private_3_hostLN.conf.tmpl`）の先頭に、コメントとして `# bw_id: <UUID>` を書いておきます。

```gotemplate
# bw_id: e71b158e-c584-4de3-8ae1-b003011c67d4
{{ (bitwarden "item" "e71b158e-c584-4de3-8ae1-b003011c67d4").notes }}
```

`chezmoi apply` を実行すると、ローカルファイル（`~/.ssh/config.d/3_hostLN.conf`）の先頭に自動で `# bw_id: ...` が配置されます。

### 2. ローカルファイルを普段通り編集

```bash
nvim ~/.ssh/config.d/3_hostLN.conf
```

### 3. Bitwarden に逆同期

```bash
bw_edit sync ~/.ssh/config.d/3_hostLN.conf
```

- `bw_edit` が先頭の `bw_id:` ヘッダーを認識した上で、**ヘッダー行を除去して Bitwarden の notes に保存**します。
- Bitwarden 上の notes は純粋な設定本文のまま保たれ、次回以降 `chezmoi apply` を行ってもヘッダーは1行のまま維持されます（増殖しません）。

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
| `--keep-header` | ヘッダー行を除去せず、ファイル全文をそのまま Bitwarden notes に保存 |

### 例

**差分だけ確認する:**
```bash
bw_edit sync ~/.ssh/config.d/3_hostLN.conf --diff
```

**Dry Run（書き込まずに確認）:**
```bash
bw_edit sync ~/.ssh/config.d/3_hostLN.conf --dry-run
```

---

## インストール / 更新

```bash
cargo install --path .
```
バイナリは `~/.cargo/bin/bw_edit`（または `~/.local/share/cargo/bin/bw_edit`）に配置されます。
