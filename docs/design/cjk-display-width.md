# Design Doc C: 全角(CJK)表示幅のセル基準化 (S-2 / T-7)

対象 TODO タスク: **S-2**(描画・カーソルの桁計算)、**T-7**(ミニバッファ候補
リストの幅・当たり判定)
出典: `review/03-screen-window.md` §S-2、`review/04-terminal.md` §T-7
受入: CJK テキストでレイアウト境界が崩れない/カーソルが実表示位置と一致する/
CJK 入力時の候補選択が当たる。単体テスト+新 e2e 3 シナリオ(s149〜s151)。
既存 e2e 80 全 PASS・`cargo test` 全通過・`cargo build`/`cargo clippy` 警告ゼロ・
性能ゲート(451KB debug 初回フレーム、fix #2 実測 0.122s)±軽微増。

## 1. 現状の計測事実(2026-10-24 ワークリングツリー)

- `draw_cell`(screen.rs:247-293):`chars.len() >= width` で**文字数**打ち切り、
  幅 `width` まで**文字数**で空白パディング。全角 1 文字=端末 2 セルにつき、
  CJK 行は枠のセル幅を食い破り、**境界列が全角セルと重なり**、縦境界 `|` が
  崩れ、CJK 末尾は右窓内容(固定列の print)に上書きされる。
- `point_pos`(screen.rs:74-88):`pcol += 1`(**文字**カウント)。カーソル配置
  (screen.rs:199-201)は `ccol = frames[i].col + pcol` なので、全角手前まで
  カーソルが表示より左にズレる。
- `list_width`(terminal.rs:283-294)は `chars_of(list[j]).len()`(**文字**長)合計。
  `cand_at`(terminal.rs:301-313)の候補スパンも**文字**単位。入力に CJK が
  含まれると、エコー上の実セル位置とスパンが食い違い Enter-on-candidate が
  空振りする(当たり判定が左にズレる)。
- `read_line_c`(terminal.rs:344 以降):`plen = chars_of(prompt).len()`(文字)、
  `echo_goto/echo_tail` と erase の `print!("\x1b[{};{}H\x1b[K", row, plen+cur+1)`
  が計 10+ 箇所(387/401/318/324 経由)。`cur` は入力の**文字添字**、候補表示時は
  `cur = out.len() + list_width(list)` で候補域の「仮想単位」も兼ねる。
- e2e に CJK 表示シナリオは**皆無**(grep 確認:s40=CRLF、s42=tty 復元)。

## 2. 設計

### 2.1 幅関数を 1 箇所に集約(文字→セル変換の単一の源)

`screen.rs` に公開関数を追加する(`terminal.rs` からは `super::screen::char_width`
で参照。同一クレート内のモジュール循環参照は Rust が許容 — screen.rs は既に
`super::terminal::flush_stdout` を使っており対称):

```rust
pub fn char_width(c: char) -> usize  // 0 (結合記号) / 1 (既定) / 2 (CJK 系)
```

- 幅 2(範囲マッチのみ、wcwidth 風の近似):**決定済み範囲**
  U+1100-11FF(ハングル子字)、U+2E80-33FF(CJK 部首〜単位、かな・
  CJK 記号・全角記号を内包)、U+3400-4DBF(拡張A)、U+4E00-9FFF(統合)、
  U+A960-A97F、U+AC00-D7A3(ハングル音節)、U+F900-FAFF(互換)、
  U+FE10-FE19、U+FE30-FE6F(縦書き)、U+FF00-FF60(全角 Forms)、
  U+FFE0-FFE6(全角互換)、U+1F300-1F64F(絵文字)、U+20000-2FFFD、
  U+30000-3FFFD(拡張B以降)。
- 幅 0:U+0300-036F(結合発音記号)。
- **既定 1**。制御文字(<U+0020)は 1 — `draw_cell` は制御を `' '` として
  描くので整合する。
- **近似の公認**(§4 リスクに明記):U+2E80-33FF は実 wcwidth で 1 の穴を
  含む(U+3099-309F の一部、U+3040 欠番等)。範囲表を単純化し、日本語
  ファイル名/テキストで使う全集合を網羅する方を優先する。
  絵文字は U+1F300-1F64F のみ幅 2(U+1F680-1F6FF 輸送系・U+1F900-1F9FF
  補助絵文字は既定 1 のまま=**未カバー**、表の拡張は行わない)。

### 2.2 screen.rs — 描画とカーソルをセル基準に

- `draw_cell`:累積**セル**で打切る。`cells + char_width(c) > width` なら
  その文字を描かず打ち切り(全角の末尾桁溢れを描かない=右境界 `|` を
  壊さない)。後続の右境界フレームは既定通りその列を上書きする。
  パディングはセル単位(`while cells < width { push ' '; cells += 1 }`)。
  `chars`/`stys` は文字エントリのまま(スタイル対応は文字index、変更不要)。
- `point_pos`:`pcol += char_width(*c)`。`\n` で行送付・列リセットは現状どおり。
  戻り値の列は**セル**。
- カーソル配置(screen.rs:199-201):`pcol`(セル)を現状どおり `maxc = width-1`
  でクランプ。**既知の端例**:右端で全角の第 2 セルにカーソルが着地しうる
  (端末は第 1 セルへスナップするのが通例)。許容・README に注記。
- ステータス行:名称が CJK パスでも `draw_cell` のセル打切りに従う
  (`1/3 (0%)` サフィックスは末尾なので長い名前で切れる挙動は現状の ASCII と同じ)。
- 呼び出し側(`line_styles` 構築、`offs`)は**文字**基準のまま — 打切りが
  スタイリング対応に与える影響なし(スタイルは描いた文字数ぶんだけ使用)。

### 2.3 terminal.rs — 候補リストとエコーのセル基準化(T-7)

単位変換は**表示バウンダリだけ**に施し、編集状態 `cur`(入力の文字添字)と
`out: Vec<char>` の単位は変更しない(移動キーの意味論は現状維持)。

- `pub fn cells_of(s: &[char]) -> usize`(char_width の合計)を terminal.rs に追加。
- `list_width`:各候補の**セル**合計(+区切り 2+1)。ASCII の既存単体テストは
  数値不変(ASCII は 1 文字=1 セル)。
- `cand_at(list, inp_cells, caret_cells)`:スパンを**セル**で構築する
  (`s = inp_cells + 2`、`l = cells_of(candidate)`)。呼び出しは
  `cand_at(list.clone(), cells_of(&out), caret_col(0, &out, cur))` —
  **caret_cells は prompt 先頭相対(plen_c を含まない)**であることを明文化する。
- `echo_goto(row, plen, cur)`:引数意味を「列オフセット(セル)」と明文化し、
  呼び出しは `plen → plen_c`、`cur → cells_of(&out[..cur])`(候補域のキャレットは
  `caret_col(0, …)` の値を渡す。関数内算式 `plen + cur + 1` は不変)。
- `echo_tail(row, plen, cur, &out)`:呼び出し側 `cur` は**文字添字**のまま
  (out[cur..] を印刷するため)。**別名演算を明記**:_combine =
  `(plen_c + cells_of(&out[..cur])) - cur` を `plen` 引数に渡すと
  関数内の `plen + cur + 1` が意図した列(セル)に一致する —
  これにより単位の違いが関数内に漏れない。
- `read_line_c`:
  - `plen = chars_of(prompt).len()` → `plen_c = cells_of(prompt chars)`。
  - **統一ヘルパ**(print! 10+ 箇所の置換先):
    ```rust
    // caret_col: caret の列(セル)。cur<=out.len() は入力のカーソル位置(文字添字)、
    // cur>out.len() は候補域の「入力終端からのセルオフセット」(現状の仮想単位が
    // そのままセル意味になる:list_width がセル化するため)。
    fn caret_col(plen_c: usize, out: &[char], cur: usize) -> usize {
        if cur <= out.len() { plen_c + cells_of(&out[..cur]) }
        else { plen_c + cells_of(out) + (cur - out.len()) }
    }
    ```
  - 全 print! の `plen + cur + 1` → `caret_col(plen_c, out, cur) + 1`
    (echo_goto/echo_tail の呼び出しも `plen, cur` を渡していた位置を
    `caret_col(...) - 1 + 1` と同等に、関数側は `plen+cur` の引数意味を
    「列オフセット(セル)」と明文化 — 関数内算式は不変)。
  - `tl = out.len() + list_width(list)`:右辺第 2 項はセルになる。`tl` は
    「入力終端からのセルオフセット」を文字添字空間に載せた仮想単位のまま
    — `caret_col` が同じ解釈で変換するため整合(§1 の現状の二重意味が
    明示化される)。`cur = out.len() + list_width(...)` も同様。
- Tab 補完の「共通前置+候補エコー」自体は `print!` で自然に進む(端末が
  全角で 2 セル進む)。**位置指定を伴う**上記の場所だけが変換点。

### 2.4 文書

README の「Limitations/Notes」に:全角=2 セルで描画・カーソル配置する旨と
近似の注記(§4 の穴、右端カーソルのスナップ)を 1 段落。

## 3. テストとゲート

- 単体(screen.rs):`char_width` マトリクス(ASCII=1、`あ`=2、U+4E00=2、
  U+FF21=2、結合 U+0301=0、`\t`=1)。`point_pos` の CJK 列(`"あa"` で
  point 2 → 列 3)。
- 単体(terminal.rs):`cells_of`;`list_width` の CJK(`["あい"]` → 2+4+1=7);
  `cand_at` の CJK ケース(入力 CJK 込みのキャレットセル→候補1 ヒット)。
  既存 ASCII ケースは全数維持(数値同一)。
- e2e(**新規 3**、`tmux display-message '#{cursor_x}'` ヘルパ `curx()` を追加):
  - **s149 カーソルセル**:ファイル `あいう\n`、起動時カーソル列 0、
    `C-f`→`curx`=2、`C-f`→4、`C-b`→2。旧ビルドは 1/2/1 になり FAIL(分離)。
  - **s150 境界整合**:`あ`×30 の行をファイル起動で開き `C-x 3`(左窓幅 39)。
    行头が「`あ`×19 + 空白 + `|`」を `expect "$(printf 'あ%.0s' {1..19}) \|"`
    で確認(19 文字=38 セル+パディング 1)。旧ビルドは 20 番目の `あ` が
    境界列と重なり(tmux 正規化後も 19 文字目の後に `あ` が立っている)、
    パターンは `|` 直前の空白を要求するので FAIL。`{1..19}` 展開は bash 前提(e2e は bash)。
  - **s151 候補選択(T-7 分離)**:`/tmp/emur-e2e-あa.txt` と `/tmp/emur-e2e-あi.txt`
    を用意。`C-x C-f` で `/tmp/emur-e2e-あ` を入力し Tab。**候補エコーは
    file_candidates の仕様でフルパス**。単位計算(prompt 先頭相対・セル):
    入力 out = 15 字 = 16 セル。候補 1/2 は各 21 セル。
    `lw_cells = 2 + (21+1) + (21+1) = 46`。Tab 直後のキャレット
    `cur = 15 + 46 = 61`(仮想単位)。候補 1 スパン `[18, 39]`・候補 2 `[40, 61]`
    (`inp_c + 2 = 18` から)。狙いは候補 1 の先頭セル 18 = `cur = 15 + 2 = 17` →
    **`C-b` ×44**。Enter で候補 1 が選ばれ
    `expect_lastline 'Buf /tmp/emur-e2e-あa.txt 1/1 (0%)'`。
    - 旧ビルド(文字単位)は `lw_chars = 2 + 21 + 21 = 44`、`cur0 = 59`、
      同じ 44 打で `cur = 15 = out.len()` → キャレットは入力終端 →
      cand_at 空振り → 入力そのまま確定で `Failed to open`(アサート FAIL)。分離成立。
    - 打数 16 では新ビルドでも cur=45 → キャレットセル 46 ∈ 候補 2 →
      `…あi.txt` が選ばれ FAIL(打数が意味を持つことの記録)。
- フル e2e(80 シナリオ)+ release スモーク(6)。
- 性能:`time (./target/debug/emur /tmp/big_perf.rs </dev/null)` — 1 文字あたり
  char_width の範囲比較 ~26 回が加算される。基準 0.122s(10 倍の余裕)に対し
  変化計測・記録(1s ゲート維持)。
- `cargo build`/`cargo test`/`cargo clippy` 警告ゼロ(fix #6 以降の固定ゲート)。

## 4. リスクと許容

- **幅表の近似**:U+2E80-33FF 内の実幅 1 の穴(かな結合 U+3099-309F など)は
  2 として扱う。影響はそれらを含む稀なテキストのレイアウトのみ。表の単純化を
  優先し、README に注記。
- **右端カーソルの第 2 セル着地**:クランプが全角 2 文字目に触れる端例は
  端末のスナップに委ねる(§2.2)。編集ロジック(文字 index)とは無関係。
- `cur` の二重意味(入力内=文字添字/候補域=セルオフセット)は現状の構造を
  受け継ぐ。`caret_col` 1 関数に変換を閉じ込めることで混用箇所を作らない。
  (M-11 のキーテーブル化と同様の構造改善はスコープ外)
- 結合記号(幅 0)を跨ぐカーソル位置は第 1 文字の直後扱い(列は据わり)。
  日本語入力では実質発生しない。
- `split_lines`/gap buffer/highlight は**文字** index のまま(不変)。
  打切りは表示のみ。検索・状態行の % も文字基準(既存挙動維持)。
- e2e の grep はバイト比較前提(`あ` リテラル、 locale 非依存)。正規表現の
  `.`(文字/バイト)に依存するアサートは書かない(s150 はリテラル連結)。
