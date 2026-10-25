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
- [x] **B-3**: kill_to_end_of_line が mark 後方(同一行)で無音 no-op。後方 mark は領域なし扱いへ。01 §B-3 → 前方 mark のみ kill 境界(`hi=mark`/後方・off-line・mark==point は行末まで)。unit kill_backward_mark_kills_to_eol(後方 mark で行末/改行位置で行結合/前方 mark は領域辺で停止)、e2e s154(旧 worktree で無音 no-op 確認・新 PASS、fix #12)。README Region 条項に「kill の停止は前方 mark のみ」を追記
- [x] **M-4**: メインループ EOF が未保存バッファを無警告破棄。方針決定(EOF=終了据え置き+dirty 時のメッセージ/文書化)。02 §M-4 / Design Doc A → 方針確定・README/keybindings に明記(fix #1)。確認プロンプトは付けない(stdin 閉鎖後は確認不能が本質)
- [x] **M-5**: C-x k が modified バッファを無確認で空にする。確認プロンプト追加。02 §M-5 → `Kill buffer <name>? (yes|no)`(yes/y のみ実行、EOF=キャンセル=MAIN EOF 終了は M-4 仕様)。e2e s144(fix #4)
- [x] **M-6**: C-x C-f が同一ファイルを毎回新規バッファ化。既存バッファ再利用。02 §M-6 → `find_buffer`(名前=路径一致、C-x b と同一基準)で既存なら disk 再読込せず窓に表示(編集・modified 保持)。単窓は分割して新窓に、表示済みなら no-op。e2e s148(fix #7)。C-x C-w バインドは名前は変えないため対象外(README に明記)
- [x] **M-7**: 未バインドの C-x 後続が無言。default でメッセージ追加。02 §M-7 → `Undefined command: C-x <char>`。e2e s145(fix #4)。※TODO 原文の「Quit 相当」は採用せず(終了は事故リスク、 Emacs の "Undefined" 相当のメッセージ採用)
- [x] **M-8**: C-x C-w が書き出し失敗後も filename を設定。成功時のみ設定。02 §M-8 → 成功時のみバインド(失敗時 C-x C-s は "No file name" 経路)。e2e s146(fix #4)
- [x] **M-9**: read_file が全エラーを無言(空表示)。開けない時のステータスメッセージ。02 §M-9 → `Result<String,String>` 化で argv/C-x C-f/C-x C-i が `Failed to open <path>: <errno>` を表示。argv 失敗時 filename 未バインド(C-x C-s が元ファイルを上書きしない)。e2e s147(fix #4)
- [x] **M-10**: C-l が next_line を行数回繰り返す O(n×行長)。1回スキャンへ。02 §M-10 → 単一 O(n) スキャン(行数計数とライン頭計算を一体化、want が総行数超なら最終行にクランプ=M-1 維持)。C-l 実測 3/6・9→6/6・1/6、e2e s112/s141 2/2(fix #10)
- [x] **S-2**: 全角(CJK)を幅1として扱う。セル数基準化。03 §S-2 / Design Doc C → `char_width`(screen.rs、幅表の単一源)を導入し、`draw_cell` の打切り/パディングと `point_pos`(=カーソル列)をセル基準化。e2e s149/s150(fix #8)
- [x] **S-3**: split/collapse で tree のスロットが単調増加(未回收)。03 §S-3
- [x] **T-4**: read(2) エラーを EOF と同一視。Err では再試行ループとし Ok(0) をもって EOF とする(std が EINTR を再試行する事実を確認したらコメントで明文化)。04 §T-4 / Design Doc A → 再試行ループ実装。実測: std の StdinLock::read は EINTR を自動再試行しない(Err(Interrupted) を返す)ため explicit ループが本体。コメント明記済み(fix #1)
- [x] **H-2**: Rust のライフタイム `'a` を文字列として誤色。lang1 の `'` 処理改善。05 §H-2 / Design Doc B → 3条件(同一行閉じ/j>=i+2/内容が string 的)採用。フィクスチャ `fn f<'de>(x: &'de str) {}` をテストで固定(fix #2)
- [x] **H-3**: detect が拡張子の大文字小文字を区別しない(FILE.RS 無装飾)。05 §H-3 / Design Doc B → ASCII 小文字化比較+fizz マトリクステスト(fix #2)
- [x] **R-1**: clippy 94 件。`cargo clippy --fix` + 手当てで掃除。完了ゲート: `cargo clippy` 警告0行(AGENT.md に clippy ルール追記)。06 §R-1 → 102 件を `cargo clippy --fix`(92 自動)+ 手当て(&String→&str、ループカウンタ→enumerate、checked_div、if let、word_eq 書き直し、`let _ =` 受信、self-insert 2 分岐マージ)で **0 行**。AGENT.md に clippy ゼロ則を追記。e2e 76/76+release スモーク 6/6 PASS(fix #6)
- [x] **R-2**: e2e が debug のみ。release での代表スモーク追加。06 §R-2 → `test-design/release_smoke.sh`(s02/s103/s110/s112/s140/s141 を release バイナリで実行、ビルド自動、6/6 PASS)。溢れ系 s141 を含む(fix #5)
- [x] **R-3**: window/screen の単体テスト追加(buffer=find 系は B-9、highlight は H-8 が担当)。06 §R-3
  - window.rs: layout ジオメトリ(C-x 3=左 40/境界 col40/右 39、C-x 2=12+境界行+11)、狭域
    フォールバック(境界行/列なしの素分割、幅 1 の非分割)、flatten 順序(ネスト分割で 0,1,2)、
    find_slot 未探索=`kind.len()`、split 未知 leaf=false、collapse 回收(fix #13)
  - screen.rs: split_lines(末尾空行/case 網羅)、line_offsets(文字数+改行、マルチバイト=1 文字)、
    point_pos(行送りのリセット、point 越境クランプ、全角 2 マス/結合文字 0)、char_width に半角
    カタカナ U+FF71=1 を追加
  - follow-up(fix #13 ゲート由来)完了: e2e s155「C-x 1 後に C-x C-f」で単一窓は下分割で
    新ファイル表示(README 文書規則)を固定。旧(2311b76 worktree)3/3 FAIL・新 3/3 PASS
- [x] **R-4**: run.out/smoke.txt/demo.txt 未整理。.gitignore 追記。06 §R-4 → 3 ファイルを .gitignore に追加(fix #5)
- [x] **D-1**: README:148・keybindings.md:13「EOF 無条件終了」の記述を実装に合わせ修正(Phase 1 の EOF 実装修正と同時に実施、完了記録は当該コミット)。07 §D-1 / Design Doc A → README 本文+キー表+keybindings.md 更新済み(fix #1)
- [x] **D-2**: design/README.md の "Not implemented" 列表陳腐化(syntax highlighting 等)。07 §D-2 → Implemented 側に highlight/minibuffer 補完/ファイル再利用/検索を追記、Not implemented 側から syntax highlighting を除去し undo 不可の风险注記+「コマンドフレームワーク無し(補完付きプロンプトは実装済)」に更新(fix #11)
- [x] **D-3**: architecture.md の4点(gap O(1)/keymap tree/curses 描画/status top)を実態に。07 §D-3 → 「Current implementation (as-built)」節を追加し4点(挿入 O(n)/if-else 連鎖/ANSI 手書き描画+curses は size のみ/ステータス最下行)を明記、gap 本文に相互参照(fix #11)
- [x] **D-4**: README:129「全窓が同一バッファ表示」は C-x b 後誤り。07 §D-4 → 「分割直後は同一/`C-x b`・`C-x C-f` はアクティブ窓のみ切替・kill 時は共有窓全てで空になる」に書き換え(fix #11)
- [x] **D-5**: design/keybindings.md の C-</C-> → M-</M->。07 §D-5 → 行を置換(raw tty 注記つき)(fix #11)
- [x] **D-7**: README:47 の termsize 表記(直接依存は curses)。07 §D-7 → `curses::canvas::get_size`・直接依存=curses(termsize は推移)と明記。Toolchain 行の「curses (ANSI canvas)」も「size クエリのみ、描画は ANSI 手書き」に修正(fix #11)
- [x] **D-10**: README 環境記述(モデルID)が opencode.json と二重管理。分離検討。07 §D-10 → 揮発スナップショット(マシン/HW/ローカル LLM/エージェント)を docs/dev-env.md(日付付き)へ移設、README は不変のビルド事実+ポインタのみ、モデル ID の単一源=opencode.json を明記(fix #11)

## Phase 3: 低優先(Low)

- [x] **B-4**: find 系/`find_buffer` の `Vec<usize>` 戻り値を `Option<usize>` へ。01 §B-4 → find 系は fix #2 で先取り解消(main.rs 4 箇所 Option 化)。`find_buffer`(main.rs:21)は「一致列」を返す別関数のため `Vec<usize>` のまま(Design Doc B に方針明記)
- [x] **B-5**: transpose_chars が点=0で先頭2文字を swap(Emacs は no-op)。01 §B-5
  → 点=0 分岐を削除(no-op 化)。unit transpose_chars_noop_at_start・e2e s156(点 0 で no-op/点 2 で h↔i swap、HEAD worktree 2/2 FAIL・新 3/3 PASS、README/keybindings に no-op 明記、fix #18)
- [x] **B-6**: 「編集で領域が消える(mark 追従)」を README に明文化。01 §B-6
  → README Region 条項に「挿入(入力/C-j/C-m/C-o/C-y)と backspace は mark を点へ動かし領域消去、C-@ で張り直し。C-d/C-k は領域維持」を追記(コード実測:insert_char/insert_newline_after/delete_backward の 3 箇所のみ mark=point、fix #18)
- [x] **B-9**: buffer の find/クランプ/move_gap 単体テスト追加。01 §B-9 → (部分)find/gap/move_gap(挿入経路)/insert_newline_after は fix #2 で追加済。クランプ系は e2e s112/s141 が担保。残り: line_start/line_end 境界の unit
  → 残り実施(fix #21):movement_clamps_at_line_edges(beginning/end_of_line・next/previous_line のクランプと最終行停止)、transpose_at_end_no_advance(末尾=直前 2 文字 swap・point 不動)、empty_buffer_edge_edits(空で delete 偽/kill 空/C-t no-op)
- [x] **B-10**: `self: &Buffer` → `&self`、String 引数 → `&str`/`Into<String>`。01 §B-10
  - buffer.rs: 全 29 メソッドの receivers を `&self`/`&mut self` に(extern-style 廃止)。
    `Buffer::new(name/text: impl Into<String>` にし literal 呼び出しの `.to_string()` を
    削除(テスト 14 箇所+main 6 箇所。&String からは明示 `clone()`)
  - terminal.rs: `read_line`/`read_line_buffers`/`read_line_files`/`read_line_eof` の
    prompt を `impl Into<String>` 化(core の read_line_c は String のまま)。main の
    素 literal prompt 6 箇所の `.to_string()` 削除(fix #19)
- [ ] **M-11**: 300行ディスパッチをテーブル化(architecture の keymap tree 記述と共に対応)。02 §M-11
- [x] **M-12**: prev_cmd は last-command 相当に限定(現状文書と整合、注記のみ)。02 §M-12
  → main.rs `prev_cmd` 宣言に注記(直前トップレベル バイトのみ/コマンド内部バイトは更新せず/C-y 連鎖は意図的に不可=README と整合、fix #20)
- [x] **M-13**: leaf/bufs 間の point 書写しをヘルパー集約。02 §M-13
  - main.rs に `sync_leaf`(bufs→leaf の point/mark 書写し:C-x o 直前・ループ末尾の
    2 箇所)と `attach_leaf`(buffer 添付+point/mark/scroll=0:C-x b 切替 2 箇所+
    show_buffer の単一窓/分割窓の 2 分岐=計 4 箇所)を導入。ループ先頭のクランプ
    (M-15)は leaf→bufs 方向で意味論が別のため非集約(注記コメント済み)
  - per-leaf point/mark キャッシュ仕様(fix #9 ゲート記録): 分割 leaf が同一 buffer を
    alias しても point は leaf ごとに独立。C-x k は全 alias leaf を 0 化(M-15)、
    再表示時 show_buffer/C-x b は 0 リセットなので stale キャッシュは可視化されない
- [x] **M-15**: alias キル後の stale leaf-point で panic(fix #7 ゲート発見・既存バグ)。`C-x 2`→`C-e`→`C-x o`→`C-x k`→`C-x o`→`x` で buffer.rs move_gap 添字外(pure legacy キーで再現、M-6 以前から存在)。`C-x k`時に buffer を指す全 leaf の point/mark/scroll を 0 化、かつ/または main ループ先頭 sync で `point.min(buf.len())` クランプ。M-13 と併せて対応可 → 両方実施(C-x k=全 alias leaf を 0 化/ループ先頭=point・mark を len にクランプ)。e2e s153(旧 worktree で panic 確認・新 PASS、fix #9)
- [ ] **M-14**: .to_string()/clone 乱用を API 変更で削減。02 §M-14
- [x] **S-5**: Frame.kind を enum 化(境界 Frame の leaf ダミー除去)。03 §S-5
  → `Frame` を enum 化(Window{leaf,top,height,col,width} / BoundaryRow{top,col,width} /
  BoundaryCol{top,height,col})。境界の dummy `leaf: 0` と magic `kind: 0/1/2` を消去、
  境界 Frame の定数フィールド(Row.height=1/Col.width=1)も削除(デッド警告駆逐)。
  screen.rs の prepass/draw が match 網羅(fix #23)
- [x] **S-6**: scroll 再計算の不変条件をコメント化。03 §S-6
  → screen.rs scroll 再計算部コメント:毎フレーム point から再計算(ページ送りなし)、leaf の scroll は次フレームの開始点で再描画は冪等(fix #20)
- [x] **S-7**: 分割時カーソルと status clamp の関係コメント。03 §S-7
  → screen.rs `rows - 2` クランプに注記。finding の主張(下窓最下行でカーソルが 1 行上に見える衝突)はゲートで不可 reach と再判定:layout 高さ=rows-1 と scroll クランプにより crow<=rows-2 が常に成立、クランプは防御的デッドコード(finding 本文よりコードが正しい、fix #20)
- [x] **S-8**: find_slot センチネルを Option<usize> へ。03 §S-8
  → `find_slot` を `Option<usize>` 化(旧センチネル `kind.len()` 廃止)。split=match で
  早期 false、collapse=`is_some()`、unit は is_some/is_none アサート(fix #23)
- [x] **S-9**: Node.vertical の命名/コメント改善。03 §S-9
  → Node.vertical に注記:true=縦積み(C-x 2・`-` 行共有)/false=横並び(C-x 3・`|` 列共有)。Emacs の -right は水平配置で名は逆向きに読める(fix #20)
- [x] **S-10**: point_pos の同一フレーム2回呼びを1回化。03 §S-10 → fix #2 で leaf 単位 1 回化(leaf_pos ベクタでスクロール前計算とカーソル配置で共有)
- [x] **S-12**: レイアウト重なりデジェネレートのコメント。03 §S-12
  → 幅<2 の非分割(両子同一ジオメトリ)に注記。ゲート訂正:80 列でも C-x 3 を約 7 回(左 leaf 追従で 80→40→…→1)で到達しうる。クリップ機構は意図的に持たない設計と明記(fix #20)
- [x] **T-5**: fd0 固定の設計注記(stdin 非 tty で raw mode なし)。04 §T-5
  → save_raw_mode の doc に fd0 固定・非 tty では tcgetattr 失敗=saved None で復元なし(pty 前提の設計、graceful 放弃)を明記(fix #20)
- [x] **T-6**: Tab 補完 0 候補時に bell。04 §T-6
  → read_line_c の補充分岐に `hits.is_empty()` で `\x07`(BEL)追記。pty プロブで分離確認:
  無候補 Tab=新ベルあり/旧無音、1 候補 Tab=ベルなし+サフィックス echo 継続(fix #22)
- [x] **T-7**: list_width/cand_at の当たり判定が文字数基準(CJK 候補でズレ)。セル数基準へ(Design Doc C と同時)。04 §T-7 → `cells_of` 追加、`list_width`/`cand_at` はセル合計、`read_line_c` の全 print!(キャレット配置/erase)は `caret_col`(二重単位の変換点を単関数に集約)経由へ。e2e s151(fix #8)
- [x] **T-8**: file_candidates: ディレクトリ末尾 `/`、ソート、`~` 展開。04 §T-8
  → `file_candidates`: 候補を `cands.sort()`(read_dir 順の露出を排除、Emacs の
  辞書順一覧に合わせる)。dir エントリに末尾 `/` を付与(`DirEntry::file_type` は
  symlink を追わない: merged-usr の `/bin` はスラッシュ無しで一覧=仕様)。
  `expand_tilde` 新設(`~` と `~/...` のみ `$HOME` 展開、`~user` はリテラル)を
  fs 側 dir に適用、一覧エコーは入力 head を保持。`dir==""`(入力 `/...`)は
  `read_dir("")` が常に失敗していたので `/` に写す(fix #22 ゲート Minor-1 の
  follow-up)。main.rs の C-x C-f/C-x C-w/C-x C-i プロンプトで `expand_tilde`
  (バファ名・保存先は実パスになる)。unit 2 件(sort+slash+root と tilde 規則)、
  e2e s157(辞書順)/s158(dir スラッシュ+EISDIR 失敗)/s159(`~/emur` Tab で
  $HOME のファイルを開く)/s160(`/` Tab で ` /usr/` 一覧)(fix #24)
- [x] **T-9**: VMIN/VTIME 明示設定。04 §T-9
  → save_raw_mode で `c_cc[VMIN]=1`/`c_cc[VTIME]=0` を明示(環境継承に依存しないblocking 1 バイト読み)。フル e2e 全入力経路が回帰網(fix #22)
- [ ] **T-11**: print+flush 出力ヘルパー集約。04 §T-11
- [x] **H-4**: md_styles 未閉結バッククォートのテスト追加。05 §H-4
  → unit md_unclosed_code_span_stops_at_line_end:未閉結 ` は行末まで装飾し次行を巻き込まないことをアサート(fix #21)
- [x] **H-5**: 数値スキャン過装飾(2hex)の軽微修正/注記。05 §H-5
  → 修正採用(fix #21):数値トークンの英字は `0x` 直後(x は先頭数字の次・先頭が 0)でのみ受理。`2face`→`2` のみ装飾/`0x1F`・`0xDEADbeef`・`1.2` は従来どおり。unit number_scan_letters_only_after_0x 追加(finding の `2hex` は 'h' で既に停止する例。実過装飾は 2face 型)
  - ゲート Major-1 対応:指数 `1e6`/`12.3e4`/`1e-3`(e/E は直後が数字、または符号+数字
    のときのみ合流。`2e` は `2` のみ)と `0b101`(0 直後の b)を復元。Rust の
    数値サフィックス(`1.0f32`/`2u8`)は 2face パターンと区別不能のため意図的に
    非受理(=旧 scan は過装飾で塗っていた、コメントに明記)。unit に指数・二進・
    符号付き指数・dangling e のケース追加
- [x] **H-6**: HTML タグ内属性文字列の装飾(注記)。05 §H-6
  → 注記採用(fix #21):tag スキャンが `<`→`>` まで style 5 で塗り属性の "x" は string(2) にならない(見た目のみ・設計どおり)とコメント明記
- [x] **H-7**: スタイルコードを enum 化。05 §H-7
  → `highlight::Style{{Plain,Keyword,Str,Comment,Number,Marker}}` を導入(素 usize 0..5 を
  全置換、`Str`=予約語回避)。`styles()->Vec<Style>`/`md_styles(&mut [Style])`、screen.rs の
  `style_esc` は網羅 match(新 variant 追加は compile error で検出)。SGR 出力は byte 同一
  (36/35/32/34/1;36/0m、e2e 装飾系 s90-s94/s103/s105 と release smoke color で担保)(fix #23)
- [x] **H-8**: detect/styles の単体テスト追加。05 §H-8 → fix #2 で追加(detect 拡張子マトリクス、lifetime フィクスチャ、string/char リテラル、json、md)
- [x] **R-5**: curses 依存縮小(size 検出のみ)/[profile] 注記。06 §R-5
  - curses 0.1.0(実態は `termsize::get().unwrap_or(0,0)` の薄ラッパーとソース確認)を
    直接依存 `termsize` 0.1.9 に置換。`screen::term_size()` ヘルパーに集約し 3 呼び出し
    箇所(draw_all/read_line_c/search)から使用。curses は Cargo.lock から消滅(純 Rust は
    fix #15 ゲートで確認済みの情報と整合)。README 依存記述 2 箇所更新(fix #17)
  - `[profile.release]` はレビュー注記どおり未設定のまま(起動性能に無関係・優先度低。
    設定するなら lto/strip 候補として記録)
- [x] **R-6**: CI 追加(build+test+clippy+e2e)。06 §R-6 → 導入時 `cargo clippy --all-targets` の既存 test コード 17 lint(fix #7 ゲート検出。AGENT.md の現行ゲートは plain `cargo clippy`=0 で合格)も掃除してゲートに含めるとよい
  - 実 lint 数は 18(12 `unnecessary to_string`+5 `== ""`+1 owned 比較。buffer.rs 13・terminal.rs 5)を全て代码側で掃除(`&"x".to_string()`→`"x"`(find_* は元から `&str` 引数)、`== ""`→`.is_empty()`、`cut == "world".to_string()`→`cut == "world")。`--all-targets`=0 を AGENT.md ゲート化(fix #15)
  - `.github/workflows/ci.yml`: 単一 jobs(gates)/ubuntu-24.04で build(`-D warnings`)・clippy(`--all-targets -- -D warnings`)・unit・e2e(tmux 同梱ランナー、単一 job で tmux 競合なし)。`--locked`・branches:[master] 付き(fix #15)
- [x] **D-6**: keybindings.md の C-d 二重掲載・C-\ 凡例削除。07 §D-6
  → 凡例から `C-\`(未バインド)削除。C-d/DEL を Editing 節へ集約(Movement 節から削除、二重掲載解消)、fix #18
- [x] **D-8**: C-s/C-r/C-l minibuffer は補完なしの明記。07 §D-8
  → keybindings.md Minibuffer 節に前置き(補完=path/buffer-name のみ、C-s/C-r/C-l は素の入力行・Tab はリテラル)、README 一文追記、fix #18
- [x] **D-9**: README:79 に「全読み込みエラーで空で開く」注記(M-9 と同時)。07 §D-9 → M-9 と同時に新挙動(Failed to open 表示・filename 未バインド)を README へ記述(fix #4)

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
- **ステータス行の msg 慣習**: msg はコマンドが明示的に `""` を代入するまで**残り続ける**
  (self-insert やカーソル移動では消えない)。e2e のアサートはこれを前提に書く
  (s145: 'Z' 後も "Undefined command: C-x Q 1/3 (2%)" が正しい期待値)。
  消すコマンド(C-x 1 等)を使ってからのアサートが堅い。
- **ステータス行は幅 cols(80)でクリップ**: 長い msg(Failed to open + errno + 位置)は
  80 桁で切れ、残りは折返し行になる。長い期待値は `expect_lastline`(完全一致)でなく
  `expect`(grep 系)を使う(s147)。
- **C-x C-s は filename が書き込み先**: C-x C-w 失敗時に filename をバインドすると
  後続の C-x C-s が**書けないパスを保存先化**して上書き試行の連鎖になる。
  「書き込み成功時のみバインド」が本質(M-8)。失敗後は "No file name for this buffer"
  経路に乗る(s146 がこれを断言)。
- **read_file の失敗秘匿はデータ喪失経路を伴う**: 失敗時に filename を付けて空で開くと
  C-x C-s が**元ファイルを上書き**しうる。argv 起動の読み込み失敗時は
  filename なしの空バッファ+Failed to open 表示(fix #4)。
- **release スモークの組み立て(R-2)**: e2e.sh のヘルパは行範囲 sed で抽出する
  慣習(1-53=本体ヘルパ、543-545=色付きキャプチャ系)。追加ヘルパ
  (expect_lastline)は生成側で定義し、BIN は変数再代入で release を向くようにする。
  debug と release で挙動が分かれるのは整数溢れ系(u32 溢れ=debug panic/release wrap)
  なので、s141(go-to-line 15 桁)を代表シナリオに含める。
- **dispatch 連鎖の終端 else は前置 if を巻き込む**: C-x ハンドラは
  `if n == 0x03 {...}`(独立 if)→ `if n == 0x31 {...} else if ...` の鎖、という
  構造だった。M-7 で鎖の終端に `else { Undefined }` を足すと、**独立 if の
  C-x C-c が鎖に落ち**、キャンセル時の "Cancelled" が "Undefined command" で
  上書きされた(e2e s111 が検出、ゲートレビューが REJECT で特定)。
  教訓: **既存ディスパッチに else 節を足すときは、鎖の先頭を独立 if にせず
  `else if` で連結して全分岐を単一チェーンにする**+既存の Cancelled 系 e2e
  (s111/s15/s142)を必ず回す。
- **e2e 期待値はコードから逆算せず実測併用**: msg サフィックス付きステータス
  (`<label> <line>/<total> (<pct>%)`)の完全一致は、ラベルが msg 継承・クリップで
  読み違えやすい。まず tmux で実測してから expect_lastline を書く(s145/s146 で実施)。
- **clippy 実測ベースライン**: HEAD=109 → fix #2 作業ツリー=118(ゲート計測)。
  新規カテゴリ(manual_repeat_n/partialeq_to_none/unnecessary_cast/ptr_arg×2)は
  ドライブバイ修正済み(100 に減少)。R-1 の対象として計上。
- **clippy スイープの手順(R-1 実施記録)**: (1) `cargo clippy --fix --bin emur` で
  安全適用分(92/102)を一括、(2) 残りは手で lint を読んで解消。今回の手当て内訳:
  `&String`→`&str`(ptr_arg)、明示カウンタループ→`enumerate`(explicit_counter_loop)、
  `if x>0 {a/x} else {0}`→`checked_div().unwrap_or()`(manual_checked_division)、
  単一パターン `match Some/None`→`if let`、`is_ok()` 破棄→`let _ =`(unused_must_use 系)、
  (3) **自動修正の産物を点検**: `TtyGuard { saved: None }// comment` のコメント密着と
  `if so.flush().is_ok() {}` 空ブロックは sed 由来の傷。機械修正後はビルド/テストで
  なく diff を目視する。(4) self-insert の ASCII/UTF-8 分岐は本体が同一なので
  `(0x20..0x7f).contains(&b) || b >= 0x80` にマージ(分岐重複 lint 対策)。
- **clippy ゼロは AGENT.md のビルド則に明文化済み**: 今後 `cargo clippy` の
  `warning:` 0 行が全コミットのゲート。`#[allow(...)]` による黙殺は禁止(警告則と同じ)。
- **grep によるゲート判定はパイプ末尾に注意**: `cargo build 2>&1 | grep -cE '^warning:'` は
  ビルド失敗時も警告 0 行で exit 0 になり「合格」と誤読する(実際は E0308 で fail していた)。
  **ビルド成否は `cargo build 2>&1 | tail -1`(Finished/error 行)を併記して判定する**。
- **フル e2e とゲート/プローブの並行は禁止(run11 の偽 FAIL)**: tmux セッション名は
  シナリオ番号由来(`emu-e2e-sNN`)でゲート内の同名プローブは suites 側の稼働中セッションを
  `start_session` が kill して相互に偽 FAIL を起こしうる(s124)。CPU 競合は STEP=0.4s の
  タイミングアサートを flake させる(s83)。**e2e と tmux/cargo を含むレビューは直列に**し、
  suite FAIL 時はまず単体 3 回の実行で再現性を確認して製品バグを切り分ける。
- **C-x C-f 再利用の一致基準(M-6)**: 名前一致(`find_buffer`=C-x b と同一)。Emacs の
  「ファイル実体一致」は採用しなかった:C-x C-w で *demo* に filename をバインドした場合の
  み差が出る(実体一致なら *demo* を再表示)。名前一致は「C-x b で見える状態」と 1:1 で
  理解しやすく、バッファ名の重複(=M-6 の症状)を確実に防げる。C-x C-w バインド系は
  対象外になることを README に明記。表示ロジックは `show_buffer` に集約し、
  「現在表示中なら no-op(point リセットしない)」を分岐化。
- **`expect_lastline` は完全一致なのでエスケープしない**: grep 系の `expect` と混同して
  `\*` と書くとバックスラッシュ込みの文字列比較になり必ず FAIL する(modified フラグ
  `*` はそのまま書く)。
- **e2e s21 の Flake 根本原因(既存)**: 単発キャプチャが**描画と競合**する
  (C-x 3 適用前の 2 窓状態、またはフレーム先頭の screen_clear 直後に下窓が
  未描画)。`expect_occ`/`expect_count` 系(件数アサート)は分割直後の
  キャプチャに弱い。※行結合(|)でトークンが消える説は誤り(結合行は
  `grep -oE` で 2 件数える)とゲートレビューで確定。
  境界行のダッシュ数は幅=cols(80)で固定長アサート(`^-{79}$`)は不可。
  置換したアサート(左窓 2 行+境界列 glue 行+ダッシュ行、3 回ポーリング)は
  単体 3/3 安定。

- **実測レイアウトはコード算術より強い(fix #8/s150)**: Doc C もゲートレビューも
  80 列の左右分割を「左 39+境界+右 40」と想定したが、`layout()` は
  `w1 = width / 2` = 左 **40** セル/境界列 40/右 39 が実測(tmux capture で
  境界の直前が あ×20=40 セルと確認)。s150 は「あ×20[|]」+「あ×21 の不在」に
  修正し、分離は右窓の連係長(旧=30 連、新=19)で成立。**新しいアサートは
 pane 実キャプチャから導出する**こと(想定計算で書くと自分もゲートも同じ穴を踏む)。
- **bash に NUL は送れない**: `sendb $'\000'` は**空文字列**(bash の文字列に
  NUL は存在不能)。e2e で `C-@`(mark)を送るには tmux の名前付きキー
  `tmux send-keys -t "$SESSION" C-@` を使う(bash heredoc 経由の printf は
  パイプ repro でなら可、python pty でも `b"\x00"` で可)。
- **C-b は region clamp で mark まで跳ぶ(仕様)**: `backward_char` は
  「mark が現行行にあれば mark 位置で停止」。ファイル起動直後は mark=0 なので
  point 2 から C-b 1 回で 0 に飛ぶ(1 文字ずつではない)。1 移動のテストは
  先に `C-@` で mark を point に同期してから。**tmux send-keys -l は制御
  バイトを複製しない**(od で 1 バイト確認済み)— 「C-b が 2 動く」は
  全てこの clamp が原因で、バグでも tmux の癖でもない。
- **read_dir の候補順は非決定**: s151 の Tab 候補は実測で a→i の順(a.txt が
  後ろ)。候補の当たり判定テストは期待値をファイル名でハードコードせず、
  **pane のエコー行から先頭候補を awk で抽出**して照合する。
- **raw モードのキー挙動追跡は python pty.fork が最速確定**: strace は
  ptrace 不可で使えない環境。pty で送信→ストリーム内の最終
  `\x1b[row;col]H` を正規表現抽出すればカーソル座標が 0.5s で読める
  (tmux capture より遥かに速い)。tmux 再現と食い違うときのリトマスに。
- **Doc C ゲートレビューの実効(fix #8)**: 候補エコーが basename ではなく
  **フルパス**であることを見逃すと s151 の打鍵数が根本から誤った
  (C-b×16 仮定 → 正 44)。数式(入力 16 セル/lw=46 セル/cand スパン
  [18,39])をゲートが再計算して修正させた。ドキュメントの算術は
  ゲートで再検算させる価値がある。

- **幅 0 文字は「セル-文字」減算を壊す(fix #8 ゲート Major-1)**: `cells_of(x) >= len(x)`
  は幅 0(結合記号)の存在で崩れる。`plen_c + cells_of(&out[..cur]) - cur` の別名演算は
  debug ビルドで panic(exit 101)、release で巨大列数の ESC(エコー破損)。実測 repro:
  Find-file で U+0301×13 + Backspace。**セル算術は足算だけの関数(caret_col)に寄せる**
  のが正(fix #8 で echo_tail はキャレット・セル列を直接受ける)。pty repro で修正後に
  panic 消失を確認。
- **Doc の「分離」主張は旧ビルド実測が必須(fix #8 ゲート Major-2)**: s150 の
  「21 連が残り FAIL」は新ビルド capture からの**推論**で、旧ビルドの pane は
  新と同一(境界フレームと右窓フレームが左窓の溢れを上書き、右窓溢れは pane 端で
  欠落)。旧 HEAD の git worktree を /tmp に作ってビルドし、同じ入力で比較して
  初めて主張が確定する(worktree add → cargo build → tmux capture → worktree remove
  --force)。s150 は回帰ガードに降格し、分離は s149/s151/s152 が担う。
- **可視化できない溢れと可視化される溢れ**: 単一窓では溢れは次の行の自フレームに
  隠される。分割後の 2 行目(左窓描画**後**に右窓 1 行目の自動ラップが落下)が
  分離点になる(frames 順=左→境界→右)。s152 の壊れ pane 実測:
  「あ×11+空白 10+あ×4+|」(境界が 25 文字目に移動するのが壊れの指紋)。
- **tmux capture の「20 文字+|」**: 左窓 40 セルは あ×20 が**ぴったり**収まる
  (19+空白+| ではない)。レイアウトは `w1 = width / 2`(window.rs)で偶数幅は左が
  1 列多い。行末の空白は capture 時に strip されるので、境界の位置は
  「直前文字列の長さ」で測る(境界 index=セル幅/2)。

- **bash の `git commit -m "..."` はメッセージ内バッククォートをコマンド実行する**:
  ダブルクォート中の \`2face\` 相当が置換され、語句が消えたメッセージがそのまま
  コミットされた(fix #21)。教訓:メッセージは \`git commit -F <file>\`(ヒアドキュメント
  経由)で渡し、バッククォート/`$(` を含めない。push 済みは amend+
  `--force-with-lease`(ソロ運用)で復旧。
- **read_dir 順はファイルシステム依存**:ローカル ext4/tmpfs と GitHub ランナーの
  ファイルシステムで readdir の列挙順が異なり、Tab 補完候補の先頭が変わる
  (s133 が CI 初回赤、fix #16 で順序非依存アサート化)。候補順に依存する
  シナリオは「最左候補=Left クランプ先」の不変量だけで主張を書く。
- **CI 導入は e2e を素朴に通すだけで効く**:build/clippy(-D warnings)/unit/
  e2e の単一 job(GitHub ランナーに tmux 同梱)で、ローカルでは見えなかった
  環境差(上記 readdir 順)が初回実行で露呈した(fix #15/初回赤→fix #16 で緑)。

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
- 2026-10-24: **fix #3(コミット ec5887e)**: M-3(C-s 中の矢印キーで `[B` ゴミ self-insert)。
  ESC 受理時に矢印シーケンスを消費+移動。e2e s143 追加。Phase 1 の High 全項目完了。
- 2026-10-24: **fix #4(コミット 8f98342)**: M-5/M-7/M-8/M-9(main.rs UX 群)+D-9README 注記。
  read_file→Result 化・C-x k 確認・Undefined command・C-x C-w 成功時のみ filename・
  Failed to open 表示。e2e s144〜s147 追加。コミットメッセージの TODO 原文「Quit 相当」は
  メッセージ表示に方針変更(理由を TODO に記録)。ゲート REJECT の B-1(C-x C-c キャンセル
  が Undefined で上書き)は鎖先頭 else if 連結で解消、76/76 PASS。
- 2026-10-24: **fix #5(コミット dadc431)**: R-2(release スモーク `test-design/release_smoke.sh`
  6 シナリオ PASS)+R-4(.gitignore 3 項目)。
- 2026-10-24: **fix #6(コミット 64c4fd7)**: R-1(clippy スイープ)。102 件 → `cargo clippy --fix`
  で 92 件自動適用+残り 10 件を手当てして **clippy 警告 0 行**。AGENT.md に clippy ゼロ則追記。
  全 6 ファイル+AGENT.md の変更。e2e 76/76、release スモーク 6/6、cargo test 16/16。
  コミット前ゲートレビュー APPROVE(全 62 hunk の意味変化監査、Blocker/Major なし)。
  Minor drive-by: terminal.rs EOF コメント復元、word_eq を allocation-free zip 版に
  (旧版はキーワード毎 Vec 確保で hot path)、自動修正の産物(`}//` 密着/空 if)掃除。
  word_eq 置換後は unit 16/16+e2e s103/s105(ハイライト実色アサート)+release スモーク 6/6 で検証。
- 2026-10-24: **fix #7(コミット 0d7d0a4)**: M-6(C-x C-f が同一ファイルを毎回新規バッファ化)。
  既存バッファ(`find_buffer` の名前一致=C-x b と同一基準)を disk 再読込なしで表示。
  表示済み窓では no-op。表示ロジックは `show_buffer` に集約(単窓→分割下/分割木→現窓)。
  e2e s148(編集保持+重複なし)。README/keybindings 記述追加。
- 2026-10-24: Design Doc C 確定(コミット ea3e2b7)。fix #8 ゲートで s151 の算術(候補=フルパス/
  lw=46 セル/C-b×44)・caret_cells の prompt 相対・echo_tail 別名演算・絵文字未カバーの正直化を反映。
- 2026-10-24: **fix #8(コミット 3dce691)**: S-2/T-7(全角表示幅のセル基準化、Design Doc C)。
  `screen.rs`: `char_width`(幅 2 の 14 範囲+幅 0 の結合 0x0300-036F+既定 1、単一の幅源)、
  `draw_cell` の打切り/パディングと `point_pos`(=カーソル列)をセル化(全角は枠内で
  割り込み描きせず)。`terminal.rs`: `cells_of` 追加、`list_width`/`cand_at` をセル合計、
  `read_line_c` のキャレット系 print 全 14 箇所を `caret_col`(cur の二重単位=入力内
  文字添字/候補域セルオフセットの変換点、足算のみ)へ集約。`echo_tail` はキャレット
  セル列を直接受ける(ゲート Major-1: 旧案 `(plen_c+cells_of(..))-cur` 別名演算は
  幅 0 文字で usize アンダーフロー→debug panic。列直接渡しが正)。README に CJK 描画
  段落(近似表の穴/絵文字未カバー/表外結合記号=1/右端キャレット)。unit +6(幅
  マトリクス/point_pos セル/cells_of/list_width/cand_at キャレット 12 判別/caret_col
  単位+結合記号のアンダーフロー回帰)、e2e +4: s149(カーソル列 0→2→4、C-@ 後 C-b→2)、
  s150(左窓 40 セル=あ×20+境界;ゲート実測で旧ビルドも通る=回帰ガード)、s151(C-b×44
  で先頭候補セル着地;read_dir 順非決定に pane から cand1 抽出)、s152(2 行目の溢れ
  破壊=分離: 旧は約 11 文字+空白+4 文字に壊れ/新は健全、旧 worktree ビルドで実測)。
  ゲート(APPROVE-WITH-CHANGES)Major-1/2・Minor-1/2 すべて適用(コード=列直接渡し、
  Doc=分離力の降格と s152 追加、README=表外結合記号の条項)。性能 0.073-0.088s(451KB debug)。
  (fix #8 のコミットハッシュは次エントリに記載)
- 2026-10-24: **fix #9(コミット 002984f)**: M-15(alias キル後の stale leaf-point で panic)。
  二層対策: (1) `C-x k` はkill 対象 buffer を指す**全 leaf** の point/mark/scroll を 0 化
  (C-x 2/3・M-6 再利用で作れる alias を網羅)、(2) main ループ先頭の leaf→buffer 書写しで
  point/mark を `buf.len()` にクランプ(in-place 置換の他の経路にも効く安全層)。
  e2e s153: C-x 2→C-e→C-x o→C-x k→C-x o→x。新ビルド 2/2 PASS、HEAD worktree
  ビルドで同一手順 2/2 FAIL(panic、分離成立)。アサートはステータス形式
  `<msg> <line>/<total> (<pct>%)` の完全一致(`Killed buffer <path> 1/1 (0%)`)。
  ゲート APPROVE(Blocker/Major なし。旧ビルドの FAIL は `^x$` 側=insert 時の
  panic と判明。split 後 active は元 leaf のため stale は非 active 側=全 leaf
  リセットが効く経路)。フル e2e 82/82・release 6/6・clippy 0・test 22/22。
  (fix #9 のコミットハッシュは次エントリに記載)
- 2026-10-24: **fix #10(コミット 41d1bf3)**: M-10(C-l の go-to-line 2 パス走査→1 回)。
  行数計数スキャン+先頭からの next_line 反復を、改行数を数えながら `want_ln` 行目の
  行頭を記録する単一スキャンに統合(want が超えたら最終行頭を記録した位置にクランプ、
  M-1 のセマンティクス不変)。到達点は旧実装と同一(先頭 0・列 0 で進む next_line の
  着地=行頭)。ゲート APPROVE(HEAD worktree との A/B 差分テストで旧新バイト一致:
  want=1/正常/want==総数/超過/末尾改行/末尾なし/empty/マルチバイト)。ゲート提案の
  `break`(want_ln 到達で走査打ち切り、C-l 1 が巨大ファイルで O(初行))を適用後、
  全ゲート再実行。検証: pty で C-l 3→3/6、9→6/6(100%)、1→1/6、6→6/6、5→5/6、
  e2e s112/s141 単体 2/2、clippy 0・test 22/22。break 適用後のフル e2e 82/82 solo
  (/tmp/e2e_run17.out)+release スモーク 6/6(/tmp/rs6.out)。
  (fix #10 のコミットハッシュは次エントリに記載)
- 2026-10-24: **fix #11(コミット 1dd2dd6)**: docs クラスタ D-2/D-3/D-4/D-5/D-7/D-10。
  design/README.md=実装済み一覧更新+undo 风险注記+curses 注記。architecture.md=
  「Current implementation (as-built)」節で4点の乖離を明記(gap 本文に相互参照)。
  README=窓表示セマンティクス(D-4)/termsize→curses(D-7)/環境節を docs/dev-env.md へ
  移設しポインタ化+モデル ID 単一源=opencode.json(D-10)。design/keybindings.md=
  M-</M->(D-5)。ドキュメントのみ(コード不変)、cargo build/clippy 警告ゼロ確認。
  (fix #11 のコミットハッシュは次エントリに記載)
- 2026-10-24: **fix #12(コミット 2311b76)**: B-3(kill_to_end_of_line の後方 mark で無音 no-op)。
  kill 境界を「現行内の前方 mark」のみに限定(後方 mark・off-line・mark==point は
  行末まで、改行位置では行結合)。unit 1 追加(3 ケース:後方 mark で行末 kill/改行
  位置で行結合/前方 mark は領域辺で停止)、e2e s154(mark=0 の状態で C-f×3→C-k、
  新=行内容 kill+modified、旧=無音 no-op/無 modified;HEAD worktree で 2/2 FAIL・
  新 2/2 PASS)。README Region 条項更新。clippy 0・test 23/23・フル e2e 83/83 solo
  (/tmp/e2e_run18.out)・release 6/6(/tmp/rs7.out)。ゲート APPROVE(セマンティクス
  行列監査・C-k シナリオに旧 no-op 依存なし確認)。
  (fix #12 のコミットハッシュは次エントリに記載)
- 2026-10-25: **fix #13(コミット 2bf6e09)**: S-3(collapse で tree 配列を再構築し compact 化)。
  旧 collapse は leaf/slot/node を push するだけで split→C-x 1 の反復で
  `kind`/`which`/`nodes`/`leaves` が単調増加(リーク)。新 collapse は生き残る
  leaf(=指定 leaf)の内容だけを保って配列を再構築し(root=0、単一 leaf slot、
  nodes 空)、新しい leaf index(0)を返す。main.rs の C-x 1 は
  `active = tree.collapse(active)` で index を再写す。unit
  collapse_shrinks_to_one_leaf(1 分割→collapse で 1 leaf 化・index 0 化、
  5 回の split+collapse 反復で配列が 1 のまま)追加。e2e 新シナリオなし
  (既存 s102/s105/s122/s145 の C-x 1 フローが回帰保証: 新旧の可視挙動は同一で
  変化は内部スロット回收のみ、unit 側で回帰防止)。clippy 0・test 24/24・
  フル e2e 83/83 solo(/tmp/e2e_run19.out)・release 6/6(/tmp/rs8.out)。
  ゲート APPROVE・新旧 tmux ビルド比較で s102/s105/s122/s145 相当+周辺経路が
  全一致。Minor 記録:(a) 4 フィールドコピーは unit で追記アサート済(point/mark/
  scroll 非 0 で分割→コピー側 collapse)。(b) 可視挙動の実差分として「C-x 1 後の
  C-x C-f」は旧=幽霊 leaf により README 文書規則(単一窓では下分割で新窓に表示)
  に反していた(新は文書どおり)。この経路の e2e シナリオ追加は R-3 の follow-up
  として記録。
  (fix #13 のコミットハッシュは次エントリに記載)
- 2026-10-25: **fix #14(コミット b86d228)**: R-3(window/screen 単体テスト)+fix #13 ゲート follow-up。
  window.rs: layout ジオメトリ(80 列で左 40/境界 col 40/右 39、24 行で 12+境界行+11)、
  狭域フォールバック(行/列境界なしの素分割、幅 1 は非分割)、flatten ネスト順序、
  find_slot 未探索=`kind.len()`、split 未知 leaf=false。screen.rs: split_lines(末尾空行)、
  line_offsets(文字数+改行、マルチバイト=1 文字)、point_pos(行送り/越境クランプ/全角 2/
  結合 0)、char_width 半角カタカナ U+FF71=1。unit 24→30。e2e s155(C-x 1 後の C-x C-f が
  単一窓で下分割・新ウィンドウ表示の README 規則、旧=in-place 置換で境界なし):
  新 3/3 PASS・HEAD~1(2311b76)worktree 3/3 FAIL。フル e2e 84/84・release 6/6 は
  本コミット前に solo 実行で確認(/tmp/e2e_run20.out・/tmp/rs9.out)。ゲート APPROVE
  (スロット代数ハンドトレース・分離再現検証済み)。Minor 織り込み済み: collapse 未知
  leaf=インデッシュ返却+不変アサート、最小境界ジオメトリ(高さ 3=1+行+1、幅 3=1+列+1)、
  split_lines("\n")=["",""]。
  (fix #14 のコミットハッシュは次エントリに記載)
- 2026-10-25: **fix #15(このコミット)**: R-6(CI 導入 + test コード lint 全掃除)。
  `cargo clippy --all-targets` の既存 18 lint(buffer.rs 13: `&"x".to_string()`→`"x"`/
  `cut == "world".to_string()`→`cut == "world"。terminal.rs 5: `== ""`→`.is_empty()`)を
  掃除して 0 に(test コードのみ、非 test コードは不変)。AGENT.md の clippy ゲートを
  `--all-targets` 版に拡大。`.github/workflows/ci.yml` 新規(push:master/PR、ubuntu-24.04
  単一 job: `RUSTFLAGS=-D warnings` build `--all-targets`→clippy `--all-targets --
  -D warnings`→unit→e2e(tmux 同梱。単一 job で tmux 競合回避、release smoke は非収録))。
  `--locked`・`branches:[master]` はゲート Minor 織り込み。curses 0.1.0 は純 Rust
  (jackpot51 系 ANSI curses、C コンパニ不要・libtinfo 非依存とゲートで確認)。
  unit 30/30・`RUSTFLAGS=-D warnings` build/test 通過・clippy `--all-targets` 0。
  非 test コード不変のため e2e 再実行不要(binary 内容同一)。ゲート APPROVE。
  初回 CI 実行(push 40138398806):build/clippy/unit tests=success、e2e のみ FAIL。
  原因は emur 側でなく s133 の read_dir 順序依存(GitHub ランナー fs で候補が
  b,a 順になり Left クランプ先が b 化)。ローカルは a 作成順 fs のため 84/84。
  → fix #16 で s133 を順序非依存化。
  (fix #15 のコミットハッシュは次エントリに記載)
- 2026-10-25: **fix #16(コミット 3a85ace)**: R-6 続・CI 検証で露呈した s133 の
  read_dir 順序依存を除去。Tab 候補リストの assert を 2 順容忍の正規表現にし、
  開くファイルは「最左候補=Left クランプ先」として `[ab]` 幅で検証
  (expect_lastline は厳密一致なので grep 化)。ローカル probe(作成順スワップ)で
  断言が両順序に耐えることを確認(ローカル fs はハッシュ順序のため実 open は a 側、
  b 側の open 経路は CI ランナーの s133 失敗再現=断言の順序非依存性で担保)。
  CI 再検証は本 push の実行で確認。
  (fix #16 のコミットハッシュは次エントリに記載)
- 2026-10-25: **fix #17(コミット 67ecf76)**: R-5(curses 依存を termsize 直依存に置換)。
  curses 0.1.0 の canvas::get_size は `termsize::get().unwrap_or(Size{0,0})` の薄
  ラッパー(レジストリ ソースで確認・ゲートでも再検証)。Cargo.toml を termsize 0.1.9
  (lock 済み版と同版)に置換し、`screen::term_size()` に集約(呼び出し 3 箇所=draw_all/
  read_line_c/search プロンプト、rows==0=非 tty フォールバック意味論は byte 同一)。
  curses は Cargo.lock から消滅(emur deps=libc/termios/termsize、lock 更新をコミット
  により CI の --locked 節と整合)。README 依存記述=termsize 0.1.9 へ更新。
  `[profile.release]` は低優先注記どおり未設定のまま(TODO 記録)。clippy `--all-targets`
  0・unit 30/30・`RUSTFLAGS=-D warnings --locked` 構築可・フル e2e 84/84(run22)・
  release 6/6(rs10)。パイプ時 s140/s142 の 24x80 フォールバックは termsize::get()の
  None 経路で同一(非 tty ゲート確認)。ゲート APPROVE(両コミット)。
  CI 検証:run(67ecf76)=成功(s133 修正+termsize 置換を含む全ステップ green)。
  GitHub ランナーで e2e 84/84=初フルグリーン(fix #16/#17 統合ゲート済み)。
  (fix #17 のコミットハッシュは次エントリに記載)
- 2026-10-25: **fix #18(コミット d0785e4)**: 低コスト簇 B-5/B-6/D-6/D-8。
  B-5=transpose_chars の点 0 分岐(先頭 2 文字 swap)を削除し Emacs 準拠で no-op 化
  (unit transpose_chars_noop_at_start=点 0 で不変+点 2 で h↔i swap・point 進)。
  e2e s156(C-t@0 でデモ 1 行目不変/C-f×2 後の C-t で `Tihs ...`、HEAD=67ecf76
  worktree probe 2/2 実行 FAIL[旧 1 行目=`hTis ...`(swap_at(0) で T↔h)]・
  新 probe 3/3 実行 PASS)。README/keybindings に
  no-op を明記。B-6=README Region 条項に編集での領域消去(挿入系と backspace は
  mark=point、C-d/C-k は維持)を追記(コード実測 3 箇所のみ追従)。D-6=keybindings.md
  凡例 `C-\` 削除・C-d/DEL を Editing 節へ集約(二重掲載解消)。D-8=keybindings.md
  Minibuffer 節に補完射程の前置き(C-s/C-r/C-l は素の入力・Tab リテラル)+README 一文。
  unit 31/31・clippy `--all-targets` 0・build 0・フル e2e 85/85(/tmp/e2e_run23.out)・
  release 6/6(/tmp/rs11.out)。s156 追加による release_smoke 行範囲(1,53/543,545)は
  末尾追加のため不変(実測)。ゲートレビューはコミット前に実施。
  (fix #18 のコミットハッシュは次エントリに記載)
- 2026-10-25: **fix #19(コミット 360c6b3)**: B-10(API スタイル)。buffer.rs の全 29
  メソッド receivers を `&self`/`&mut self` 化。`Buffer::new` を `impl Into<String>` 2 引数に
  (テスト 14・main 6 呼び出しの literal `.to_string()` 消去。&String 引数は明示 clone)。
  terminal.rs の read_line 系 4 ラッパ(read_line/read_line_buffers/read_line_files/
  read_line_eof)の prompt を `impl Into<String>` 化(core read_line_c は String のまま、
  再帰 echo が String を要求)/main の literal prompt 6 箇所 `.to_string()` 消去。
  挙動不変(型変換のみ)。main の to_string 数 51→39・buffer.rs テスト 15→2。
  加えて M-13 を同クラスタで実施(`sync_leaf`/`attach_leaf` ヘルパー集約、上記)。
  ゲート Minor 織り込み=末尾使用の clone 除去(main.rs:90 `Some(path)` 移動/
  :299 `Buffer::new(sel, "")`)。unit 31/31・clippy `--all-targets` 0・build 0・
  フル e2e 85/85・release 6/6=最終ツリーでコミット前に実行(/tmp/e2e_run24.out は
  B-10 単体・run26/rs14 は B-10+M-13 最終)。ゲートレビューはコミット前に実施。
  (fix #19 のコミットハッシュは次エントリに記載)
- 2026-10-25: **fix #20(コミット 0cf0597)**: 注記簇 S-6/S-7/S-9/S-12/T-5/M-12(コメントのみ、
  コード挙動不変)。S-6=scroll 毎フレーム再計算の不変条件、S-7=`rows-2` クランプの
  見かけ 1 行上説明、S-9=`Node.vertical` の向き注記(true=縦積み/false=横並び)、
  S-12=幅<2 同一ジオメトリ重なり容忍の設計、T-5=`save_raw_mode` の fd0 固定/非 tty
  graceful 放弃 doc、M-12=`prev_cmd` last-command 簡略化の範囲。build 0・clippy
  `--all-targets` 0・unit 31/31(コメントのみでコード生成不変のため e2e 再実行は
  対象外。直前の 360c6b3 で CI=success・フル e2e 85/85=ランナー含め確認済み)。
  (fix #20 のコミットハッシュは次エントリに記載)
- 2026-10-25: **fix #21(コミット 26678f3)**: 検査/装飾簇 B-9 残り・H-4・H-5・H-6。
  B-9=unit 3 追加(movement クランプ境界/transpose 末尾/空バッファ編集)。
  H-4=md 未閉結 ` が次行を巻き込まない unit。H-5=数値スキャン修正(英字は 0x の
  直後だけ受理。`2face`→`2` のみ。`0x1F`/`0xDEADbeef`/`1.2` 不変)+unit。
  H-6=HTML tag スキャンの設計注記(属性 string 化なしは意図的)。
  clippy `--all-targets` 0(同一ブロック警告は条件マージで解消)・build 0・unit 36/36・
  フル e2e 85/85・release 6/6=最終ツリーでコミット前実行(/tmp/e2e_run28.out・
  /tmp/rs16.out。run27/rs15 は Major-1 対応前の中間版)。highlight e2e
  シナリオ(s90-s94)は 10 進数値のみで hex 依存なし(スキャン変更に安全)。
  ゲート APPROVE-WITH-CHANGES(Major-1=指数/二進の被装飾減少→復元で対応)。
  (fix #21 のコミットハッシュは次エントリに記載)
- 2026-10-25: **fix #22(コミット 6be8961)**: T-6/T-9(terminal 挙動小 2 件)。
  T-6=Tab 補完 0 候補で `print!("\x07")` BEL(旧=無音)。pty プロブ分離=
  新 BELL_zero:True/BELL_one:False+サフィックス echo 継続、旧(HEAD worktree)
  BELL_zero:False。T-9=save_raw_mode で c_cc[VMIN]=1・VTIME=0 明示
  (ICANON 解除だけでは環境継承の c_cc に依存する)。clippy `--all-targets` 0・
  build 0・unit 36/36・フル e2e 85/85(/tmp/e2e_run29.out=全入力経路の回帰網)・
  release 6/6(/tmp/rs17.out)。
  ゲート APPROVE(コミット 6be8961 後に実施=手順の順序を逸脱。次コミットからゲート
  プレコミット順序に復旧)。Minor-1=root パス `/` の Tab は `file_candidates` が
  dir="" を read_dir("") と呼ぶため元々無一覧(既存欠陥)。新ベルが正しく鳴る。
  T-8 で dir 空文字を `/` に写して一覧化を担保(fix #22 ゲート由来の follow-up)。
  (fix #22 のコミットハッシュは次エントリに記載)
- 2026-10-25: **fix #23(コミット 44fbacd)**: S-5/S-8/H-7(型整理 3 件、挙動・出力 byte 同一)。
  S-5=`Frame` を enum 化(Window/BoundaryRow/BoundaryCol)。境界の dummy `leaf:0` と
  magic `kind:0/1/2` を消去、境界の定数フィールド(Row.height=1/Col.width=1)も削除で
  dead-code 警告を字段除去で解消(`#[allow]` 不使用)。screen.rs prepass/draw は match
  網羅(arm 本体は HEAD と expression 単位の同一=ゲートで token 級検証)。
  S-8=`find_slot` を `Option<usize>` 化(センチネル `kind.len()` 廃止)、split=None で
  早期 false、collapse=`is_some()`、unit は is_some/is_none。
  H-7=`highlight::Style` enum(Plain/Keyword/Str/Comment/Number/Marker)で素 usize 0..5 を
  全置換、`style_esc` は網羅 match(SGR 文字列は HEAD と同一)、`toks: Vec<Style>`。
  言語コード(detect の usize)は対象外として保持。
  ゲート APPROVE(token 級 byte 同一検証・11/11 の style 書き換え 1:1 対応・テスト
  強化のみで弱化なし)。ゲート Minor 織り込み=境界 arm の余分な `..` 除去と match
  インデント統一。証拠=unit 36/36・build/clippy `--all-targets` 0・
  `RUSTFLAGS=-D warnings --locked --all-targets` rc0・フル e2e 85/85(run30、色 7 シナリオ
  含む)+release smoke 6/6(rs18、最終ツリーを forced rebuild で実行)。run30 は
  リネーム前の review diff 内容バイナリで走行、その後の編集は空白のみ+全字段束縛 arm の
  `..` 除去=意味論不変(ゲートが byte-strip 同一で検証)、追加の描画 probe
  (C-x 2/C-x 3 境界行・列・collapse)PASS。
  (fix #23 のコミットハッシュは次エントリに記載)
- 2026-10-25: **fix #24(このコミット)**: T-8(file 補完の細部 3 点+root 一覧)。
  `file_candidates` に (1) `cands.sort()` で辞書順(read_dir 順の露出を排除)、
  (2) dir エントリへ末尾 `/`(`DirEntry::file_type` は symlink を追わないため
  merged-usr の `/bin` はスラッシュ無し=仕様。unit/e2e は実 dir の `/usr` で検証)、
  (3) `expand_tilde` 新設で fs 側 dir を展開(`~` と `~/...` のみ、`~user` は
  リテラル。一覧エコーは入力 head を保持するので `~/emur`+Tab は `~/emur-t8-...`
  と続く)、(4) `dir==""` を `/` に写して root 一覧(fix #22 ゲート Minor-1)。
  補間の prefix フィルタは素名 vs tail のまま(`completions()` が全文を全文と
  再検査して一致。full 文字列比較へ寄せると `~/emor` 型の入力が壊れる。
  デバッグで `cands=0` の原因はテスト側のタイポ `~/emor` と判定、実装は素名比較に据え置き)。
  main.rs は C-x C-f/C-x C-w/C-x C-i の入力直後に `expand_tilde`(バファ名/保存先は実パス)。
  unit 2 件追加(sort/dir slash/root の `/usr/`/tilde 規則)=38/38、e2e s157-s160
  登録(89 シナリオ)。s160 は一覧トークンが head の `/` を保つため pattern は
  `' /usr/'`(先頭スラッシュ必須。`' usr/'` は永久に一致せず)。s159 は file 名
  `emur-t8-marker.txt` に対し入力が `~/emor` のタイポで FAIL していた(実装不具合なし)。
  旧バイナリ(44fbacd worktree)で s157-s160 全 FAIL の分離確認済み(旧は readdir 順・
  スラッシュ無し・`~` リテラル・root 無一覧)。README/keybindings.md に一覧順/
  dir `/`/`~` 展開/ベル/EISDIR 失敗を追記。ゲート APPROVE 後コミット。
  証拠=unit 38/38・build/clippy `--all-targets` 0・`RUSTFLAGS=-D warnings --locked
  --all-targets` rc0・フル e2e 89/89(run31)+release smoke 6/6(rs19)。
  (fix #24 のコミットハッシュは次エントリに記載)
