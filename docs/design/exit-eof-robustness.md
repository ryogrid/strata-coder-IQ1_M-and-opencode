# Design Doc A: EOF・終了系の堅牢化

対象 TODO タスク: **T-1, T-2, M-2, M-4(方針), T-3, T-4, M-1, D-1**
出典: `review/02-main.md`、`review/04-terminal.md`、`review/07-docs-consistency.md`
状態: 設計(実装前サブエージェントレビュー対象)

## 1. 現状の問題(実測済み)

| ID | 症状 |
|----|------|
| T-1 | `read_byte()` が EOF で 0 を返し、`read_line_c` が NUL を無限挿入し続ける(3秒で25MB)。全ミニバッファが EOF でハング |
| T-2 | `read_char_from` が continuation byte を EOF で 0 補完し NUL/空白化 |
| M-2 | 終了確認プロンプト中の EOF で無限ループ(35MB/3秒)。`restore_mode` 到達不能 |
| T-3 | raw mode 復元が `main` 正常リターン経路のみ。panic/SIGTERM で端末が生モード漏れ |
| M-1 | go-to-line の u32 溢れで debug ビルドが panic(T-3 と結合して端末を壊す) |
| T-4 | read(2) エラーを EOF と同一視(EINTR で誤終了しうる) |
| D-1 | README:148 の「EOF は無条件に終了」がミニバッファ経路と矛盾 |

## 2. 方針(制御の中心)

**EOF = 「入力の終わり」。コマンド文脈ごとに意味を決めて一元化する。**

1. **ミニバッファ入力中の EOF = そのプロンプトのキャンセル**。
   呼び出し側の既存「Cancelled」経路(`""` 相当)に乗せる。
   ただし EOF であることは型(`Option<String>`)で区別し、終了確認だけが
   EOF を明示的に「終了」の意味として扱う。
2. **メインループ(キー読み取り)中の EOF = 無条件終了(据え置き)**。
   e2e・スモークが「stdin 閉鎖で終了」に依存しているため変更しない。
   未保存データは失われる — 仕様として README に明記する(M-4 方針決定)。
3. 結果として「終了確認中に EOF」は: `confirm_exit` が EOF(`None`)を `true`
   (=終了)として直接返す **一歩で終了**する。README が謳う「EOF で無条件終了」
   と同じ挙動を一義的に実現し、M-2 のハングは消える。
   (メインループの次の読み取りも EOF なので、キャンセル扱いでも結果は同じだが、
   実装は直接返却の一歩とする。)

## 3. 実装設計

### 3.1 terminal.rs

- `read_line_c(prompt, cands, file_comp) -> Option<String>`:
  ループ内の `read_byte()` → `read_byte_eof()` に変更。`rb[1] == 1`(EOF)なら
  `None` を返す(プロンプト行はそのまま。次のフレームで消える)。
- ラッパ(`None → ""` に潰す):
  - `read_line(prompt) -> String`
  - `read_line_buffers(prompt, names) -> String`
  - `read_line_files(prompt) -> String`
  - `read_line_eof(prompt, cands, file_comp) -> Option<String>` を公開
    (`confirm_exit` 専用)。
- `read_char_from(first) -> char` → `read_char_eof(first) -> Option<char>`:
  continuation byte の読み取りに `read_byte_eof` を使い、EOF なら `None`。
  呼び出し側(main.rs self-insert UTF-8・C-q・C-s ループ・read_line_c printable)は
  `None` のとき挿入しない / 入力を打ち切る(read_line_c は `None` を返す)。
- `TtyGuard`: `save_raw_mode() -> TtyGuard`(内部に `Option<Termios>`)。
  `impl Drop` は `restore_mode(self.saved)` を呼ぶ(**`restore_mode` を呼ばれたまま
  残すこと** — 呼び出し元が消えると dead_code 警告で警告ゼロゲートを割る)。
  `main` は `let _g = terminal::save_raw_mode();` と**名前付きで**保持
  (`let _ =` は即破棄なので不可)。末尾の `restore_mode` 呼び出しは削除。
  panic の unwind でも Drop が走る。
  `save_raw_mode` は復元対象 Termios を静的スロット(§3.2 シグナル参照)にも
  登録する(登録はガード生成時。シグナルハンドラ設置より先にスロットが埋まる)。
- `read_byte_eof`: `Ok(1) => [b,0]`、`Ok(0) => [0,1](EOF)`、
  `Err(Interrupted) => continue(再試行)`、`Err(_) => [0,1](EOF 扱い)`。
  Rust std が EINTR を内部再試行する事実はコメントで明記(T-4)。
- `read_line_c` 内の矢印シーケンス副読み(ESC 分岐の `read_byte`)は
  **意図的に `read_byte` のまま**(EOF→0 は `[` に一致せず、ループ先頭の
  `read_byte_eof` が `None` を返すのでハングしない)。実装時に統一しないこと。

### 3.2 main.rs

- `confirm_exit`: `read_line_eof` を使用。`None`(EOF)→ `true`(=終了)を返す。
  コメント: 「EOF では確認できないため終了する。未保存データは失われる(M-4 仕様)」。
- go-to-line(M-1): 手動ループ(`want_ln * 10 + digit`)を以下に置換:
  - `q.bytes().all(|c| c.is_ascii_digit())` で弾く(先頭 `+` は `parse` が
    通すため、**デジタルゲートを parse 前に置く**。現状の digit ループと同等の
    受理集合を保つ)。
  - `q.parse::<u64>()`(溢れは Err)。`want_ln == 0` も Invalid。
  - **総行数を 1 回の O(n) スキャンで数え**、移動ループの上限を
    `min(want_ln, 総行数)` にする。**大きな数は最終行へクランプ**(README/
    design/keybindings.md/README キー表・e2e s112 が文書・実測で clamp を
    検証済み。Emacs の goto-line も clamp。よって Invalid にしない)。
    ループは総行数以下で停止保証あり。1行ずつ移動の単一スキャン化は M-10。
    (レビュー Blocker#1 の「ハングする」指摘は妥当。採用した解消方法は
    Invalid 化ではなく**ループ上限を行数に束縛**する方で、既存 clamp 仕様・
    s112・README を保持する。)
- self-insert UTF-8 / C-q: `read_char_eof` が `None` なら挿入しない。
- パニックフック: `save_raw_mode` 前に `std::panic::set_hook` で
  「emur panicked — restoring terminal」+ 位置情報を stderr へ(復元本体は
  Drop。フックは可視化)。
- シグナル(SIGTERM/SIGHUP/SIGINT/SIGQUIT): Cargo.toml に `libc = "0.2"` 追加
  (termios 経由でビルド済みの推移依存なので実コストなし)。復元対象 Termios は
  静的スロット `static AtomicPtr<Termios>` に登録(`Box::leak` で 1 回だけ leaks —
  进程寿命ぶんのみ、無害)。ハンドラはスロットの `tcsetattr` 適用後 `_exit(128+sig)`。
  **設置順序**: `save_raw_mode()` がガード生成時にスロット登録 → `main` はガード
  取得後に panic フックとシグナルハンドラを設置(順序を逆にした場合、設置窓の
  シグナルでスロット未登録=raw mode 漏れとなる)。
  TtyGuard の Drop とハンドラは同じスロットから復元する(冪等)。
  補足: raw mode(ISIG クリア)では SIGINT/SIGQUIT は tty から生成されないため、
  ハンドラは外部 `kill` のみ対象(コメントで明記。Ctrl-C で検証しようとしないこと)。

### 3.3 ドキュメント(D-1、M-4)

- README:146-148 / キーバインド表 `C-x C-c, C-c` 行 / design/keybindings.md:12-13 /
  **main.rs:32-35 の `confirm_exit` コメント**(「EOF exits unconditionally」の
  正確化)を、
  「キー読み取り中の EOF は無条件終了(**未保存変更は失われる。確認は出ない**)。
  ミニバッファ入力中の EOF はそのコマンドをキャンセルする」に更新。

## 4. テスト計画

- 手動(再現の逆転確認):
  1. `printf 'x\003' | emur` → 数秒以内に終了(無限出力なし)。
  2. `printf 'x\01499999999999999\n' | emur` → panic せず終了。
  3. `printf '\01412\n' | emur` → go-to-line 回帰なし。
- e2e 追加(`test-design/e2e.sh`):**パイプ系は tmux を使わない別ランナー**を新設する
  (`report()` は tmux セッションを kill するため既存ランナーでは不可)。
  パイプ・シナリオは `printf ... | timeout 10 ./target/debug/emur > out; rc=$?` を実行し、
  `rc == 0`(`timeout` の 124 は FAIL)**かつ `wc -c out` が上限(1MB)未満**を assert する
  (終了コードだけでは「3 秒で 35MB 流出後に終了」する旧バグを捕まえられない)。
  新規ランナーのシナリオは e2e.sh の末尾明示ランナーリストに追加する。
  - **s140**: `printf 'x\003' | timeout 10 emur` → rc 0 + 出力 < 1MB(ハング・流出なし)。
  - **s141**(既存 tmux ランナー): `C-l` + `999999999999999` + Enter → status は
    最終行にクランプ(`Buf *demo* 3/3 (100%)`)、エディタ生存・panic なし
    (ハングなしを時間内に判定 = ステータスが status 形式であること)。
    その後の `Z` 入力が効くことで生存を確認。
  - **s142**: `printf '\022abc' | timeout 10 emur`(Enter なし EOF)→ rc 0、
    出力に `Cancelled` を含む(EOF で C-r がキャンセルされ、ステータスメッセージ
    付きフレームが 1 枚描画されて終了する)。
- `cargo test` 全通過 + `cargo build` 警告ゼロ + 既存 e2e 68 シナリオ全 PASS。

## 5. リスクと据え置き事項

- `C-x C-b` の Enter 巡回: EOF → `""` → 巡回 1 回 → メインループ EOF で終了。
  セッション終了操作として受け入れる(文書化)。
- `libc` 依存追加は Cargo.lock を更新する。Linux 専用前提でリスクなし。
- シグナルハンドラ内は async-signal-safe な syscall(tcsetattr / _exit)のみ使う。
- panic=abort は導入しない(unwind に依存)。
- SIGKILL は捕捉不能(復元されず raw mode 漏れうる)。本設計の到達限界として明記。
- M-4「EOF で未保存データ喪失」は仕様として明記。確認プロンプトは追加しない
  (stdin 閉鎖後は確認が本質的に不可能)。
