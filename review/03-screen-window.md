# レビュー: src/screen.rs / src/window.rs (描画・レイアウト)

## 高

### S-1. 1フレームあたり全バッファを複数回走査+全面再描画し、ハイライトと
相まって-large file で実用限界 (実測済み)
`screen::draw_all` (`screen.rs:70-201`) は毎フレーム:
- `buf.text()` を 3 回(window ループ内 `screen.rs:93,140,142`、point_pos 経由 `93,166`)+ 全文 `split_lines` (140)
- `styles(buf.text(), lang)` で全バッファのハイライト (142) — 呼び出し側は
  可視範囲しか使わない
- 全行を `draw_cell` で無条件 print (`145-164`)

`highlight::styles` が内部 clone で O(n²)(05-highlight.md §H-1 参照)なので、
451KB の .rs で **1フレーム描画に実測 12.9 秒**。1KB でも全走査が毎キーストローク
走る。

修正案(優先順):
1. `styles` にスライス参照を渡す(05 参照)。
2. `text()` を 1 回だけ生成し `split_lines` と point_pos をそこから導く。
3. ハイライトは表示ウィンドウの可視行範囲(off..off+len)だけ計算する。
4. 差分描画(dirty 行のみ print、または curses 側の double buffer)を導入して
   ちらつきと出力バイト数を減らす。

## 中

### S-2. 全角(CJK)文字幅を 1 として扱う
`split_lines`/`draw_cell`/カーソル位置 (`screen.rs:140-176,222-241`) は
すべて「文字数」で桁を計算する。ターミナルは全角を 2 セル使うため、日本語
テキストでは:
- カーソルが実際の表示位置より左にずれる (`screen.rs:175-176`)
- 右端トランケートがセル数でなく文字数になる (`screen.rs:228-230`)
- 縦境界 `|` がウィンドウ幅と合わさり崩れる

エディタは UTF-8 入力を受け付ける(README:49)ので、日本語を使うユーザーは
確実に踏む。wcwidth 相当のテーブル(最低限: CJK 統一表意文字/かな/全角記号を
幅 2)を 1 箇所に集約し、`draw_cell` と point_pos/カーソル算出をセル基準に統一する。

### S-3. window.rs の split/collapse でスロットが無限増殖する
`window.rs:120-156`: 分割も `collapse` も新しい leaf/slot/node を push するだけで、
古い leaf・kind/which スロット・node を回收しない。split → C-x 1 → split を
繰り返すと `leaves`/`kind`/`which`/`nodes` が単調増加する(リーク)。
C-x o の `flatten` は root から辿るだけでコストは木の実数なので、`leaves` の
ゴミはメモリと leaf index の肥大化のみ。長時間セッションでは
`collapse` 時に root 配下の部分木を compact する、を推奨。

### S-4. draw_all は curses の size を毎フレーム取っている(良)が、
curses crate の使われ方が「size 取得専用」
`screen.rs:12` は `curses::canvas::get_size()` のためだけ。README:47 は
size 検出を "termsize" と書いているが直接依存は curses(termsize は推移依存)。
README/コメントの名称を合わせるのが正確(07-docs-consistency.md §D-7)。
コメント自体 (`screen.rs:4-6`) は実態(手書き ANSI)と一致していて good。

## 低

### S-5. 境界 Frame の `leaf` フィールドがダミー値
`window.rs:74,87`: `Frame { leaf: 0, kind: 1|2, ... }` と leaf を埋める意味が
無い。`kind` を enum(Window/BoundaryH/BoundaryV)にすれば無効フィールドが消え、
`screen.rs:87,116,128` のマジックナンバー比較も消える。

### S-6. scroll 再計算ロジックは README 通りだがコメントが無い
`screen.rs:95-102`: point が可視範囲を出た分だけスクロール(1行ずつ追従)。
README:134-136 の記述と一致。この「scroll は毎フレーム point から決まる
(stateful なページ送りではない)」不変条件をコメント化すべき。

### S-7. カーソルの status line  clamp はrows基準
`screen.rs:192-195`: `crow > rows - 2` で status 行を避ける。分割時は
下側ウィンドウの最下行 crow と rows-2 が衝突しうる(境界行 `-` の直上)。
実害はカーソルが1行上に見えるだけで、入力位置は正しく保たれる。要コメント。

### S-8. find_slot の「見つからず」センチネルが `kind.len()`
`window.rs:103-118`: 返り値がスロット数と同じ値=not found。`Option<usize>` に
できる。`collapse` (`window.rs:149-156`) は find_slot 失敗時に無言で
root を新 leaf に向ける(通常は active leaf なので成功するが、
active が不整合なときに根こそぎ壊れる)。

### S-9. vertical の命名が逆になりやすい
`window.rs:69` の `vertical: true` = 縦積み(下に分割)、`main.rs:93-98` の
C-x 3(右分割)は `false`。Emacs の "split-window-right" は水平方向。
`stacked`/`side_by_side` 等のリネームかコメントが欲しい。

### S-10. point_pos が active leaf について1フレームで2回走る
`screen.rs:93,166`。結果を1回分にできる(軽微)。

### S-11. status line の表示は仕様通り
`Buf <name>[ *] <line>/<total> (<pct>%)` を status 行に描画し、メッセージが
1 フレームだけ status を置換する (`screen.rs:182-190`) — README:137-142 と一致。
`st_pct` は文字数基準 (`screen.rs:172`)。pct 0 で status に 0/1 (0%) 表示、
空ファイルで division by zero なし (`len>0` ガード、good)。

### S-12. レイアウト境界行/列のフォールバックは丁寧
`window.rs:70-99`: 3 行/列未満では境界を諦めて素分割、2 未満は同一領域に
重なる(デジェネレートを許容)。デジェネレートの 2 子同一ジオメトリ
(`window.rs:95-98`) は両子が同じセルに重なるため見た目が破綻するが、
80x24 で split を無限に繰り返さない限り到達しない。コメント推奨。
