# TODO-UNDO(emur Undo キーバインディング作業台帳)

`C-/`(0x1f)で Undo を行うキーバインディングの実装と、各種テストへの
組み込み・再テストの進捗台帳。既存の規約(警告ゲート・tmux 直列・
ゲート・コミット規律)は TODO-PBT.md と同一。

## 設計メモ(実装前に確定)

- 方式=**全状態スナップショット**スタック。text+point+mark を 1 entry として
  保存し、restore で復元(元に戻し後の編集も gap 再構築で正常)。
- スナップショット機構は `src/buffer.rs`(`snapshot()`/`restore()`/`UndoState`)、
  ** スタックとグループ化は dispatch 層(`main.rs` の `Ed`)**。
  グループ化はコマンド列の知識が必要で buffer は知らないため。
- グループ tag:1=タイピング連続(self-insert / `C-j`/`C-m` / `C-q`)、
  2=`C-k` 連続(kill run)、0=単発変更(`C-d`/BS/`C-o`/`C-t`/`C-y`/insert-file)。
  連続 same-tag はランの先頭 pre-state にマージ(1 Undo=1 ラン)。
- ラン継続判定=`touched`+`group_alive`:ループ頭で `touched=false`、
  push 時に `touched=true`+`group_alive=true`、ループ底で
  `if !touched { group_alive=false }`(テキストを変更しないコマンド=
  移動・C-SPC・window 操作・Undo 自体はグループを切る)。
- no-op セーフティ:pre スナップショットを取り、**text が実変化した時だけ**
  push(BS@0、C-d@end、空 yank、C-t@0、空 insert-file は entry を残さない)。
- 履歴上限 200 entry(超過で最古を捨てる)。`C-x k` は該当 buffer の
  entry を `retain` で破棄。
- restore は `modified=true`(Undo 自体がファイルに対する変更)。
- バインド:`C-/` は xterm 系では 0x1f(=`C-_` と同 byte)。`plain_key(0x1f)`。
- msg:`Undo` / `No change to undo`(ステータス行に出るので e2e から見える)。

## Phase A: 実装(emur 本体)

- [x] A1 `buffer.rs`:`UndoState`+`snapshot()`+`restore()`(gap 末尾再構築、
      point/mark は min() で clamp)
- [x] A2 `keymap.rs`:`Cmd::Undo`+`plain_key` 0x1f+ピンテスト
- [x] A3 `main.rs`:`Ed.undo` スタック+`UndoEntry`+`push_entry`/`undo`、
      dispatch arm、変更ハンドラ 10 箇所にスナップショット取り込み
      (insert_file/BS/C-d/C-o/C-t/C-k/C-y/C-q/改行/自挿入)、
      `C-x k` で retain、ループ頭/底の touched/group_alive 配線
- [x] A4 ハンド検証(tmux):タイピング 1 ラン=1 Undo、kill run=1 Undo、
      空スタックで `No change to undo`、ステータス `Undo N/M (P%)`

## Phase B: 単体テスト

- [x] B1 `buffer.rs` tests 3 件(snapshot/restore 往復・clamp(防御的・
      hand-built state でピン・改修履歴は学習記録参照)・modified フラグ)
- [x] B2 `main.rs` tests 7 件(タイピング/kill グループ、移動でグループ切れ、
      point/mark+modified 復元、空スタック、上限 200、no-op 非 push)
- [x] B3 ルート `cargo test --locked` 52 件緑(既存 42+新規 10)

## Phase C: テスト統合

- [x] C1 e2e:新規シナリオ `s161`(タイピンググループ復元→kill run 復元→
      `No change to undo` の連鎖、ステータス行正規表現でピン)。
      挿入位置は 545 行目の後(release_smoke の sed 1,53/543,545 依存を
      保全)、runner 行に `t_s161`、クリーンアップに seed ファイル追加
- [x] C2 PBT P0:`Snap`/`Restore(idx)` op 追加(snapshot() 忠実度=buffer 側
      と ref 構築 state の一致、restore 後は全状態スウィープで検証)。
      MA-2 の pub API 全網羅を維持
- [x] C3 PBT P2:`P2Op::Undo`(`-H 1f`)+Sim が `push_entry`/`touched`/
      `group_alive`/pop+restore を完全ミラー(グループ合并まで一致させる)
- [x] C4 PBT P3/P4:Undo op 追加(liveness/pane-sane survival に織り込み)
- [x] C5 P1 は Ctrl バイトプール(0x00-0x1f)に 0x1f が自動内包済み(追加作業無し)
- [x] C6 `test-design/pbt-design/03-properties.md`:P0 op 記述・P2 表+
      Undo ミラー段落・P3/P4 ops 更新

## Phase D: ドキュメント

- [x] D1 README.md:機能一覧に Undo 段落、キー表に `C-/` 行
- [x] D2 `docs/design/keymap-dispatch-table.md`:素 map に 0x1f 行+
      `Cmd` 列挙スナップショットに `Undo`

## Phase E: フルテスト(直列厳守)

- [x] E1 ルートゲート:`RUSTFLAGS=-D warnings` build/clippy --all-targets 警告ゼロ+
      `cargo test --locked` 52 緑(コードゲートも同ゲートを再走し緑)
- [x] E2 pbt-test ゲート(clippy 警告ゼロ)+P0 512 cases 緑
- [x] E3 PBT フル(pbt-run.sh:P0+P1-P4、tmux 直列)=rc=0(21 passed)
- [x] E4 e2e.sh フル=90 件全 PASS(s161 含む、FAILED=0)
- [x] E5 release ビルド+release_smoke.sh=rc=0

## Phase F:  Reviews

- [x] F1 コードゲート(サブエージェント、プレコミット):APPROVE
      (Blocker 0/Major 0/Minor 3)
- [x] F2 readme-reviewer:Undo 記述は全件正確+他 drift 8 指摘
- [x] F3 织り込み済(Minor=台帳 11→10、README に全局スタック追記、
      drift 8 件=README/design/keybindings/docs/dev-env。织り込み内容は
      文档のみでコード不変=E1-E5 の再走ゲートはコードゲート側で
      全 cargo 再走済み・緑)

## Phase G: コミット

- [ ] G1 `git add -A && git commit -n`(ユーザ指示により -n)+push
- [ ] G2 CI 緑確認(gh run watch)
- [ ] G3 変更履歴の最終化(コミットハッシュ・バックフィル)

## 学習記録

- **マージ不具合の実測発見**:最初の設計は「ループ頭で group_alive を
  リセット」→毎回消えるのでマージが一度も発火せず、タイピング 3 文字が
  3 entry になった(tmux ハンド検証で検出)。ループ底判定(`touched` フラグ)
  に修正して解消。ハンド検証を先にやったことで単体テストを書く前に
  設計欠陥を捕まえられた。
- **xterm 系の `C-/` は 0x1f**(`C-_` と同 byte)。端末差の吸収を README/
  設計文書 D に明記。
- **restore の clamp は防御的**:スナップショットは「自分自身の text と
  整合した position」しか持ち得ない(毎反復の sync_in が clamp 済み)ため、
  `min()` は到達不能経路の防御。単体テストは hand-built の `UndoState`
  (point=9 > len=2)で clamp をピンし、この事実をコメントに記録。
- **グループ切れテストの语义**:テキストに触れない移動(C-a で point 1→0)は
  ループ底ルールでランを切る。EOL 系の「移動自体はするが text 不変」の
  コマンドはグループを切る、という仕様がテスト `motion_breaks_the_group` に
  固定した。最初のテスト案(C-a 後にタイピングで 1 列)は point が先頭に
  戻るため挿入位置が変わって失敗=実装でなくテスト側の誤りだった。
- **e2e の行番号結合**:release_smoke.sh は e2e.sh の `sed -n '1,53p'` と
  `sed -n '543,545p'` を抽出するため、新シナリオは 545 行目の「後ろ」に
  挿入し、543-545(cap_e/expect_e/expect_absent_e)と 1-53(harness)の
  内容を動かしてはいけない。runner ループ行は in-place 編集。
- **e2e の msg 可視化**:kill 等の msg を設定しないコマンドの後は前コマンドの
  msg がステータス行に残る。s161 はこれを利用して kill run 中に
  `Undo 1/1 (0%)`(前 msg=Undo、1/1=空 buffer)を assert している。
- **P2 Sim の完全ミラー義務**:P2 は編集ラウンドトリップなので、Undo は
  「エディタと同じグループ合并挙動」を Sim が再現しないと model/実機が
  発散する。touched/group_alive/merge 条件/tag を main.rs と 1:1 で写した。

## 変更履歴

- 2026-10-25: 台帳作成。実装一式(buffer.rs snapshot/restore、keymap 0x1f、
  main.rs undo スタック+10 ハンドラ+ループ配線)・単体テスト 10 件・
  e2e s161・PBT P0 Snap/Restore と P2/P3/P4 Undo・README/設計文書 D 更新を
  作業ツリーに集約。
- 2026-10-25: readme-reviewer 织り込み(Undo 記述は全件正確の判定。他 drift
  8 指摘=ステータス行「1 フレーム」主張の実態化/Wrote \<path\>/libc 依存/
  repo 一覧に pbt-test・docs・review/cargo test 説明+PBT entry/
  highlight 拡張 cc,hpp/keybindings.md に C-/ 行/dev-env OpenCode 版)。
  コードゲート APPROVE(Blocker/Major 0。Minor=本台帳の 11→10 を修正済み、
  undo スタックが editor 全局=設計どおり・README 明記、perf 注記は上限
  200 が文档化済みのため据え置き)。
