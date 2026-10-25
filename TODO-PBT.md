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

- [ ] A1 `test-design/pbt-design/` に設計文書 7 点を書く
      (README / 01 overview / 02 package / 03 properties / 04 tmux harness /
      05 CI / found-bugs 雛形)
- [ ] A2 設計ゲート(サブエージェント)を HEAD 実装と照合して実行
- [ ] A3 ゲート指摘(Blocker/Major/Minor)を文書に織り込み、台帳に記録

## Phase B: テスト基盤

- [ ] B1 `pbt-test/` 雛形(package、`[workspace] default-members=["."]`、
      `proptest` 追加、`src/lib.rs` の `#[path]` include)で
      `cargo build -p pbt-test` が通る
- [ ] B2 harness(`tests/support`):セッション起動/終了(Drop ガード)、
      起動時 pbt-* 掃讨、`send_hex`/`send_text`、`capture`/`poll_capture`、
      EMUR_BIN 解決、`PBT_CASES` 読込
- [ ] B3 ゲート:pbt-test build/clippy 警告ゼロ・`#[allow]` 無し・
      ルート `cargo build/test -p emur` は従来どおり(影響ゼロ)

## Phase C: P0(gap buffer 差分テスト、tmux 無し)

- [ ] C1 naive モデル実装(op 全メソッド+point/mark+kill 返り値)
- [ ] C2 P0 実装+512 ケース通過
- [ ] C3 プリコミット・ゲート → コミット/ push

## Phase D: P1-P4(tmux 統合ファジング)

- [ ] D1 P1 raw byte fuzz(クラッシュフリー+画面健全性)
- [ ] D2 P2 編集ラウンドトリップ(モデル replica 一致+ステータ行+保存)
- [ ] D3 P3 window 構造ランダムウォーク(split/collapse 境界)
- [ ] D4 P4 resize ファジング
- [ ] D5 プリコミット・ゲート → コミット/ push
- [ ] D6 `test-design/pbt-run.sh` 実装(直列実行の単一分入口)

## Phase E: 本走・トリアージ・不具合対応

- [ ] E1 全プロパティ連続 3 回フル走(ローカル、直列)
- [ ] E2 失敗は全て found-bugs.md に再現手順付きで記録(モデル側不具合も
      「NOT-A-BUG」判定根拠を明記)
- [ ] E3 emur 実バグは 1 件=1 コミットで修正(コードゲート+フル e2e+CI。
      修正は当台帳に変更履歴として記録、既台帳 TODO-261024 の対象外)
- [ ] E4 修正後は該当プロパティ回帰+ regressions 再走

## Phase F: CI 統合と締め

- [ ] F1 `ci.yml` に PBT ステップ追加(05 の並び。run/ watch で緑確認)
- [ ] F2 学習記録の追記(台帳末尾)
- [ ] F3 変更履歴の最終化(コミットハッシュ・バックフィル済み)

## 学習記録

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

- 2026-10-25: TODO-PBT 作成+設計文書 7 点(このコミット)。設計ゲートは次。
