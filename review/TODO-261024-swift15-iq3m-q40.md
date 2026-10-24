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

- [ ] **T-1**: `read_line_c` の EOF 無限ループ(NUL 無限挿入。実測3秒で25MB出力)。EOF を「キャンセル」として区別返却。受入条件: 全ミニバッファ経路(C-r/C-l/C-x C-f/C-w/C-i/C-x b/C-x C-b/終了確認)で EOF ハング解消、`restore_mode` 到達。04 §T-1 / Design Doc A
- [ ] **T-2**: `read_char_from` が EOF 時に NUL/空白を積む。EOF で挿入中止。04 §T-2 / Design Doc A
- [ ] **M-2**: 終了確認(confirm_exit)中の EOF は無限ループ(実測35MB/3秒)。EOF=キャンセル(確認は終了しない)+メインループ EOF の無条件終了(§M-4 方針)で終了する旨を実装・コメント・README に明記。02 §M-2 / Design Doc A
- [ ] **M-1**: go-to-line の u32 溢れ panic(実測 exit 101)。`parse` + バッファ行数へのクランプに置換(debug/release 挙動差=R-2 も参照)。02 §M-1 / Design Doc A
- [ ] **T-3**: raw mode 復元を Drop ガード+panic hook+シグナルハンドラで保証。04 §T-3 / Design Doc A
- [ ] **M-3**: C-s 増分検索中の矢印キー ESC accept 後、残 `[A` が self-insert される。02 §M-3
- [ ] **H-1**: `heq_at` の全文 clone で styles が O(n²)(451KB 1フレーム実測12.9秒)。スライス参照化。受入目標: 451KB .rs で 1フレーム < 1秒。05 §H-1 / Design Doc B
- [ ] **S-1**: draw_all が全バッファを毎フレーム複数回走査+全面 print。text() 1回化+styles 1回化(可視行範囲 styles と差分描画は Design Doc B でスコープ判断)。03 §S-1 / Design Doc B

## Phase 2: 中優先(Medium)

- [ ] **B-1**: gap buffer が gap を前確保せず挿入 O(n)。new() で gap 預け插入 O(1) 償却。01 §B-1 / Design Doc B
- [ ] **B-2**: find_forward/backward が毎回全文 text() 生成。char_at 直接スキャンへ。01 §B-2 / Design Doc B
- [ ] **B-3**: kill_to_end_of_line が mark 後方(同一行)で無音 no-op。後方 mark は領域なし扱いへ。01 §B-3
- [ ] **M-4**: メインループ EOF が未保存バッファを無警告破棄。方針決定(EOF=終了据え置き+dirty 時のメッセージ/文書化)。02 §M-4 / Design Doc A
- [ ] **M-5**: C-x k が modified バッファを無確認で空にする。確認プロンプト追加。02 §M-5
- [ ] **M-6**: C-x C-f が同一ファイルを毎回新規バッファ化。既存バッファ再利用。02 §M-6
- [ ] **M-7**: 未バインドの C-x 後続が無言。default で "Quit" 相当。02 §M-7
- [ ] **M-8**: C-x C-w が書き出し失敗後も filename を設定。成功時のみ設定。02 §M-8
- [ ] **M-9**: read_file が全エラーを無言(空表示)。開けない時のステータスメッセージ。02 §M-9
- [ ] **M-10**: C-l が next_line を行数回繰り返す O(n×行長)。1回スキャンへ。02 §M-10
- [ ] **S-2**: 全角(CJK)を幅1として扱う。セル数基準化。03 §S-2 / Design Doc C
- [ ] **S-3**: split/collapse で tree のスロットが単調増加(未回收)。03 §S-3
- [ ] **T-4**: read(2) エラーを EOF と同一視。Err では再試行ループとし Ok(0) をもって EOF とする(std が EINTR を再試行する事実を確認したらコメントで明文化)。04 §T-4 / Design Doc A
- [ ] **H-2**: Rust のライフタイム `'a` を文字列として誤色。lang1 の `'` 処理改善。05 §H-2 / Design Doc B
- [ ] **H-3**: detect が拡張子の大文字小文字を区別しない(FILE.RS 無装飾)。05 §H-3 / Design Doc B
- [ ] **R-1**: clippy 94 件。`cargo clippy --fix` + 手当てで掃除。完了ゲート: `cargo clippy` 警告0行(AGENT.md に clippy ルール追記)。06 §R-1
- [ ] **R-2**: e2e が debug のみ。release での代表スモーク追加。06 §R-2
- [ ] **R-3**: window/screen の単体テスト追加(buffer=find 系は B-9、highlight は H-8 が担当)。06 §R-3
- [ ] **R-4**: run.out/smoke.txt/demo.txt 未整理。.gitignore 追記。06 §R-4
- [ ] **D-1**: README:148・keybindings.md:13「EOF 無条件終了」の記述を実装に合わせ修正(Phase 1 の EOF 実装修正と同時に実施、完了記録は当該コミット)。07 §D-1 / Design Doc A
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

- (記載待ち)

## 変更履歴

- 2026-10-24: TODO 作成(review/00-07 から整理)。内訳 高8/中26/低32+良好点記録8。
- 2026-10-24: サブエージェントレビュー(REJECT)対応 — T-7 チェックボックス追加(Blocker#1)、
  M-2 の EOF 指示を「キャンセル」に訂正(Blocker#10、データ喪失を招く逆指示を解消)、
  T-1/M-1/H-1/S-1/R-1/R-3/T-4/D-1 の受け入れ条件・スコープ・担当分離を明記、件数を実測値に訂正。
