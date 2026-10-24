# TODO-261024-swift15-iq3m-q40 — emur レビュー対応 進捗管理

- 出典: `review/00-overview.md`〜`review/07-docs-consistency.md`(2026-10-24、HEAD 7be785e)
- ID は出典ファイルの識別子(B=buffer、M=main、S=screen/window、T=terminal、H=highlight、R=repo、D=docs)
- 進捗記号: `[ ]` 未着手 / `[x]` 完了(完了時に commit hash を行末に追記)
- 運用手順:
  - TODO 作成時・Design Doc 作成時・修正コミット前にサブエージェントレビューを経る
  - 1指摘(粒度が小さいものは数個)完了ごとに commit & push
  - 各コミット前に `cargo build` `cargo test`(警告ゼロ・全通過)、挙動を変える修正では `bash test-design/e2e.sh`(68シナリオ全PASS)を確認
  - 複雑な修正は `docs/design/` に Design Doc を置く(対応関係が分かるファイル名)
  - 学習した内容は末尾「学習記録」に追記する
- Design Doc 対応: A=`docs/design/exit-eof-robustness.md`(T-1,T-2,M-1,T-3,T-4,M-2,M-4,D-1)/ B=`docs/design/perf-highlight-buffer.md`(H-1,S-1,B-1,B-2,H-2,H-3)/ C=`docs/design/cjk-display-width.md`(S-2,T-7)

## Phase 1: 高優先(High)— 実測で再現確認済み

- [x] **T-1**: `read_line_c` の EOF 無限ループ(NUL 無限挿入。実測3秒で25MB出力)。EOF を「キャンセル」として区別返却。受入条件: 全ミニバッファ経路(C-r/C-l/C-x C-f/C-w/C-i/C-x b/C-x C-b/終了確認)で EOF ハング解消、`restore_mode` 到達。04 §T-1 / Design Doc A → fix #1 で解消確認(rc=0・4.4KB。e2e s140/s142 追加)
- [x] **T-2**: `read_char_from` が EOF 時に NUL/空白を積む。EOF で挿入中止。04 §T-2 / Design Doc A → `read_char_eof` 化で対応(fix #1)。C-q+EOF の NUL 1文字のみ既知の限界(コメント明記)
- [x] **M-2**: 終了確認(confirm_exit)中の EOF は無限ループ(実測35MB/3秒)。EOF=キャンセル(確認は終了しない)+メインループ EOF の無条件終了(§M-4 方針)で終了する旨を実装・コメント・README に明記。02 §M-2 / Design Doc A → 実装は「EOF なら直接 true(=終了)」の一歩案(Design §2.3 の理由付き採用、挙動は同一)
- [x] **M-1**: go-to-line の u32 溢れ panic(実測 exit 101)。`parse` + バッファ行数へのクランプに置換(debug/release 挙動差=R-2 も参照)。02 §M-1 / Design Doc A → デジタルゲート+u64 parse+総行数バウンドでクランプ維持。e2e s141 追加
- [x] **T-3**: raw mode 復元を Drop ガード+panic hook+シグナルハンドラで保証。04 §T-3 / Design Doc A → TtyGuard Drop + static AtomicPtr + SIGTERM/HUP/INT/QUIT(pty 実測: exit 143 で復元)
- [ ] **M-3**: C-s 増分検索中の矢印キー ESC accept 後、残 `[A` が self-insert される。02 §M-3
- [ ] **H-1**: `heq_at` の全文 clone で styles が O(n²)(451KB 1フレーム実測12.9秒)。スライス参照化。受入目標: 451KB .rs で 1フレーム < 1秒。05 §H-1 / Design Doc B
- [ ] **S-1**: draw_all が全バッファを毎フレーム複数回走査+全面 print。text() 1回化+styles 1回化(可視行範囲 styles と差分描画は Design Doc B でスコープ判断)。03 §S-1 / Design Doc B

## Phase 2: 中優先(Medium)

- [ ] **B-1**: gap buffer が gap を前確保せず挿入 O(n)。new() で gap 預け插入 O(1) 償却。01 §B-1 / Design Doc B
- [ ] **B-2**: find_forward/backward が毎回全文 text() 生成。char_at 直接スキャンへ。01 §B-2 / Design Doc B
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
- [ ] **H-2**: Rust のライフタイム `'a` を文字列として誤色。lang1 の `'` 処理改善。05 §H-2 / Design Doc B
- [ ] **H-3**: detect が拡張子の大文字小文字を区別しない(FILE.RS 無装飾)。05 §H-3 / Design Doc B
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

- [ ] **B-4**: find 系/`find_buffer` の `Vec<usize>` 戻り値を `Option<usize>` へ。01 §B-4
- [ ] **B-5**: transpose_chars が点=0で先頭2文字を swap(Emacs は no-op)。01 §B-5
- [ ] **B-6**: 「編集で領域が消える(mark 追従)」を README に明文化。01 §B-6
- [ ] **B-9**: buffer の find/クランプ/move_gap 単体テスト追加。01 §B-9
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
- [ ] **H-8**: detect/styles の単体テスト追加。05 §H-8
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
- 2026-10-24: **fix #1(このコミット)**: T-1/T-2/M-1/M-2/M-4/T-3/T-4/D-1 実装。
  コミット前ゲートレビュー APPROVE(C-q EOF の NUL 限界コメント、C-x C-c 行 EOF 記述を反映)。
  e2e に s140/s141/s142 追加(パイプラナー rc+サイズ+内容アサート)。
  実施中の追加対応: e2e s21 が Flake(境界行+境界列同時で grep 件数 2 化/境界行幅=cols=80)
  → 明示レイアウトアサートに置換(単体 3/3 通過)。学習記録に追加。
  (fix #1 のコミットハッシュは次エントリに記載)
