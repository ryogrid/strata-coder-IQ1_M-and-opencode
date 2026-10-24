# emur コードレビュー 総評

- 対象: 2026-10-24 時点の master (HEAD 7be785e) の `src/` 全6ファイルと、
  ビルド・テスト・ドキュメント・リポジトリ hygiene。
- 検証実測済み項目: `cargo build`(警告0) / `cargo test`(6件全通過) /
  `cargo clippy`(警告94件) / e2e ハーネス(tmux 3.4、シナリオ全通過・FAIL 0) /
  EOF 無限ループ再現 / go-to-line オバーフロー panic 再現 / 450KB ファイルの
  描画時間実測(約12.9秒)。

## 全体の評価

小規模ながら gap buffer・二分ウィンドウツリー・ミニバッファ補完・増分検索・
シンタックスハイライトまでを Emacs 風に実装した意欲的なプロジェクトで、
e2e テストの厚み(68シナリオ)とドキュメントの整備度は実に見事。
モジュール分割(buffer/window/screen/terminal/highlight)も責務が明確で読みやすい。

その一方で、**終了系・EOF 系の堅牢性に重大な穴**があり、実運用では端末を
フリーズさせうる。また描画・ハイライトの計算量が全バッファ基準で、
大きいファイルでは実用に耐えない。

## 重大度サマリ

| 重大度 | 件数 | 代表例 |
|--------|------|--------|
| 高 | 4 | EOF 時のミニバッファ無限ループ(生モード漏れを伴うフリーズ)、go-to-line の u32 溢れ panic、ハイライトの O(n²) で 450KB 入力 1フレーム 12.9秒、検索中の矢印キーでゴミ文字挿入 |
| 中 | 10 | EOF 無確認終了による未保存データ喪失、C-x k の無確認破棄、同一ファイル多重オープン、gap buffer の挿入 O(n)、全角幅非対応、raw mode 漏れ(panic/シグナル)、clippy 94 件 他 |
| 低 | 多数 | 各ファイルに列挙 |

## 指摘ファイルの案内

| ファイル | 範囲 |
|----------|------|
| `01-buffer.md` | src/buffer.rs(gap buffer) |
| `02-main.md` | src/main.rs(起動・キーディスパッチ) |
| `03-screen-window.md` | src/screen.rs / src/window.rs(描画・レイアウト) |
| `04-terminal.md` | src/terminal.rs(tty・入力・ミニバッファ) |
| `05-highlight.md` | src/highlight.rs(シンタックスハイライト) |
| `06-build-tests-repo.md` | Cargo / clippy / テスト / リポジトリ hygiene |
| `07-docs-consistency.md` | README.md / design/*.md と実装の整合 |

## 優先対応の推奨(順)

1. **EOF の扱いを一元化する**(04-terminal.md §1)。`read_byte` が EOF で 0 を
   返す設計がミニバッファ側の無限ループを生んでいる。EOF=キャンセル(Emacs の
   C-g 相当)として扱う。
2. **raw mode 復元をDropガード/panic hookで保証する**(02/04)。現状 panic・
   SIGTERM で端末が生モードのまま暴走する。
3. **go-to-line の溢れを `u32::from_str_radix` + 範囲チェックで書く**
   (02-main.md §1)。debug/release で挙動が変わるのも e2e を debug のみで
   回していることと相まって危険。
4. **ハイライトをスライス参照 & 可視行範囲のみの計算にする**(05/03)。
5. CJK 全角幅の扱い(表示セル数の算出)を設計に組み込む。
6. `cargo clippy --fix` で大半の lint を掃除し、clippy をルールの対象に追加する。

## 実測ログ(抜粋)

- `printf 'x\014' | emur` (debug): 3秒で 25.8MB 出力し continue(無限ループ、
  timeout kill)。`printf 'x\003' | emur`: 同様に 35.9MB/3秒。
- `printf '\01499999999999999\n' | emur` (debug): `panicked at src/main.rs:363
  attempt to multiply with overflow`、exit 101、raw mode 復元されず。
- 451KB の .rs ファイル起動時 1 フレーム描画: 実測 12.9 秒。
- e2e: `bash test-design/e2e.sh` → 全 PASS、FAIL 0、exit 0。
