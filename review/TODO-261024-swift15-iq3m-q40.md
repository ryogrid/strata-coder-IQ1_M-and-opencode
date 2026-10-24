# TODO-261024-swift15-iq3m-q40 — emur レビュー対応 進捗管理

- 出典: `review/00-overview.md`〜`review/07-docs-consistency.md`(2026-10-24、HEAD 7be785e)
- ID は出典ファイルの識別子(B=buffer、M=main、S=screen/window、T=terminal、H=highlight、R=repo、D=docs)
- 進捗記号: `[ ]` 未着手 / `[x]` 完了(完了時に commit hash を行末に追記)
- 運用手順:
  - TODO 作成時・Design Doc 作成時・修正コミット前にサブエージェントレビューを経る
  - 1指摘(粒度が小さいものは数個)完了ごとに commit & push
  - 各コミット前に `cargo build` `cargo test`(警告ゼロ・全通過)、挙動を変える修正では `bash test-design/e2e.sh`(現在 72 シナリオ全PASS)を確認
  - 複雑な修正は `docs/design/` に Design Doc を置く(対応関係が分かるファイル名)
  - 学習した内容は末尾「学習記録」に追記する
- Design Doc 対応: A=`docs/design/exit-eof-robustness.md`(T-1,T-2,M-1,T-3,T-4,M-2,M-4,D-1)/ B=`docs/design/perf-highlight-buffer.md`(H-1,S-1,B-1,B-2,H-2,H-3)/ C=`docs/design/cjk-display-width.md`(S-2,T-7)

## Phase 1: 高優先(High)— 実測で再現確認済み

- [x] **T-1**: `read_line_c` の EOF 無限ループ(NUL 無限挿入。実測3秒で25MB出力)。EOF を「キャンセル」として区別返却。受入条件: 全ミニバッファ経路(C-r/C-l/C-x C-f/C-w/C-i/C-x b/C-x C-b/終了確認)で EOF ハング解消、`restore_mode` 到達。04 §T-1 / Design Doc A → fix #1 で解消確認(rc=0・4.4KB。e2e s140/s142 追加)
- [x] **T-2**: `read_char_from` が EOF 時に NUL/空白を積む。EOF で挿入中止。04 §T-2 / Design Doc A → `read_char_eof` 化で対応(fix #1)。C-q+EOF の NUL 1文字のみ既知の限界(コメント明記)
- [x] **M-2**: 終了確認(confirm_exit)中の EOF は無限ループ(実測35MB/3秒)。EOF=キャンセル(確認は終了しない)+メインループ EOF の無条件終了(§M-4 方針)で終了する旨を実装・コメント・README に明記。02 §M-2 / Design Doc A → 実装は「EOF なら直接 true(=終了)」の一歩案(Design §2.3 の理由付き採用、挙動は同一)
- [x] **M-1**: go-to-line の u32 溢れ panic(実測 exit 101)。`parse` + バッファ行数へのクランプに置換(debug/release 挙動差=R-2 も参照)。02 §M-1 / Design Doc A → デジタルゲート+u64 parse+総行数バウンドでクランプ維持。e2e s141 追加
- [x] **T-3**: raw mode 復元を Drop ガード+panic hook+シグナルハンドラで保証。04 §T-3 / Design Doc A → TtyGuard Drop + static AtomicPtr + SIGTERM/HUP/INT/QUIT(pty 実測: exit 143 で復元)
- [x] **M-3**: C-s 増分検索中の矢印キー ESC accept 後、残 `[A` が self-insert される。02 §M-3 → C-s ループ内で ESC 直後の `[<ABCD>` を丸ごと消費し、accept 後に矢印として移動するよう実装(Emacs isearch 相当)。e2e s143 追加(fix #3)
- [x] **H-1**: `heq_at` の全文 clone で styles が O(n²)(451KB 1フレーム実測12.9秒)。スライス参照化。受入目標: 451KB .rs で 1フレーム < 1秒。05 §H-1 / Design Doc B → 全関数スライス化。受入ビルド=debug で 15.3s→0.122s(約10倍の余裕。fix #2)
- [x] **S-1**: draw_all が全バッファを毎フレーム複数回走査+全面 print。text() 1回化+styles 1回化(可視行範囲 styles と差分描画は Design Doc B でスコープ判断)。03 §S-1 / Design Doc B → BufView フレームキャッシュ(text()/lines/offs/toks をバッファ毎1回、窓共有時1回)。可視範囲限定・差分描画はスコープ外と明記(fix #2)

## Phase 2: 中優先(Medium)

- [x] **B-1**: gap buffer が gap を前確保せず挿入 O(n)。new() で gap 預け插入 O(1) 償却。01 §B-1 / Design Doc B → 末尾にサイズ GAP=8192 前確保+枯渇時 splice で in-place 再充填(fix #2)。潜行バグ insert_newline_after の gap_start += 1 欠落(F-4)も同時修正・テスト固定
- [x] **B-2**: find_forward/backward が毎回全文 text() 生成。char_at 直接スキャンへ。01 §B-2 / Design Doc B → Option<usize> 戻り値で直接スキャン(fix #2)
- [ ] **B-3**: kill_to_end_of_line が mark 後方(同一行)で無音 no-op。後方 mark は領域なし扱いへ。01 §B-3
- [x] **M-4**: メインループ EOF が未保存バッファを無警告破棄。方針決定(EOF=終了据え置き+dirty 時のメッセージ/文書化)。02 §M-4 / Design Doc A → 方針確定・README/keybindings に明記(fix #1)。確認プロンプトは付けない(stdin 閉鎖後は確認不能が本質)
- [ ] **M-5**: C-x k が modified バッファを無確認で空にする。確認プロンプト追加。02 §M-5
- [ ] **M-6**: C-x C-f が同一ファイルを毎回新規バッファ化。既存バッファ再利用。02 §M-6
- [ ] **M-7**: 未バインドの C-x 後続が無言。default で "Quit" 相当。02 §M-7
- [ ] **M-8**: C-x C-w が書き出し失敗後も filename を設定。成功時のみ設定。02 §M-8
- [ ] **M-9**: read_file が全エラーを無言(空表示)。開けない時のステータスメッセージ。02 §M-9
- [ ] **M-10**: C-l が next_line を行数回繰り返す O(n×行長)。1回スキャンへ。02 §M-10
- [ ] **S-2**: 全角(CJK)を幅1として扱う。セル数基準化。03 §S-2 / Design Doc C
- [ ] **S-3**: split/collapse で tree のスロットが単調増加(未回收)。03 §S-3
- [x] **T-4**: read(2) エラーを EOF と同一視。Err では再試行ループとし Ok(0) をもって EOF とする(std が EINTR を再試行する事実を確認したらコメントで明文化)。04 §T-4 / Design Doc A → 再試行ループ実装。実測: std の StdinLock::read は EINTR を自動再試行しない(Err(Interrupted) を返す)ため explicit ループが本体。コメント明記済み(fix #1)
- [x] **H-2**: Rust のライフタイム `'a` を文字列として誤色。lang1 の `'` 処理改善。05 §H-2 / Design Doc B → 3条件(同一行閉じ/j>=i+2/内容が string 的)採用。フィクスチャ `fn f<'de>(x: &'de str) {}` をテストで固定(fix #2)
- [x] **H-3**: detect が拡張子の大文字小文字を区別しない(FILE.RS 無装飾)。05 §H-3 / Design Doc B → ASCII 小文字化比較+fizz マトリクステスト(fix #2)
- [ ] **R-1**: clippy 94 件。`cargo clippy --fix` + 手当てで掃除。完了ゲート: `cargo clippy` 警告0行(AGENT.md に clippy ルール追記)。06 §R-1
- [ ] **R-2**: e2e が debug のみ。release での代表スモーク追加。06 §R-2
- [ ] **R-3**: window/screen の単体テスト追加(buffer=find 系は B-9、highlight は H-8 が担当)。06 §R-3
- [ ] **R-4**: run.out/smoke.txt/demo.txt 未整理。.gitignore 追記。06 §R-4
- [x] **D-1**: README:148・keybindings.md:13「EOF 無条件終了」の記述を実装に合わせ修正(Phase 1 の EOF 実装修正と同時に実施、完了記録は当該コミット)。07 §D-1 / Design Doc A → README 本文+キー表+keybindings.md 更新済み(fix #1)
- [ ] **D-2**: design/README.md の "Not implemented" 列表陳腐化(syntax highlighting 等)。07 §D-2
- [ ] **D-3**: architecture.md の4点(gap O(1)/keymap tree/curses 描画/status top)を実態に。07 §D-3
- [ ] **D-4**: README:129「全窓が同一バッファ表示」は C-x b 後誤り。07 §D-4
- [ ] **D-5**: design/keybindings.md の C-</C-> → M-</M->。07 §D-5
- [ ] **D-7**: README:47 の termsize 表記(直接依存は curses)。07 §D-7
- [ ] **D-10**: README 環境記述(モデルID)が opencode.json と二重管理。分離検討。07 §D-10

## Phase 3: 低優先(Low)

- [x] **B-4**: find 系/`find_buffer` の `Vec<usize>` 戻り値を `Option<usize>` へ。01 §B-4 → find 系は fix #2 で先取り解消(main.rs 4 箇所 Option 化)。`find_buffer`(main.rs:21)は「一致列」を返す別関数のため `Vec<usize>` のまま(Design Doc B に方針明記)
- [ ] **B-5**: transpose_chars が点=0で先頭2文字を swap(Emacs は no-op)。01 §B-5
- [ ] **B-6**: 「編集で領域が消える(mark 追従)」を README に明文化。01 §B-6
- [ ] **B-9**: buffer の find/クランプ/move_gap 単体テスト追加。01 §B-9 → (部分)find/gap/move_gap(挿入経路)/insert_newline_after は fix #2 で追加済。クランプ系は e2e s112/s141 が担保。残り: line_start/line_end 境界の unit
- [ ] **B-10**: `self: &Buffer` → `&self`、String 引数 → `&str`/`Into<String>`。01 §B-10
- [ ] **M-11**: 300行ディスパッチをテーブル化(architecture の keymap tree 記述と共に対応)。02 §M-11
- [ ] **M-12**: prev_cmd は last-command 相当に限定(現状文書と整合、注記のみ)。02 §M-12
- [ ] **M-13**: leaf/bufs 間の point 書写しをヘルパー集約。02 §M-13
- [ ] **M-14**: .to_string()/clone 乱用を API 変更で削減。02 §M-14
- [ ] **S-5**: Frame.kind を enum 化(境界 Frame の leaf ダミー除去)。03 §S-5
- [ ] **S-6**: scroll 再計算の不変条件をコメント化。03 §S-6
- [ ] **S-7**: 分割時カーソルと status clamp の関係コメント。03 §S-7
- [ ] **S-8**: find_slot センチネルを Option<usize> へ。03 §S-8
- [ ] **S-9**: Node.vertical の命名/コメント改善。03 §S-9
- [ ] **S-10**: point_pos の同一フレーム2回呼びを1回化。03 §S-10
- [ ] **S-12**: レイアウト重なりデジェネレートのコメント。03 §S-12
- [ ] **T-5**: fd0 固定の設計注記(stdin 非 tty で raw mode なし)。04 §T-5
- [ ] **T-6**: Tab 補完 0 候補時に bell。04 §T-6
- [ ] **T-7**: list_width/cand_at の当たり判定が文字数基準(CJK 候補でズレ)。セル数基準へ(Design Doc C と同時)。04 §T-7
- [ ] **T-8**: file_candidates: ディレクトリ末尾 `/`、ソート、`~` 展開。04 §T-8
- [ ] **T-9**: VMIN/VTIME 明示設定。04 §T-9
- [ ] **T-11**: print+flush 出力ヘルパー集約。04 §T-11
- [ ] **H-4**: md_styles 未閉結バッククォートのテスト追加。05 §H-4
- [ ] **H-5**: 数値スキャン過装飾(2hex)の軽微修正/注記。05 §H-5
- [ ] **H-6**: HTML タグ内属性文字列の装飾(注記)。05 §H-6
- [ ] **H-7**: スタイルコードを enum 化。05 §H-7
- [x] **H-8**: detect/styles の単体テスト追加。05 §H-8 → fix #2 で追加(detect 拡張子マトリクス、lifetime フィクスチャ、string/char リテラル、json、md)
- [ ] **R-5**: curses 依存縮小(size 検出のみ)/[profile] 注記。06 §R-5
- [ ] **R-6**: CI 追加(build+test+clippy+e2e)。06 §R-6
- [ ] **D-6**: keybindings.md の C-d 二重掲載・C-\ 凡例削除。07 §D-6
- [ ] **D-8**: C-s/C-r/C-l minibuffer は補完なしの明記。07 §D-8
- [ ] **D-9**: README:79 に「全読み込みエラーで空で開く」注記(M-9 と同時)。07 §D-9

## 確認済み良好点(対応不要・記録)

- B-7: insert_newline_after の gap 操作は追跡上正しい(テスト済)。
- B-8: move_gap/to_index の整合は正しい(不変条件のコメント化は S/B 系で実施)。
- M-15: NUL と EOF の区別(read_byte_eof)、終了確認 yes/y、C-s wrap/bell/C-g 復元は README 通り。
- S-11: ステータス行フォーマット・pct は README と一致、空ファイル 0 除算なし。
- S-4/README 注記: curses は size 検出のみ(実装はコメントと一致、表記は D-7)。
- T-10: read_line_c の候補リスト状態管理と単体テストは丁寧。
- R-7: e2e ハーネス・TODO 運用・.gitignore は良好。
- H-9: highlight の doc-comment は正確。キーワード表の言語分離は Low。

## 学習記録(作業中に追記)

- **EOF 設計**: 「EOF=入力の終わり」を文脈別に固定。ミニバッファ=キャンセル、
  メインループ=無条件終了(未保存データ喪失は仕様と明記)、終了確認=EOF なら
  直接終了(閉じた stdin に確認は取れない)。`read_byte()` の EOF=0 は
  Ctrl-space(NUL)と区別不能なので、ループは必ず `read_byte_eof()` を使う。
- **C-q+EOF の限界**: 先頭バイトが 0 で来るため C-q では NUL が 1 文字挿入される
  (read_char_eof は continuation 用の API なので先行できない)。バウンド済み
  (直後にメインループ EOF で終了)なのでコメント明記で許容。
- **raw mode 復元**: TtyGuard の Drop(unwind 含む)+ static AtomicPtr スロット
  経由のシグナルハンドラ(SIGTERM/HUP/INT/QUIT → tcsetattr → _exit(128+sig))。
  復元順序は「スロット登録(save_raw_mode 内)→ ハンドラ設置(main)」。
  SIGKILL と「raw 設定直後〜ハンドラ設置前の窓」のリークは設計上の到達限界。
- **exit code の検証は pty で**: tmux capture-pane 経由だと shell ラッパが exit
  code を消す。`pty.fork` で直接 SIGTERM を送り WEXITSTATUS=143 を確認する手法。
- **std::io::StdinLock::read は EINTR を自動再試行しない**(Err(Interrupted) を
  返す)。T-4 の再試行ループは必須。
- **debug/release の溢れ挙動差**: u32 溢れは debug=panic、release=wrap。
  e2e が debug のため release 系バグが素通り(R-2 で解消)。
- **e2e.sh の `expect_lastline` は完全一致([ = ])比較**。regex エスケープ
  (`\*`, `\(`)は grep 系 `expect`/`expect_e` 専用。expect_lastline に正規表現
  表記を書くと必ず FAIL(s141 で踏んだ。s112 等が素の表記)。
- **Rust 1.99 `function_casts_as_integer`**: 関数ポインタを `sighandler_t` へ
  直接 cast すると警告。`as *const () as libc::sighandler_t` を経由する。
- **ディスク逼迫(根 FS 100%)はツール破損として現れる**: write ツールが
  意味不明に失敗、shell ツールが空出力+非ゼロ終了になる。ENOSPC を見たら
  `cargo clean` と `go clean -cache`(~/.cache/go-build が 103GB)で回復。
  今回 113GB 空きを確保。
- **clippy 件数 94→101**: fix #1 の新代码は既存 lint カテゴリ
  (needless_return 等)を増やしただけ。R-1 で一括掃除するまで許容閾値とみなす。
- **gap 前確保の設計ポイント(fix #2)**: `new()` は末尾に**サイズ GAP** の窓
  (`gap_start=n; gap_end=n+GAP`)を置く。`gap_start==gap_end`(サイズ0)は
  len() が過大になり不正。挿入で枯渇したら `Vec::splice(gs..gs, repeat_n('\0', GAP))
  で**挿入地点に**窓を開ける(末尾 resize では挿入地点の窓にならない。
  1文字 Vec::insert を GAP 回繰り返すのは O(n·GAP))。`chars.len()-len()`=窓サイズ
  が常に成り立ち、テストの不変条件に使える(text()/len() は常に正)。
- **潜行バグ(F-4)**: `insert_newline_after` の旧 Vec::insert 分岐は
  `gap_start += 1` 欠落で挿入文字が gap に呑まれ、fresh load 直後の C-o が
  無音 no-op だった(既存テストは先行 delete で gap 非空のため素通り)。
  「gap がある前提の分岐」を新設するときは**全経路の窓整合**を見る。
- **性能ゲートの実測(debug)**: 451KB .rs 初回フレーム 15.3s → 0.122s。
  100KB=0.043s、342KB CJK=0.084s。`</dev/null` 起動は初回フレーム描画→EOF 終了なので
  `time` がそのまま 1 フレーム計測になる(fix #1 の EOF 終了に依存)。
- **ゲートレビューが証言のズレを検出**: fix #2 のゲートで「C-y も insert_string 化と
  書いてあるがコードは未移行」を Major 指摘された。**コミットメッセージ/証言は
  コードと突き合わせてから書く**(Design Doc §2.4 の使用者2名のうち C-x C-i だけ置換した)。
  実害はなかった(償却 O(1) のループだった)が、設計準拠と証言精度の問題。
- **octal 罠**: `printf '\021'` は octal 21=0x11(C-q)。C-y=0x19 は `\031`。
  パイプ再現テストのキーは 16 進換算を必ず確認する(C-y 検証が最初 C-q になっていた)。
  同じ系列で e2e s143 も `\022`(C-r)で書いてしまった。**C-s=0x13=\023**。
- **e2e アサーションの算術**: パーセンテージは point*100/buf.len()。demo バッファは
  len=43(22+1+19+1、末尾改行で 3 行目=空)。期待値は実測から逆算せず len から
  計算して書く(s143 で 85% と書き、実測 83% = 36*100/43 が正)。
- **e2e.sh のヘルパは定義位置に注意**: `expect_lastline` は ~390 行目、`run_pipe` は
  ~47 行目。スニペットを切り出して単体実行するとき「先頭 52 行+関数」では
  expect_lastline が未定義で全 FAIL になる(製品バグと誤読した)。probe には
  必要ヘルパを明示的に足す。
- **M-3 の設計**: ESC は C-s ループの受理キーなので、ESC を受け取ったら**直後の
  バイトも自分のループで読む**(未消費のまま break すると main ループが
  `[`/`B` を printable として self-insert する)。ESC 直後が `[` でない場合は
  1 バイトだけ飲んで accept(メインループの Meta 接頭辞読みと同じブロッキング性)。
  受理後の移動(Up/Down/Left/Right)は Emacs の isearch が矢印で確定+移動することに倣う。
- **clippy 実測ベースライン**: HEAD=109 → fix #2 作業ツリー=118(ゲート計測)。
  新規カテゴリ(manual_repeat_n/partialeq_to_none/unnecessary_cast/ptr_arg×2)は
  ドライブバイ修正済み(100 に減少)。R-1 の対象として計上。
- **e2e s21 の Flake 根本原因(既存)**: 単発キャプチャが**描画と競合**する
  (C-x 3 適用前の 2 窓状態、またはフレーム先頭の screen_clear 直後に下窓が
  未描画)。`expect_occ`/`expect_count` 系(件数アサート)は分割直後の
  キャプチャに弱い。※行結合(|)でトークンが消える説は誤り(結合行は
  `grep -oE` で 2 件数える)とゲートレビューで確定。
  境界行のダッシュ数は幅=cols(80)で固定長アサート(`^-{79}$`)は不可。
  置換したアサート(左窓 2 行+境界列 glue 行+ダッシュ行、3 回ポーリング)は
  単体 3/3 安定。

## 変更履歴

- 2026-10-24: TODO 作成(review/00-07 から整理)。内訳 高8/中26/低32+良好点記録8。
- 2026-10-24: サブエージェントレビュー(REJECT)対応 — T-7 チェックボックス追加(Blocker#1)、
  M-2 の EOF 指示を「キャンセル」に訂正(Blocker#10、データ喪失を招く逆指示を解消)、
  T-1/M-1/H-1/S-1/R-1/R-3/T-4/D-1 の受け入れ条件・スコープ・担当分離を明記、件数を実測値に訂正。
- 2026-10-24: Design Doc A 作成(`docs/design/exit-eof-robustness.md`、コミット eb95b5a)。
  サブエージェントレビュー APPROVE-WITH-CHANGES → Blocker#1(M-1 のハング)を
  「移動ループを総行数に束縛(クランプ維持)」で解消する版に置換して反映。
- 2026-10-24: **fix #1(コミット 4e461ec)**: T-1/T-2/M-1/M-2/M-4/T-3/T-4/D-1 実装。
  コミット前ゲートレビュー APPROVE(C-q EOF の NUL 限界コメント、C-x C-c 行 EOF 記述を反映)。
  e2e に s140/s141/s142 追加(パイプラナー rc+サイズ+内容アサート)。
  実施中の追加対応: e2e s21 が Flake(描画競合。境界行幅=cols=80)→ 明示レイアウト
  アサート+ポーリングに置換(単体 3/3 通過)。学習記録に追加。
- 2026-10-24: Design Doc B 作成(`docs/design/perf-highlight-buffer.md`、コミット e9c83e6)。
  ゲートレビュー APPROVE-WITH-CHANGES 反映: gap 窓は「サイズ GAP」で前確保(F-1)、
  再充填は挿入地点 in-place splice(F-2)、insert_newline_after の潜行バグ F-4 をスコープ化、
  H-2 は 3 条件+フィクスチャ固定(F-5)、受け入れビルド=debug 明記(F-6)、
  メモリ/残存コスト/padding 不変条件/CJK 計測の注記追加。
- 2026-10-24: **fix #2(コミット 606a080)**: H-1/S-1/B-1/B-2/H-2/H-3/B-4(find 系)+B-9 部分
  +H-8 完了。コミット前ゲート APPROVE-WITH-CHANGES → C-y の insert_string 化漏れ(Major)を
  反映+新規 clippy カテゴリをドライブバイ修正。性能: 451KB debug 15.3s→0.122s。
  cargo test 16/16、e2e 71/71、ビルド警告ゼロ。Design Doc A の clamp 記述整合を
  docs 追記コミット(f0fd5aa)で反映。
- 2026-10-24: **fix #3(このコミット)**: M-3(C-s 中の矢印キーで `[B` ゴミ self-insert)。
  ESC 受理時に矢印シーケンスを消費+移動。e2e s143 追加。Phase 1 の High 全項目完了。
  (fix #3 のコミットハッシュは次エントリに記載)
