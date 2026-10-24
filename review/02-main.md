# レビュー: src/main.rs (起動・キーディスパッチ)

## 高

### M-1. go-to-line (C-l) で u32 溢れ panic → ターミナル生モード漏れ (実測済み)
`main.rs:359-367`: `want_ln = want_ln * 10 + digit` が u32 を溢れる。
実測: `printf '\01499999999999999\n' | emur` (debug) で
`panicked at src/main.rs:363: attempt to multiply with overflow`、exit 101。
panic すると `main` の末尾 `terminal::restore_mode` (`main.rs:396`) まで
到達せず、**端末は raw mode が復元されず ECANON/ECHO が死んだまま**になる。
release ビルドでは溢れは wrap するため、デバッグビルド(e2e が使う)と本番で
挙動が違う点も問題。

修正案: `q.parse::<usize>()` を使い、`u64` 相当+上限(行数)で検証する。
加えて panic 時にも `restore_mode` する仕組みは 04-terminal.md §3 を参照。

### M-2. EOF がミニバッファ内で無限ループする(minibuffer 経路の終了確認が機能しない)
`confirm_exit` (`main.rs:36-48`) は `terminal::read_line` を呼ぶが、stdin が
EOF だと `read_line_c` は無限ループする(`read_byte()` が EOF で 0 を返し、
それが NUL 文字として self-insert 扱いされ続ける。詳細は 04-terminal.md §1)。
実測: `printf 'x\003' | emur`(汚れたバッファで C-c、直後に stdin EOF)→
3秒で 35.9MB 出力、プロセスは生存。コメント (`main.rs:34-35`) と README の
「EOF は無条件に終了」は、メインループ (`main.rs:70-73`) にしか成立しない。

修正案: EOF=キャンセル(="" を返す)を `read_line_c` 側で実装し、
`confirm_exit` はキャンセル時に終了しない(または明示的に "Cancelled")。
コメントと README の記述を実装に合わせる。

### M-3. 検索(C-s)中の矢印キーがバッファにゴミ文字を挿入する
`main.rs:235`: 増分検索ループは ESC(0x1b) を「Enter 相当の accept」で抜ける。
矢印キー `ESC [ A` の ESC で accept した後、残りの `[` と `A` が次のメイン
ループ反復で printable self-insert に落ち (`main.rs:385`)、**バッファに `[A`
が挿入される**。README/キーバインド文書は「ESC ends like Enter」としか書かず
矢印の扱いに言及なし。
修正案: 検索ループ内で ESC を見たら即座に 1 バイト先読みし `[` なら
第3バイトを消費して矢印として扱うか、検索中は ESC を明示的に flush する。

## 中

### M-4. EOF が無確認で全未保存バッファを破棄する
`main.rs:70-73`: メインループは EOF で `break` するだけ。dirty バッファが
あっても確認しない(tmux セッションのクローズ、パイプ入力などで実際に起きる)。
README:146-148 の「EOF exits unconditionally」は事実だが、**未保存データの
無警告喪失**としては Emacs の挙動(EOF は quit に等しい=確認あり)から乖離。
せめて `confirm_exit` を経るか、dirty 時にメッセージを残す設計を推奨。

### M-5. C-x k が修正済みバッファを無確認で破棄する
`main.rs:172-177`: 名前の通り中身を空バッファに差し替えるだけで、modified
でも確認しない。Emacs は "Buffer modified; kill anyway?"。C-x k は undo も
無い(README により明示)ため、ワンキーでデータが消える。

### M-6. C-x C-f は同じファイルを毎回新規バッファ化する
`main.rs:120-123`: ファイルを開くたびに `bufs.push` するので、同じファイルを
2回開くと同名バッファが2つでき、C-x b のリストも重複する。
`find_buffer` (`main.rs:21-30`) は名前検索で使われているのに、open 時に
既存バッファを再利用しないのは一貫性がない。Emacs は同名ファイルの既存
バッファを再表示する。

### M-7. 未バインドの C-x 後続キーが無言で握り潰される
`main.rs:82-215`: C-x プレフィックスの if 連鎖に else がなく、`C-x z` などは
何も起きず `prev_cmd` も更新されない。Emacs は "Quit" と echo する。
C-x プレフィックス全体の default 節として "Cancelled"/bell を出すべき。

### M-8. C-x C-w は書き出し失敗後も filename を設定したままにする
`main.rs:156-158`: `bufs[bi].filename = Some(path)` を書き出し前に設定するため、
失敗すると「保存先が設定されたのに modified」状態が残る。Emacs は成功時のみ
設定。C-x C-s で同じエラーが毎回出続ける。

### M-9. read_file / read_line_files はエラーを静黙する
`main.rs:7-12`: 存在しないファイル・ディレクトリ・権限エラーが全部
空バッファ/空文字列になる。README:79 は「missing files open empty」を
仕様として明記しているが、`insert-file`/`write-file` 側(`main.rs:165-171`)も
同じで、失敗時のステータスメッセージ(Emacs: "File does not exist")が無い。
`read_line_files` の Tab 補完はカレントディレクトリのみ(`terminal.rs:191`)で、
`~` の展開も無い。

### M-10. go-to-line の O(n) ループ
`main.rs:371-376`: 1行ずつ `next_line` を繰り返す。行数×行長が大きい
(例: 10万行)ファイルでは C-l 1回で体感待ち。`line_start` 走査か
行オフセットキャッシュで O(n) 1回にできる。

## 低

### M-11. キーディスパッチが平坦な if-else の巨大連鎖
`main.rs:75-389`: 約300行の 1 関数。`design/architecture.md:50-55` は
「keymap tree で解決」と書く(実装と不一致、07-docs-consistency.md 参照)。
`(byte, prefix) -> Command` のテーブル化が保守性・ドキュメント整合の両方で有効。

### M-12. prev_cmd は直前のトップレベルバイトのみ
`main.rs:394`: C-k 連結は「直前が C-k」の単一バイト記憶で実現。C-s の最中に
複数バイト消費しても prev_cmd は C-s のまま、など厳密には Emacs の
last-command とは違う。実用上は README の記述(「他のコマンドで連鎖解除」)と
整合しており許容範囲。C-y 直後の C-y に意味を持たせる拡張も不可。

### M-13. 起動直後の同期の不変条件が暗黙的
`main.rs:66-69`: ループ先頭で `bufs[bi].point/mark` を leaf に写す
(leaf 側の point が正である)という同期契約がある。C-x o / C-x 2 / C-x C-f 側で
逆方向の書写し (`main.rs:100-101,134-136`) を手動でやっており、新しいキーを
追加するたびに「leaf に書くのか bufs に書くのか」を間違えると症状が subtil な
バグになる。`sync_point()` のようなヘルパーに集約を推奨。

### M-14. .to_string() / clone の乱用(スタイル)
`main.rs:56,121,142,157` 等、所有済みの String を `.to_string()` で複製する
箇所が多数。`fn read_file(path: &str)` 等にすれば消える。find_buffer の
`Vec<usize>` 戻り値 (`main.rs:21`) も B-4 と同じ話。

### M-15. 検証できて良好な点
- NUL(Ctrl-space)と EOF の区別を `read_byte_eof` で正しく扱っている
  (`main.rs:70-74`, `terminal.rs:53-63`)。
- 終了確認の yes/y のみの終了、それ以外キャンセルは Emacs 準拠で明快。
- C-s の wrap 検索、bell による入力の dropout、C-g で点復元は
  README の記述どおり実装されている (`main.rs:222-282`)。
