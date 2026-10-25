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
- [ ] C3 プリコミット・ゲート → コミット/ push(D とcombined で実施)

## Phase D: P1-P4(tmux 統合ファジング)

- [x] D1 P1 raw byte fuzz(クラッシュフリー+画面健全性、0x03 除外)
- [x] D2 P2 編集ラウンドトリップ(モデル replica 一致+ステータ行厳密一致
      +保存+seed 不変。sync_in の point/mark clamp をモデルに反映=PBTF-006)
- [x] D3 P3 window 構造ランダムウォーク(split/collapse 境界=
      「行中に 8 連の '-'」判定。ダッシュ行は全行 '-' でない=PBTF-004)
- [x] D4 P4 resize ファジング(狭幅でステータ行が折り返すため
      pane_sane_lax=ステータ検査無し。03 §P4 の設計逸脱として明記)
- [ ] D5 プリコミット・ゲート → コミット/ push
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
- [ ] F3 変更履歴の最終化(コミットハッシュ・バックフィル)

## 学習記録

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

- 2026-10-25: コードゲート APPROVE-WITH-CHANGES(Blocker 1/Minor 3)。
  Blocker=CI の EMUR_BIN 相対基準(cargo test の CWD=パッケージ dir)→
  `../target/debug/emur` に修正+実測確認(02/05 反映)。Minor=tmux_fuzz
  ヘッダの fork モード記述を timeout=0 実情に更新。本走 2 周目の P1 失敗は
  isearch 中の 3-byte lead が C-g サフィックス 2 枚を消費(PBTF-007)→
  サフィックス 4 枚化(03 §P1/04 §3 反映)。フル 3 連続走はゲート後実施。
- 2026-10-25: PBT 実装一式(pbt-test スキャフォールド+support harness+
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
