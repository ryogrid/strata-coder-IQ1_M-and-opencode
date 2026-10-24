# レビュー: src/terminal.rs (tty・入力・ミニバッファ)

## 高

### T-1. read_line_c が EOF で無限ループする(全面修復対象・実測済み)
`terminal.rs:303-455` の入力ループは `read_byte()` (`terminal.rs:66-68`) を
使うが、`read_byte` は EOF で **0 を返す**。0 は Enter(0x0D/0x0A)でも
C-g(0x07)でもなく、「printable char」分岐 (`terminal.rs:441-454`) に落ち、
`read_char_from(0)` が NUL を返して `out` に積まれ続ける:

- stdin が閉じられるとミニバッファは永久にループし、echo_tail が NUL を
  print し続ける(実測: 3秒で 25.8MB / exit 不能・timeout kill)。
- 全ミニバッファプロンプタ(C-r / C-l / C-x C-f / C-w / C-i / C-x b / C-x C-b /
  **終了確認**)が EOF 後にハングする。終了確認中の EOF はコメント
  (`main.rs:34-35`)・README:148・design/keybindings.md:13 の
  「EOF exits unconditionally」と正面から矛盾する。
- ハング時は `restore_mode` が呼ばれないため、raw mode が漏れる
  (端末が改行も echo も効かなくなる)。

修正案: ループ内で `read_byte_eof()` を使い、EOF では `""` を返して
キャンセル扱い(Emacs の C-g に相当)。あわせて `main.rs` 側の
`confirm_exit` は EOF/キャンセル時に終了しない仕様を明確化する。

### T-2. read_char_from も EOF で NUL を積む
`terminal.rs:71-92`: continuation byte を `read_byte()` で読むため、EOF で 0 を
読み、`String::from_utf8(vec![先頭, 0])` が失敗して `' '` になる。C-q 直後の
EOF は ' ' を挿入し、UTF-8 途中での EOF は ' ' になる(無限ループにはなら
ないが、入力が黙って文字化けする)。`read_byte_eof` を使い、EOF では
挿入を中止するのが正しい。

## 中

### T-3. raw mode 復元が正常リターンにしか紐付いていない
`save_raw_mode` (`terminal.rs:19-39`) は起動時に呼ぶが、復元は
`main.rs:396` の正常経路だけ。以下で **raw mode が漏れる**:
- panic(M-1 の溢れ panic で実測: exit 101、端末が生モードのまま)
- SIGTERM / SIGHUP / SIGKILL(ISIG は切っているので C-c は捕まるが、
  `kill` は捕まらない。POSIX ではプロセス終了後も termios 設定は残る)
修正案: `restore_mode` を Drop で呼ぶガードオブジェクトを main の先頭に
置き、`std::panic::set_hook` で panic 時復元、SIGTERM/SIGHUP ハンドラで
復元して終了。

### T-4. read(2) エラーを EOF と同一視する
`read_byte_eof` (`terminal.rs:56-63`): `match r { Ok(1) => …, _ => EOF }`。
EINTR(シグナル割り込み)で read が失敗すると「EOF」になり、メインループが
-editor は SIGINT 系を受けない設定でも-予期せず終了しうる。
エラー(EINTR 含む)では再試行、0 バイト読みでのみ EOF とすべき。

## 低

### T-5. fd 0 ハードコード
`terminal.rs:20,46`: stdin がパイプ/ファイルのとき termios は失敗し
raw mode は設定されない(`None` を返して復元もしない)。tmux e2e は pty 側で
動作するので設計は成立しているが、「stdin が tty でないときは
raw mode をあきらめる」旨のコメントを関数 doc に書くべき。

### T-6. Tab 補完の 0 候補時に無音
`terminal.rs:335-377`: 候補が 0 件なら bell も表示も無く無反応。
Emacs は bell を鳴らす。`print!("\x07")` で十分。

### T-7. list_width / cand_at は文字数基準
`terminal.rs:244-276`: 候補リストの echo 幅とキャレット判定が display cells
でなく文字数。S-2 と同じ理由で CJK 候補(ファイル名に日本語)では位置計算が
ずれる。ミニバッファの echo 自体は 1 行なので実害は軽いが、Enter-on-candidate
の当たり判定がズレ得る。

### T-8. file_candidates の細部
`terminal.rs:170-242`:
- ディレクトリ候補に末尾 `/` を付けないので、ディレクトリと判別できず、
 TAB 一意補完後も自分で `/` を打つ必要がある(UX)。
- 候補のソートが `read_dir` 順(ファイルシステム順)で不定。Emacs は
  アルファベット順。ソートを入れる。
- `~` の未展開。T-6 と合わせて小改善の塊にできる。

### T-9. VMIN/VTIME の未設定(現状は問題なし)
raw mode で VMIN/VTIME を触っていない。ICANON 解除済みで VMIN は継承される
ため、通常の tty では read は 1 バイト到着で返る。明示的に
`t.c_cc[VMIN]=1; t.c_cc[VTIME]=0;` を設定すると環境依存が消える。

### T-10. read_line_c のカーソル/echo ロジックは丁寧に作られている
`terminal.rs:279-293` (echo_goto/echo_tail)、`cand_at` の span 規定と単体テスト
(`terminal.rs:474-497`)、入力編集で古い候補リストを消す (`terminal.rs:434,449-451`)
など、状態の整合への配慮が-visible。コメント(259-263行目)も正確。good。

### T-11. flush_stdout の位置
`terminal.rs:10-16`: stdout が行バッファのままの設計をコメントで説明しており
明快。print! 直後の flush が全経路に散っており (`screen.rs:197,199,270` 等)
忘れやすいので、出力ヘルパー(print + flush)に集約すると安全。
