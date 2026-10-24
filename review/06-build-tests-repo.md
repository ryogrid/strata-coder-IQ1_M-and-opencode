# レビュー: ビルド・テスト・リポジトリ hygiene

実測環境: rustc/cargo 1.99.1(README:38 の claims と一致)、tmux 3.4、
2026-10-24、HEAD 7be785e。

## 実測結果サマリ

| コマンド | 結果 |
|----------|------|
| `cargo build` | 成功、`warning:` 0 行(AGENT.md のルールは充足) |
| `cargo test` | 6 テスト全通過(buffer 4、terminal 2) |
| `cargo clippy` | **警告 94 件**(うち 89 件は `--fix` 適用可) |
| `bash test-design/e2e.sh` | 全 68 シナリオ PASS、FAIL 0、exit 0 |

## 中

### R-1. clippy 94 件が放置されている
`cargo build`/`cargo test` は警告ゼロだが(AGENT.md のルール対象)、
clippy は `len_zero`(`x.len() > 0` → `!x.is_empty()`)、
`useless_vec`、`manual_range_contains` などが大量。89 件は自動修正可能で、
`cargo clippy --fix` + 手当てでほぼゼロにできる。
AGENT.md の「warning-free」ルールに clippy を含め、
`[lints.clippy]` を Cargo.toml に宣言して CI で回すのが恒久対策
(edition 2021 + rustc 1.99 なので `[lints]` テーブルは使える)。

### R-2. e2e が debug ビルドのみ・release の挙動差がテストされない
e2e.sh の `BIN="./target/debug/emur"`。debug では panic する u32 溢れ
(02-main.md M-1)が release では wrap するため、**release 固有のバグ
(桁溢れで任意行への移動)は e2e を通過する**。
release ビルドでの e2e スモーク(代表シナリオで可)を推奨。
余談: 現物チェックで `target/release/emur` は 10/18 付の古いバイナリで、
C-l 機能が存在しなかった(`strings` に "Go to line" なし)。
スモークテストはビルド後に限り、`cargo build --release` 済みの
バイナリを direct run しないこと。

### R-3. 単体テストのカバレッジが実装面に対し薄い
`cargo test` の 6 件は buffer の movement/edit 中心。未カバー:
- `window.rs` の layout/split/collapse/flatten(境界行フォールバック等)
- `screen.rs` の split_lines/line_offsets/point_pos(純関数なので容易)
- `highlight.rs` の detect/styles(全面テスト皆無、05-highlight.md H-8)
- `buffer.rs` の find 系・クランプ系(01-buffer.md B-9)
e2e が強いプロジェクトなので unit は補助でよいが、リファクタ(性能修正、
スライス参照化)の安全網として unit が要る。

### R-4. ワークツリーに未整理ファイル
- 未追跡: `run.out`(54KB の ANSI フレームダンプ=デバッグ残骸)、
  `smoke.txt`(75B)、`demo.txt`(0B)。`.gitignore` に
  `run.out`/`smoke.txt` を追加か、削除を推奨。`demo.txt` は
  e2e/スモークが生成する成果物に見える(生成物は git に載せない)。
- `opencode.json` に未コミットの変更(MCP/model の context 設定変更)が
  作業ツリーに残っている。レビュー作業とは無関係なので本レビューの
  commit には含めない。ユーザーが判断して commit すべき。

## 低

### R-5. Cargo.toml のポイント
- `curses = "0.1.0"`: 実質 `get_size()` のためだけの依存(S-4)。
  将来 dependencies を减らす好機(termsize 直接依存にすれば
  README:47 の記述とも一致する)。
- Cargo.lock が commit 済み = バイナリ crate として正しい。good。
- `[profile.release]` は未設定(lto 等)。起動性能には無関係、優先度低。
- version 0.1.0 / edition 2021 は README と整合。

### R-6. CI が無い
`.github/workflows` 等が無く、検証は全てローカル運用。
「build + test + clippy(-D warnings) + e2e(tmux 入りの Linux runner)」
の 1 ジョブだけでも置く価値が高い(e2e は tmux 必須なので runner 要件に明記)。

### R-7. 良好点
- e2e ハーネスの作り(tmux capture-pane ベース、expect/expect_count、
  シナリオ ID 管理、`EMU_EXITED $?` の終了コード確認)が堅実で、
  130+ シナリオ相当が 0 FAIL で回っているのは本プロジェクトの最大の資産。
- `design/TODO.md` の「1 タスク 1 commit」運用と e2e シナリオ ID の
  対応(s120-s124 等)が commit 履歴から追跡可能。good。
- `.gitignore` の `target/` 指定は妥当。AGENT.md のビルドルール
  (警告を `#[allow]` で黙殺しない)は文化として正しい。
