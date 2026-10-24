# レビュー: src/buffer.rs (gap buffer)

## 高

### B-1. gap が事実上常に長さ 0 で、挿入が O(n) になる
設計文書 (`design/architecture.md:9`) は「gap buffer によりカーソル付近の
挿入/削除は償却 O(1)」と謳うが、実装はそうなっていない。

- `Buffer::new` は gap を確保しない: `chars` を詰め、`gap_start = gap_end = 0`
  (`buffer.rs:20-31`)。
- `insert_char` は gap が空のとき `Vec::insert` で挿入する (`buffer.rs:62-69`)。
  gap が空のままなので、挿入のたびに配列末尾まで memmove が走る = 1文字
  O(n)。大きなファイルのロード(`main.rs:167` の insert-file も 1文字ずつ
  `insert_char`)や入力は O(n²) になる。
- gap が伸びるのは `delete_forward`/`delete_backward` の時だけ (`buffer.rs:77-95`)。

修正案: `new()` で `chars.resize(n + GAP, '\0')` のように gap を預け
(例: GAP=4096〜8192)、`insert_char` の gap 消費経路 (`buffer.rs:62-64`) を
通す。リサイズ戦略(消費時に gap を補充)も入れる。
テスト: 1文字挿入ごとの `chars.len()` 変化を追う回帰テストがあると良い。

## 中

### B-2. find_forward / find_backward が毎回 buffer 全体の text() を生成する
`buffer.rs:283` と `buffer.rs:313` で `self.text()` を呼び、O(n) の文字列
コピー+アロケーションを毎回行っている。増分検索 `C-s` は 1 キーストローク
ごとに `find_forward` を呼ぶ (`main.rs:241,262-263`) ので、1MB バッファで
打鍵ごとに約 1 回分の全文コピー+総当り検索 (`O(n*m)` の素朴マッチ、
`buffer.rs:293-307`) が走る。

修正案: `char_at()` は配列添字 O(1) なので、`text()` を作らず
`self.char_at(i + k)` で直接比較する。戻り値も `Vec<usize>` ではなく
`Option<usize>` にできる(次項)。

### B-3. kill_to_end_of_line: mark が同じ行の後方(点より前)で C-k が無音の no-op
`buffer.rs:235-245`: mark が現行内で点より前にある場合 `hi = mark < point`
となり `stop = hi`、`n = 0` で何も削除せず空文字列を返す。Emacs はこの状況
(領域が後方)では行末まで kill する。README は「mark が前方にあるとき停まる」
としか書しておらず、この no-op は文書化されていない。
修正案: 後方 mark は領域なし扱い(行末まで)に統一するか、挙動を README に書く。

## 低

### B-4. 戻り値 `Vec<usize>` の API がイディオムに合わない
`find_forward` / `find_backward` は高々 1 要素の `Vec<usize>` を返す
(`buffer.rs:281,311`)。`Option<usize>` が自然で、呼び出し側の
`hit.len() > 0` / `hit[0]` パターン (`main.rs:241-244,286-288`) も消える。
同様に `find_buffer` (`main.rs:21`) も `Option<usize>` にできる。

### B-5. transpose_chars が点=0で先頭2文字を入れ替える
`buffer.rs:226-229`: 点=0 なら Emacs の C-t は no-op だが、本実装は先頭2文字を
swap して点を 1 にする。挙動の divergence(Emacs 準拠を標榜するなら)。

### B-6. insert_char / insert_newline_after / delete_backward が mark を点に追従させる
`buffer.rs:71,93,205`。編集で領域を無効化するのは transient-mark 的な設計判断と
して妥当だが、README は「C-a/C-e/C-b/C-f/C-k が領域端で停まる」としか書かず、
「文字を打つと領域は消える(C-@ で張り直しが必要)」が未文書。ユーザーの驚きの元。

### B-7. insert_newline_after の gap 消費経路に gap_start += 1 がない? — 検証結果: 問題なし
`buffer.rs:196-207`: gap 非空時は `chars[gap_start] = '\n'; gap_start += 1`、
空時は `Vec::insert` の後 `gap_end += 1`(`gap_start` は `Vec::insert` のシフトに
任せず明示加算なし)。追跡した範囲では論理位置の崩れは無し(点は動かさない =
README 通り)。ただし `insert_char` との差分(点は動かす/動かさない)がコメントに
のみ依存。テスト `buffer.rs:424-425` が担保していてgood。

### B-8. move_gap 自体は正しい
`buffer.rs:46-58` の左方/右方シフトは添字追跡の上で正しく、`to_index` との
整合も取れている(gap が [pos, pos+gapsize) に着く)。コメントがほぼ無いので
不変条件(gap は常に [gap_start, gap_end)、len() = chars.len() - gapsize)を
1行ずつコメント化しておく価値がある。

### B-9. 単体テストの範囲
既存テスト (`buffer.rs:355-429`) は movement/kill/transpose/検索の一部を
カバーするが、未検証: `move_gap` の単体(大きな gap を跨ぐ移動)、
`find_backward` の折返し/終端クランプ (`buffer.rs:323-326`)、
`next_line` の最終行クランプ (`buffer.rs:169-174`)、
`transpose_chars` の境界(点=0/末尾)、空バッファでの `delete_forward`。
特に find 系とクランプ系は今回見つけた挙動分岐(B-3)を含むので追加価値が高い。

### B-10. API スタイル
`self: &Buffer` 形式 (`buffer.rs:34` 他) は `&self` で十分。
`new(name: String, ...)` は `impl Into<String>`/`&str` を取る方が呼び出し側の
`.to_string()` 乱発(main.rs 全体)を防げる。gap buffer 本体のロジックは
分かりやすく、コメント(1-3行目)も的確。
