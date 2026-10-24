# レビュー: src/highlight.rs (シンタックスハイライト)

## 高

### H-1. heq_at が全文 Vec<char> を毎回 clone する → styles が O(n²) (実測裏付け)
`highlight.rs:15-28`: `heq_at(cs: Vec<char>, …)` は呼び出しごとに `cs.clone()`
を引数へ渡しており、`styles` 本体の各ループで多発的に呼ばれる
(`highlight.rs:183, 193, 196, 198, 212, 215, 70-90`)。1文字スキャンごとに
全文(=n char)コピーなので `styles` 全体が O(n²)。これを `screen.rs:142` が
**毎フレーム全バッファに対して**呼ぶ。実測: 451KB の .rs 起動時 1 フレーム
12.9 秒 (00-overview.md)。

修正案:
- `heq_at(cs: &[char], i, w: &str)`、`hchars_of(s: &str) -> Vec<char>` 等に
  シグネチャを変える(所有権を渡さない)。
- `styles` が `Vec<usize>` ではなく `&[char]` を受けるようにする
  (`screen.rs:142` は `buf.text()` の String を作らず、可視行範囲の char
  スライス+オフセットで style を計算する。03-screen-window.md §S-1 と連動)。

## 中

### H-2. Rust の_lifetime を文字列として誤色する
`highlight.rs:229`: `lang == 1`(c-family)でも `'` を文字列開始とみなす。
Rust のライフタイム注釈 `'a`, `'de`, `match … 'label:` などが
閉じクォート(=改行手前まで)まで magenta の文字列になる。
`lang == 1` を C 系と Rust で分割するか、Rust では `'` を無視する、
「直前が空白/括弧/`,`/`;` で、同一行内に閉じ `'` があるときのみ文字列」等の
ヒューリスティックで抑える。

### H-3. detect が拡張子大文字小文字を区別しない/マルチドットは末端のみ
`highlight.rs:53-64`: 最後の `.` を採用(正: `a.b.rs` → rs)。
`FILE.RS` / `x.MD` は不一致で無装飾になる(Emacs は大小無視)。
`heq_at` の比較前に lower-case 化するのが手軽。

## 低

### H-4. md_styles の細部
- インラインコードが閉じられず改行で終わるとき、バッククォートから行末までを
  文字列装飾する (`highlight.rs:144-158`)。Markdown 的には閉じ未結成のまま
  行末まで装飾は許容範囲だが、次の行を巻き込まない挙動を確認するテストが欲しい。
- リストマーカー (`-`/`*`/`+` + 空白) 判定がインデント付きで動く
  (`highlight.rs:127-143`) — good。
- 見出しは行頭 `#` のみ (`highlight.rs:133`)。README:151 の "markdown
  headings" と整合。

### H-5. 数値スキャンは概ね良好
`highlight.rs:268-286`: `0x1F`、`1.2` を拾い、`1..2` は 1 トークンになる
(許容)。16 進 a-f を `.` と同列で拾うので、`0xDEADbeef` も OK。
`2hex` のように数字直後の英字も 16 進の一部として装飾される(軽微な過装飾)。

### H-6. HTML タグ内の文字列は装飾上書き
`highlight.rs:252-267`: `<a href="x">` を tag(5)で塗り、属性の `"x"` は
文字列(2)にならない。見た目の問題のみ。

### H-7. スタイルコードがマジックナンバー
`0..5` の素の usize がスクリーン側 (`screen.rs:203-220` の style_esc) と
結合している。`enum Style { Plain, Keyword, String, Comment, Number, Marker }`
にすると style_esc の match が網羅チェックされる。

### H-8. 単体テストが皆無
`detect` の拡張子マトリクス、コメント/文字列/数値/キーワード/md/html の各
スタイル配列をテストで固定すべき(リファクタの安全網にもなる)。
e2e 側には色を assert する手段が tmux capture-pane では限られるので、
純粋な styles() 入力→出力テストが適している。

### H-9. 言語セットの設計メモ
- lang 2 (py/sh/rb/yaml/toml) を 1 クラスにしているため、`"""` は sh/yaml でも
  文字列扱いになる (`highlight.rs:212-228`)。yaml の `"""` は稀で実害は小さい。
- kw リスト (`highlight.rs:95-117`) は lang で共有で十分に見えるが、
  py の `echo`(shell 由来)は py でキーワード表示になる。言語分割で整理できる。
- 全言語の doc-comment(1-5行目)が正確で読みやすい。good。
