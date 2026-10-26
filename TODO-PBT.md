# TODO-PBT(emur PBT テスト作業台帳)

PBT(Property-Based Testing)で emur を検証する作業の進捗台帳。
設計文書=英文 `test-design/pbt-design/`(README が索引)。
不具合記録=英文 `test-design/pbt-design/found-bugs.md`。
テストコード=`pbt-test/`(新規 cargo パッケージ)。

## 規約(既存台帳と同規律)

- 設計・実装の各段階でサブエージェント・ゲート(設計ゲート=DOC 監査、
  コードゲート=プレコミット)。APPROVE 前にコミットしない。
- 1 指摘=1 コミット、コミットごとに push、CI 緑確認。
- 警告ゲート:`cargo build`/`cargo test`/`cargo clippy --all-targets` 警告ゼロ
  (ルート=emur、pbt-test とも)。`#[allow]` 禁止。
  依存 crate が `RUSTFLAGS=-D warnings` で警告を出す場合のみ、
  02 §Build gates の事前登録にしたがって build ゲートを平文 build+grep 0 に
  落とし、この台帳に逸脱を記録する。
- tmux スイート(e2e / release smoke / PBT tmux バイナリ)は直列。
  実行中は cargo ビルド・他 tmux・該当スクリプト編集をしない。
- コミットメッセージにバックティック/`$(` を入れない(`git commit -F` 使用可)。

## Phase A: 設計(DOC 先行)

- [x] A1 `test-design/pbt-design/` に設計文書 7 点を書く
      (README / 01 overview / 02 package / 03 properties / 04 tmux harness /
      05 CI / found-bugs 雛形)(コミット a616eaa)
- [x] A2 設計ゲート(サブエージェント)を HEAD 実装と照合して実行
      → APPROVE-WITH-CHANGES(Blocker 1/Major 4/Minor 10、実 tmux・実 scratch
      workspace・proptest 1.12 ソース読みによる検証付き)
- [x] A3 ゲート指摘(Blocker/Major/Minor)を全件織り込み(01-05・found-bugs
      更新)。Blocker=gap window の `'\0'` padding 主張は実測で偽
      → 構造不変条件に置換、found-bugs に PBTF-001(NOT-A-BUG)記録

## Phase B: テスト基盤

- [x] B1 `pbt-test/` 雛形(package、`[workspace] default-members=["."]`、
      `proptest` 追加)で `cargo build -p pbt-test` が通る
      (ゲート MA-2 により lib.rs は作らず **test target 別 `#[path]` include**。
      src/ ディレクトリ無し=lib target 無し)
- [x] B2 harness(`tests/support`):セッション起動/終了(Drop ガード)、
      起動時 pbt-* 掃討(+掃討後 settle と new-session 1 回再試行=PBTF-005)、
      `send_hex`、`send_text`(=UTF-8→hex 変換の -H 経由=PBTF-003)、
      `capture`/`capture_poll`、`pane_sane`/`pane_sane_lax`、EMUR_BIN 解決、
      `PBT_CASES` 読込、`cfg()`(timeout=0=PBTF-002 逸脱、max_shrink_time 15s)
- [x] B3 ゲート:pbt-test build/clippy 警告ゼロ・`#[allow]` 無し・
      `RUSTFLAGS=-D warnings cargo build -p pbt-test --all-targets` rc=0・
      ルート `cargo test`(既定=emur のみ、42 件)は影響ゼロ(実測)

## Phase C: P0(gap buffer 差分テスト、tmux 無し)

- [x] C1 naive モデル実装(op 全メソッド+point/mark+modified+kill 返り値
      +find 返り値+gap 窓の健全性+len 同一性+char_at/to_index/line_* 全走査)
- [x] C2 P0 実装+512 ケース通過(include された buffer 単体 14 件も同時実行)
- [x] C3 プリコミット・ゲート → コミット/ push(D5 と combined で実施済み
      → コミット 17e6637)

## Phase D: P1-P4(tmux 統合ファジング)

- [x] D1 P1 raw byte fuzz(クラッシュフリー+画面健全性、0x03 除外)
- [x] D2 P2 編集ラウンドトリップ(モデル replica 一致+ステータ行厳密一致
      +保存+seed 不変。sync_in の point/mark clamp をモデルに反映=PBTF-006)
- [x] D3 P3 window 構造ランダムウォーク(split/collapse 境界=
      「行中に 8 連の '-'」判定。ダッシュ行は全行 '-' でない=PBTF-004)
- [x] D4 P4 resize ファジング(狭幅でステータ行が折り返すため
      pane_sane_lax=ステータ検査無し。03 §P4 の設計逸脱として明記)
- [x] D5 プリコミット・ゲート(コードゲート APPROVE-WITH-CHANGES:Blocker 1=
      CI EMUR_BIN/Minor 3。全件織り込み+実測確認)→ コミット/ push 17e6637
- [x] D6 `test-design/pbt-run.sh` 実装(直列実行の単一分入口)

## Phase E: 本走・トリアージ・不具合対応

- [x] E1 全プロパティ連続 3 回フル走(ローカル、直列)= 3 周とも rc=0
      (P0 512+P1 24+P2 24+P3 16+P4 16、各 ~160 秒)。
      過去走の失敗(PBTF-006/007)は修正織り込み後に消滅
- [x] E2 失敗は全て found-bugs.md に再現手順付きで記録(モデル側不具合も
      「NOT-A-BUG」判定根拠を明記)→ PBTF-001〜006(emur 実バグ 0 件)
- [x] E3 emur 実バグは 1 件=1 コミットで修正 → 該当なし(6 件全部
      NOT-A-BUG:テスト基盤/モデル/文書側の缺陷)。実バグ候補は検出されず
- [x] E4 修正後は該当プロパティ回帰+ regressions 再走
      (P2=モデル修正後 24 件×3 周緑。P3=判定式修正後 12 件×4 周緑)

## Phase F: CI 統合と締め

- [x] F1 `ci.yml` に PBT ステップ追加(05 の並び)+ ジョブ
      `timeout-minutes: 20`(timeout=0 逸脱の外側安全網)。CI 緑確認は push 後
- [x] F2 学習記録の追記(台帳末尾)
- [x] F3 変更履歴の最終化(コミットハッシュ・バックフィル済み。
      found-bugs PBTF-002〜007 の Fix 欄に 17e6637 を明記)

## Phase G: 結合テスト設計(ターミナル全体のモデル照合)- ユーザ指示 2026-10-25

背景:既存 P0-P4 はモデルが buffer レベルのみ(P2)で、P3/P4 は構造
クオリタティブ判定(ダッシュ行/生存性)に留まる。ウィンドウ分割と
ミニバッファ周りは過去のバグ多発領域。ユーザ認識(=本タスクの前提):
モデルを使う目的は「モデルがキー入力で描画するもの」と「tmux が同一
キー入力で描画するもの」を照らし合わせること。**よってターミナル全体
(ウィンドウツリー几何・各リーフの mode line/ステータス行・echo 領域・
バウンドリ文字)をモデル化し、全面差分を取る**。各種キーバインドは
「1 操作=1 op」として列に含め、分割直後・ミニバッファ中・プレフィックス
待受などのタイミングでの C-x/Meta/C-g/バッファ選択が正しく動くことを
検証する。

- [x] G1 操作面の網羅棚卸:plain/C-x/Meta 全バインド+プロンプト状態
      (C-x b 選択+Tab 補完、C-x C-f/C-w/C-i、C-x k yes|no、C-l、
      C-s isearch、C-r、C-c 終了確認、C-g 取消)を 1 操作=1 op で列挙。
      分割直後の C-x、ミニバッファ中のバッファ選択、各時点の C-g を
      列として明示(01 §3 / 03 §P5-P7 で確定)
- [x] G2 モデル範囲決定:全面描画モデル(80x24 グリッド=text 領域+
      境界+リーフ別モード行+echo 行)、マルチバッファ/エイリアス状態、
      ツリー几何(リーフ point/mark/scroll)、ミニバッファ状態機械。
      照合面=capture-pane プレーンテキスト(色は別レイヤのスポット)。
      既存 P2 との違い(buffer-only → 全面)を明文化(02)
- [x] G3 プロパティ設計:P5=全面描画差分(生成列を model と emur が
      同じキー入力で実行→グリッド一致)、P6=ミニバッファ状態遷移
      (入力/Tab/yes|no/C-g/EOF)、P7=分割×プレフィックス列挙交互(03)
- [x] G4 `test-design/pbt-design-combination/` に英文マークダウン群で
      書き出し(README/01 overview/02 model/03 properties/04 harness/
      05 known-gaps。found-bugs は既存 pbt-design/found-bugs.md 継続=英文)。
      設計中の追加判断:window.rs/buffer.rs は `#[path]` で逐語 include(純粋)、
      screen.rs/terminal.rs は入出力境界のため I/O 制約と MA-2 dead_code を避け
      **モデル側ミラー**(規則は screen.rs/terminal.rs 行番号付きで対応表化)。
      全面差分=80x24 グリッド vs capture(行末 trim 法)
- [x] G4b 設計中の追加発見:C-x C-s はプロンプト無し(bound filename /
      未bound=No file name for this buffer)=P5 op 追加済み
- [x] G5 設計ゲート(サブエージェント、HEAD 実装照合)=APPROVE-WITH-CHANGES
      → 全件織り込み済み(Major J1-J6/Minor m1-m13、計 19 件):
      J1 `@`=単字+余白(draw_cell 246 実態、全文字@行は全 EOB 行で誤検出)、
      J2 C-x C-s と seed 不変条件の矛盾→生成器規則(seed 結合 buffer に出さない)、
      J3 mid-prompt echo の 80 cell 超ラップ→echo バジェット法+P6 seed 短縮、
      J4 isearch ループ全 arm(DEL pop/C-s repeat/ESC 矢印/MISS は qs から落ちる)追記、
      J5 support 3 層化(core/sess/narrow)で MA-2 使用地図を全 item 自然使用に確定、
      J6 plain C-g(Cancelled)を P5 op 追加、m1=emur_bin/tmux は private、
      m2=SETTLE は 4 個、m3=s90-s94/s140-s142 の引用正確化、m4-m13=引用/文言修正。
      ゲートが HEAD で TRUE を確認した重要主張:P6 の「listform C-x C-b + C-g は
      Cancelled でなく CYCLS する」(main.rs:410-419)、window.rs 純粋性、
      ENOENT 文字列予測可能性、checkpoints の宙吊りなし
- [x] G6 設計文書コミット=commit 0710208(design docs 6 点)+push+CI 緑確認済み

## Phase H: 結合テスト実装

- [x] H1 実装 `pbt-test/tests/combination.rs`(約 2200 行、新規 test
      target。P5 全面差分 25 ケース/P6 ミニバッファ機械 14 ケース/
      P7 分割×プレフィックス 5x8 行列+60x20/P7b。support 3 層化
      core/sess/sanity を同時実装。tmux スイート直列規律は既存と同一)
- [x] H2 本走+トリアージ:全 25 ターゲット緑(183.99 秒、--test-threads=1)。
      発散トリアージで修正したのは全てテスト側のミラー忠実度
      (Sf=0x18 0x13、sync_in の leaf→buffer コピー、isearch last 法、
      run_tail 法、find-file reuse=active leaf、kill 未変更の echo 順序、
      case11 send_hex 漏れ)。**emur 実バグ 0 件=新 PBTF なし**
- [x] H3 CI 統合(ci.yml に P5-P7 ステップ:PBT_CASES=8、
      EMUR_BIN=../target/debug/emur、--test-threads=1)+pbt-run.sh 第 3 段
- [x] H4 台帳締め:学習記録+変更履歴+ハッシュ確定(df47fbe 実装+
      ab76d62 ゲート織り込み)。コードゲート PASS・全ローカル直列緑
      (pbt-run 6m39s/e2e 3m42s/release_smoke PASS)。CI 緑確認済み

## 学習記録

- 2026-10-26(ハイライト/SGR):属性レイヤは**全面 grid テキスト比較を
  素通りする**(capture-pane -p はテキストのみ)→ P5-P7 模型変更不要、
  ピンは e2e の `-e` capture(expect_e)で行う。ウィンドウ単位の描画状態は
  **leaf 所有フィールドから読む**-`bufs[bidx].mark` は直近同期分であり、
  ループ頭の frame() は sync_in より先に走るためウィンドウ切替直後の
  1 帧だけ 1 コマンド遅延する(ゴースト region の実体=live 分割プローブ
  でしか拾えないクラス)。e2e フル走の flake は tmux サーバ側积累
  (生存セッション数十+ホスト負荷)で**失敗箇所が移動**する=単独再走全緑
  ならタイミング級と判じてよい(s63/s28/s130 が移動、r20 全緑)。
- 2026-10-26(C-v/M-v/C-w+isearch 統一):tmux capture-pane は**カーソル下の
  blank をキャプチャから落とす**ため、leading space を含む行アサーションは
  カーソルを退避させてから capture する。`expect_lastline` は完全一致で
  正規表現ではない(`\(` を渡すと静かに全滅する)。プレフィックス map
  (meta/C-x)生成のコマンドは**そのプレフィックス dispatch 側の match に
  arm が要る**plain match だけでは `_ => {}` に呑まれて無言。exhaustiveness
  チェックはこの取りこぼしを検出しない。isearch の EOF はミニバッファ EOF
  法(Cancelled)と揃えるのが一貫(既存 e2e ピンがそれを要求していた)。
- 2026-10-26(P3 flake/PBTF-008):定性的プロパティの断言は**設計の不変
  条件**であって**seed の文字列リテラル**であってはならない。分割ツリー
  のエイリアス leaf は point 独立(window.rs:151-156)なので、split 時
  copy の stale point(=split までの挿入数)でタイプすると seed 語
  "alpha" の内部(2-4 文字目)に正当に着弾し、部分文字列は消える
  (status 1/3 12% は pt3/len24 と完全一致=自己整合フレーム)。P3 の
  `pane.contains("alpha")` は buffer-only 時代の過強断言で、約 6 回の
  CI 緑後に初めて割れた(proptest seed は実行毎乱数=潜伏期間が長い)。
  弱体化=ウィンドウ行に英字が 1 つ(=collapse 全 @ 化の検出=元来意図)。
  陽な leaf-point 独立性は P5 全面グリッドが正面から固定済み。
- 2026-10-26(結合テスト実装):モデルミラーの発散は「キーの意味」では
  なく**ループの境界規則**に潜む。(1) 本番 `sync_in` は clamp 後に
  leaf→buffer へ**コピー**する(main.rs:171-184)。模型が clamp だけ
  真似ると、エイリアス leaf の point が(buffer 経由で)他 leaf に
  伝播する列でステータス `%` がズレる。(2) プロンプト読取が返った
  後に残ったバイトは**別コマンド**としてメインループに流れる
  (コマンドごとに sync_out→sync_in が挟まる)。C-x C-b 取消→Enter の
  挿入先が 1 コマンド後の CYCLED leaf になる実態を模型は run_tail で
  再現する。(3) 未変更バッファの kill はプロンプトが出ないため
  解答バイト `yes\r` が全文字メインループに流れ、fresh バッファで
  Enter→y→e→s 順にタイプされる。apply が tail を返す設計で再現。
- 2026-10-26(結合テスト実装):送信バイト表は記憶で作らず **keymap.rs
  から導出**せよ。C-x C-s を 0x18 0x06(C-x C-f)で送った事故は、
  開いた Find-file プロンプトの読取が**次の op のバイト**を吸い込み、
  まるで模型側が壊れたかのような発散( phantom "Find file:" ペイン)を
  数日間発生させた。原因は常に送信側、症状は受信側に現れる。
- 2026-10-26(結合テスト実装):isearch の `last` はエントリ地点から
  開始し(main.rs:437)、ヒット後は**マッチ開始位置**(`end - q.len()`)
  へ更新する(503)。模型が `end`(マッチ終端)を保持すると次の
  文字の探索開始が 1 文字遅れ、`C-a` の line マーク規則と相まって
  ステータス % が発散した。なお `read_char_eof` は **EOF でしか
  None を返さない**(terminal.rs:135-160):isearch 中の制御バイトは
  query 文字として bell+破棄+続行であり、ループ終端ではない
  (コードゲートで模型の break 実装を検出・修正。ASCII tail 到達
  不能なのでテストは不変)。
- 2026-10-26(結合テスト実装):「観察可能」がモデル比較の境界を決める。
  C-g 直後の `" [Cancelled]"` echo は全面再描画が即着くため**消滅
  一瞬**で poll でも捕まらない→mid 比較は C-g **前**の安定状態に切る。
  長い msg はステータス行を pane 幅で truncation するため、P1 の
  末尾 parser は truncation 行では無効(P5 の tail 検証は幅ガード付き)。
  sanity も grid 全面差分より弱い検査(tail parser)は採用しない。
- 2026-10-26(結合テスト実装):tmux `send-keys -H` は**キー引数ゼロ
  なら Enter を送る**。空 `send_hex` は絶対禁止。模型はバイトを送ら
  ず、driver が送る(`run_prompt` は model-only。呼び出し側が
  `send_hex` と対で組む)と法則化すると、模型実行のみでペインが
  起動時ままというケース11 の発散が一目で Identified になった。

- 2026-10-25(本走):プロンプト開放の「おまじなし末尾バイト」は最悪消費を
  越えねばならない。isearch の printable 経路は read_char_eof を通るので、
  3 バイト UTF-8 lead(0xE7)が C-g サフィックス 2 枚を continuation として
  合法的に食べる=サフィックスは 4 枚(3 補欠+1 実弾)。ファジングの
  フラッシュ列は「状態が消費できる最大バイト数+1」で設計する。
- 2026-10-25(コードゲート):`cargo test` の test プロセス CWD はパッケージ
  dir(pbt-test/)でジョブのルートではない。相対 EMUR_BIN はそこから解かれ、
  `target/debug/emur` は実測「binary not found」→ `../target/debug/emur` が
  正(CI Blocker として検出、実測で確認)。フォールバックの
  CARGO_MANIFEST_DIR 基準は同じ場所に着地するので unset 時は invisible。
- 2026-10-25(コードゲート):ゲートが CI 設定の Blocker を 1 件検出
  (EMUR_BIN 相対基準)。ローカル全走が EMUR_BIN 未設定で緑だったため、
  実走では絶対に現れなかったクラス=CI 専用経路はゲート/実測 probe のみ
  が拾える(PBT で CI 設定はテストできない)。

- 2026-10-25(実装):fork モードの子は「親のメモリnapshot」を継承する。
  セッション名カウンタ(SEQ)も扫讨 Once も継承されるので、全子が同じ名前で
  new-session を試し(=duplicate session)、扫讨は永久に実行されない。
  Drop ガード付き資源を持つファジングの直列化は in-process +
  `--test-threads=1` でやるのが正しく、per-case timeout は CI ジョブの
  timeout-minutes に委ねる(トレードオフを台帳に明記)。
- 2026-10-25(実装):tmux の send-keys は「コマンド文字列に argv を結合」
  する経路を通るため、**単独引数の `;` はコマンド区切りとして消える**
  (`send-keys -l -- ';'` は何も送らない。`A;B` なら無事)。入力チャネルは
  全部 `-H` ヘックスに統一した。P2 のモデル等価ファジングが 7 op の
  最小再現にシュリンクしてこれを検出した(PBT の本懐)。
- 2026-10-25(実装):モデル replica は「コマンド意味の_mirror」では足りない。
  emur のループは毎反復 frame→**sync_in(point/mark を len に clamp、
  leaf⇔buffer 双方向)**→dispatch なので、モデルも op 毎に同じ clamp を
  再現しないと「insert で mark を前に置いてから shrink 系 op」の列で
  forward_char の mark-jump がズレる。ループ不変込みでミラーせよ。
- 2026-10-25(実装):tmux 側で内部状態を読む技術=ステータ行の P% から
  point を逆算 + `tmux display-message -p '#{cursor_x} #{cursor_y}'` で
  カーソル列を直接取得 + 一時 eprintln(要 revert・git diff ゼロ確認)。
  3 段構えで 1 件のズレを step 単位に特定した。
- 2026-10-25(実装):最後のセッションを kill すると tmux サーバ自体が
  降り、即 new-session が「server exited unexpectedly」で失敗する競合が
  ある。扫討後の settle と 1 回再試行を harness に入れた。
- 2026-10-25(実装):ネスト分割のダッシュ行は「行のどこか 8 連の '-'」で
  しかないと判定できない(左列=`----|@`、右列=`@|----`、全幅=`----`)。
  行頭・行末アンカーの境界判定はレイアウト幾何で壊れる。

- 2026-10-25(設計ゲート):ゲートが scratch workspace で実証してくれた 3 件は
  文書レビューの理想形。(1) proptest の設定名を当てるな、ソースを見ろ
  (`workers` 等は proptest 1.x に存在しない。直列化のレバーは libtest の
  `--test-threads=1` と fork モードの子直列のみ)。(2) 「include して使えば
  警告ゼロ」は成立しない:bin crate 免除の clippy lint が lib crate で発火し、
  逆に target 別 include だと未使用 pub API が dead_code になる。
  「pub API 全網羅+モデルが全 field を読む」という制約を P0 に埋め込んで
  解決=制約をテストに刻印する設計。(3) ソースコードの不変条件コメントは
  実測对象(`'\0'` padding 主張は削除残字符で偽。真の契約は「ライブテキストが
  窓に露出しない」こと)。
- 2026-10-25(設計ゲート):`timeout` を持つ PBT は fork モードになり、
  子殺しは Drop ガードを飛ばす。セッション名を固定プレフィックスにして
  プロセス起動時に自前掃討する設計が、ガード不能の漏れを吸収する唯一の
  安全網になる(tmux 系テストの一般形として)。

- 2026-10-25(設計前):参考資料の実装イメージ(tmux を 1 キーごとに
  起動し sleeps で待つ素朴構成)は e2e 実測の反面教師になった。
  emur の e2e(harness 1-53 行)で実証済みの方式=セッション 1 個+
  `send-keys` バースト(inter-op sleep 無し)+バースト後 settle が速い
  (1 ケース 2-3 秒)。プロンプトを開くコマンド(C-l 等)はバースト中に
  後続バイトを吸い込むため、P2/P3/P4 は「プロンプト無しで成立する操作
  表」に限定し、P1 はそれを許す(=プロンプト乱入ファジング)と分割した。
- 2026-10-25(設計):モデル replica はEmacs 意味の再実装を避けて、
  **emur 自身の buffer.rs を `#[path]` include** する方針。buffer.rs は
  `use`/`mod` 宣言ゼロ(std のみ)なので include 可能(実測確認)。
  ディスパッチ側の意味(kill 連打ち prev_cmd==0x0b、yank、C-SPC mark)は
  03 の対応表で薄いミラーを作る。この構造では P2 は「buffer.rs 内部の
  意味バグ」を原理的に検出できないので、検出対象を 01 に明記し、
  内部は P0(naive Vec<char> 差分)が担当する役割分担にした。
- 2026-10-25(設計):`0x03`(C-c)をファジングから除外した理由=
  終了確認プロンプトに乱入すると「yes」の偶発入力でセッションが
  正常終了してしまい生存性不変条件が成立しないため。confirm 経路は
  e2e s140-s142 が固定済みなので欠く対象ではない。
- 2026-10-25(設計):`[workspace]` 追加は既存 CI ゲートの意味を変えうる。
  `default-members=["."]` でルート コマンドは emur のみのまま据え置き、
  PBT は `-p pbt-test` の明示ステップで回す(05)。tmux PBT が CI の
  並列 `cargo test` に紛れ込む事故をビルド設定で防ぐ設計。

## 変更履歴

- 2026-10-26: isearch マッチ+region のリアルタイム背景ハイライト=コミット
  1b42fee(CI 緑)の
  (Style::Isearch=43m 黄/Style::Region=46m 青、screen.rs `apply_span`、
  `Ed::isearch_hi` で C-s/C-r の現マッチを**全ウィンドウ**描画、region は
  active leaf の **leaf 所有** point/mark から描画=ウィンドウ切替直後の
  bufs.mark 1 コマンド遅延ゴーストを回避、マッチ表示中は region 抑止、
  DEL でクエリ空になると描画解除。テキストは無変更=属性のみ)。
  e2e に s165(43m ライブ+延長+Enter で消える)/s166(46m ライブ+
  C-@ で消える)追加=95 シナリオ。unit +1(apply_span intersection)=54。
  コードゲート 2 回:1 回目に MAJOR 2 件(①isearch 描画が active leaf
  のみに限定=alias window 描画漏れ、②`bufs[bidx].mark` 使用で切替直後の
  入力側ウィンドウにゴースト region)→全て织り込み再ゲート 6/6 PASS。
  検証:unit 54、combination P5/P6/P7/P7b 全緑(grid はテキストのみ=
  属性は不感知。05 §15 に登記)、e2e 95/95 緑(r18/r19 で s63/s28/s130 が
  移動 flake=負荷タイミング級。単独全緑+フル r20 緑)、pbt-run 全段緑、
  release_smoke PASS。
- 2026-10-26: C-v/M-v/C-w 実装+C-s/C-r 統一増分 isearch ループ=コミット cfc4404
  (CI 緑)のプロダクト
  keymap.rs/buffer.rs/main.rs+README 実態化)と P5 combination ミラー追従
  (模型 isearch(tail, backward)+search_step、Rf op は C-r 直 tail 経路へ
  変更=read_line/apply_rf/prompt_run Rf 分支削除、新 op Cv/Mv/Cw=pool
  weight 2)、e2e に s162(page)/s163(kill-region)/s164(C-r 増分+C-s 方向
  切替)追加=93 シナリオ。実装で拾った落とし穴 2 件=
  ①ScrollPageUp は meta_key 生成なので PrefixMeta match に arm が要る
  (plain match だけだと `_ => {}` に呑まれ無言。s162 が検出)、
  ②isearch の EOF は mini バッファ EOF 法(Cancelled)に統一(s142 既存ピン
  が緑のまま通る)。キャプチャの罠=カーソル下 blank は tmux capture から
  消える(s163 は C-f で退避)。`expect_lastline` は完全一致で正規表現では
  ない(s163/164 初版の `\(` エスケープが全滅)。
  検証:unit 53 緑、combination P5/P6/P7/P7b 全緑(全直列)、
  e2e 93/93 緑、コードゲート PASS・Minor 4 件全織り込み(README の
  `Search string not found` 例を現存 msg へ、01-overview 93、
  02-model の C-w 区間式と isearch 行指標、03-properties C-g 行指標)。
- 2026-10-26: P3 CI flake 対応=コミット aa5995e:CI run 40243973196(df47fbe)
  の p3_window_walk 失敗をトリアージ=**プロダクト正常・テスト側過強断言**
  (PBTF-008 登記)。断言を `pane.contains("alpha")` →
  「ウィンドウ行に英字(text-gone=全 @ 検出の本来意図)」へ弱体化。
  再現=ローカル 3 周目(PBT_CASES=8)。pbt-design/03 §P3 更新。
  検証:p3 4 周(PBT_CASES=16)+tmux_fuzz フル緑。CI 実証:df47fbe の
  p3 失敗は初回フラケ(ab76d62/3dcd5ad/aa5995e 連続緑、P5-P7 CI
  ステップも runner 初緑 7m15s)。コードゲート済み(PASS)。
- 2026-10-26: コードゲート(ミラー忠実度レビュー)=PASS・BLOCKER 0/Minor 7
  全件織り込み=コミット ab76d62。模型側修正=isearch 制御バイト arm を
  本番実態へ(bell+破棄+続行。break は EOF のみ main.rs:488-490)、
  ESC の 1 バイト無条件消費(452-465)、コメント実在順序修正(kill
  tail=y,e,s,Enter 着順)、引用 490→503、file_candidates の CWD
  arm 注記(production は dir="." 既定、模型は絶対パス pool で
  非到達)、capture_poll 文書 4x 化、send_hex 空ベクトル assert
  (tmux -H 法 05 §13)。再走:combination 全 25 緑 192.97s。
  実装一式=コミット df47fbe、ゲート織り込み=ab76d62。
- 2026-10-26: 結合テスト(P5-P7)実装一式:this commit。`combination.rs`
  新規(support 3 層化 core/sess/sanity 併載:mod.rs 経由で tmux_fuzz は
  sanity を、combination は core+sess のみ+ローカル comb_sane を使用。
  `Session::last_line` は `core::last_line` へ移し tmux_fuzz 呼び出し側
  追従)。pbt-run.sh 第 3 段=combination。ci.yml に P5-P7 ステップ。
  設計文書 02/03/04/05 を実装実態へ更新(sync_in コピー法/run_tail 法/
  isearch last 法/echo chunk 法/status truncation/tmux -H 法)。
  実測ローカル全直列:P0 5.5s・P1-P4 164s・P5-P7 196.7s(pbt-run.sh 計
  6m39s)、e2e 3m42s、release_smoke 22s、すべて rc=0。
  P5 25 件 59.68s・P6 14 件 28.71s・P7 行列 97.83s・P7b 5.92s
  (計 183.99s)。emur 本体修正 0 件(発散は全てテスト側ミラー忠実度、
  新 PBTF なし)。設計文書=コミット 0710208(CI 緑)。
- 2026-10-25: コードゲート APPROVE-WITH-CHANGES(Blocker 1/Minor 3)。
  Blocker=CI の EMUR_BIN 相対基準(cargo test の CWD=パッケージ dir)→
  `../target/debug/emur` に修正+実測確認(02/05 反映)。Minor=tmux_fuzz
  ヘッダの fork モード記述を timeout=0 実情に更新。本走 2 周目の P1 失敗は
  isearch 中の 3-byte lead が C-g サフィックス 2 枚を消費(PBTF-007)→
  サフィックス 4 枚化(03 §P1/04 §3 反映)。フル 3 連続走=3 周とも rc=0。
  このゲート織り込み分=コミット 17e6637(implementation と同コミット)。
- 2026-10-25: PBT 実装一式(コミット 17e6637:pbt-test スキャフォールド+support harness+
  P0 差分テスト+P1-P4 tmux ファジング+pbt-run.sh+ルート workspace 化+
  Cargo.lock+ci.yml PBT ステップ/timeout-minutes)。設計からの逸脱は
  found-bugs PBTF-002(timeout=0)/003(send_text=-H hex)/004(ダッシュ行
  判定)/005(扫討 race)/006(モデルの sync_in clamp 反映)として登記済み。
  emur 本体の修正は 0 件(実バグ未検出)。e2e.sh・release_smoke.sh 不変。

- 2026-10-25: TODO-PBT 作成+設計文書 7 点(コミット a616eaa)。
- 2026-10-25: 設計ゲート APPROVE-WITH-CHANGES 全件織り込み(このコミット)。
  Blocker: gap window `'\0'` 不変条件は実測で崩れる(削除残字符が窓内に入る)
  → P0 は「窓の健全性+acc 等価」に置換し PBTF-001(NOT-A-BUG)登記。
  Major: proptest 1.x に `workers`/`max_concurrent_tests` は存在せず
  (直列化は `--test-threads=1` のみ)、`timeout>0` は fork モードを暗黙有効化
  (fork 子殺しのセッション漏れは起動時掃討が吸収)、`max_shrink_time` 指定で
  シュリンク走を節約、lib.rs include は clippy `len_without_is_empty` で落ち、
  target 別 include は未使用 pub API が dead_code になるため P0 op list を
  pub API 全網羅(find 2 件+name/filename 読み)に。
  Minor: e2e 引用は s111+s140/s142、capture は末尾余白 trim、ルート CI
  ステップは `default-members` で不変(探针確認)、proptest 1.12×rustc 1.99 の
  `RUSTFLAGS=-D warnings` は绿灯(ゲート免除条項は休眠)。
