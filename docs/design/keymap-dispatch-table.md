# Design Doc D: キーディスパッチのテーブル化(M-11)

## 1. 背景

`src/main.rs` のメインループ(`loop { ... }` 本体、main.rs:121-548 の約 430 行、
review 当時の計測で「約 300 行」)は 1 関の平坦な if-else 連鎖である
(review/02-main.md §M-11)。

- トップレベル byte `b` を読み、`b == 0x18`(Ctrl-x)/`b == 0x1b`(Meta)の
  プレフィックスは追加の byte を読んで第二の if-else 連鎖、という構造。
- `design/architecture.md`「Key input」は「keymap tree で解決」と書くが
  実装は平坦連鎖で不一致(07-docs-consistency.md 指摘)。

目的は (a) 保守性(1 コマンド 1 関数)、(b) ドキュメント整合(keymap 表の
所在を 1 か所に)、(c) バインド網羅のコンパイル時検査(enum match)。
**挙動・出力 byte 同一**が最重要ゲート(全 89 e2e シナリオがキー入力経路)。

## 2. 現状の正確な構造(2026-10-25 ツリー)

ディスパッチの順序・陥没ポイントを列挙する(リファクタで維持すべき不文律):

1. ループ頭: `draw_all(&bufs, tree, active, msg.to_string())` → tree 再生成。
   `bi = tree.leaves[active].buffer`。M-15 の point/mark クランプは**双方向**
   (クランプ値を leaf にも buffer にも書き戻す、main.rs:128-133)。
2. `read_byte_eof()` EOF → break。`b` 取得。
3. **C-c 陥没**: `b == 0x03` は exit 確認後 **fall through** して素 map へ
   落ちるが、0x03 の arm が無いので no-op(`msg="Cancelled"` は残る)。
   `prev_cmd = 0x03` になる(M-12 の kill-run が壊れる=仕様通り)。
4. `b == 0x18`: `n = read_byte()` → C-x map(下表)。未バインド →
   `msg = format!("Undefined command: C-x {}", n as char)`(M-7)。
   C-x 中の EOF は `read_byte()` が 0 を返し "Undefined command: C-x \0" 扱い。
5. `b == 0x1b`: `n = read_byte()` → Meta map。未バインドは**無言**(現状)。
6. 素 map(下表)。printable 自挿入は `(0x20..0x7f)` と `b >= 0x80`
   (`read_char_eof` がcontinuation を消費、EOF で何も挿入しない)。
7. ループ末: `if !moved { sync_leaf(&mut tree, &bufs, active, bi); }`
   → `prev_cmd = b`。`moved` は leaf の**表示切替**(window/buffer 切替で
   attach/show した時 true=その leaf の point は既に同期済みなので
   sync_leaf をスキップ)。C-x/C-s 中の byte は prev_cmd に触れない(M-12)。

### バインド表(全 42+ 系統)

| プレフィックス | byte | コマンド | 副作用 |
|---|---|---|---|
| 素 | 0x03 | Quit(confirm_exit で yes と break) | 取消時 msg=Cancelled、素 map に 0x03 arm 無し=no-op 着地 |
| 素 | 0x18 | →C-x map | |
| 素 | 0x1b | →Meta map | |
| 素 | 0x13 | isearch(順方向、専用ループ) | msg=Cancelled/空 |
| 素 | 0x12 | isearch(逆方向)= C-r。0x13 と同一ループの `backward=true`。read_line は廃止 | msg=Cancelled/空(未実行時の "Search string not found" は消えた) |
| 素 | 0x16 | scroll-page-down(C-v): 次 line を rows-2 回(列維持、境界で clamp) | 無言・undo entry なし |
| 素 | 0x17 | kill-region(C-w): mark↔point 間を cut(point/mark は region 先頭へ)。region 空は何もしない | killed 蓄積(prev_cmd==0x17 の連続 run)。空 region は無言(entry 也無) |
| 素 | 0x07 | cancel | msg=Cancelled |
| 素 | 0x00 | set-mark | msg=空 |
| 素 | 0x01/0x02/0x06/0x0e/0x10/0x7f/0x04/0x0f/0x14/0x0b/0x19/0x05 | beginning-of-line / backward-char / forward-char / next-line / previous-line / delete-backward / delete-forward / open-line / transpose-chars / kill-line / yank / end-of-line | kill は killed 蓄積(prev_cmd==0x0b) |
| 素 | 0x0c | go-to-line(read_line、M-1/M-10) | msg various |
| 素 | 0x11 | C-q quote(次 byte を read_char_eof で自挿入) | EOF 時は read_byte=0 → NUL 1 文字挿入後、ループ頭 EOF で終了(Design Doc A の既知 quirk、コメント維持) |
| 素 | 0x0a/0x0d | 改行挿入 | |
| 素 | 0x1f | undo(T-9:`C-/`。xterm 系では `C-_` と同 byte) | msg=Undo/No change to undo。グループ化:タイピング連続(自挿入・改行・C-q)=1 グループ、C-k 連続=1 グループ、それ以外のコマンドでグループ切れ。履歴は 200 件上限、C-x k で当該 buffer の entry を破棄 |
| 素 | 0x20..0x7f, ≥0x80 | 自挿入 | |
| 素 | その他(printable でない未バインド byte) | **無言 no-op**(現状。0x03 着地もここ) | |
| C-x | 0x03 | Quit(同上) | 取消時 msg=Cancelled |
| C-x | 0x31/0x32/0x33 | delete-other-windows / split-below / split-right | msg=空 |
| C-x | 0x6f | other-window(cycle) | moved=true, msg=空 |
| C-x | 0x06 | find-file(T-8 ~、M-6 再利用、M-9 失敗報告) | 切替時 moved=true |
| C-x | 0x13 | save | msg=Wrote/Failed/No file name |
| C-x | 0x17 | write-file(M-8 成功時のみバインド) | msg various |
| C-x | 0x09 | insert-file | msg various |
| C-x | 0x6b | kill-buffer(M-5 確認、M-15 全 leaf リセット) | msg various |
| C-x | 0x62 / 0x02 | switch-to-buffer / buffer-list(cycle 含む) | 切替時 moved=true |
| C-x | その他 | Undefined command | msg=Undefined |
| Meta | 0x5b+t | 矢印 A/B/C/D(前後行/文字) | |
| Meta | 0x62/0x66 | backward/forward-word | |
| Meta | 0x3c/0x3e | beginning/end-of-buffer | |
| Meta | 0x76 | scroll-page-up(M-v): 前 line を rows-2 回 | 無言・undo entry なし |
| Meta | その他 | **無言 no-op**(現状維持) | |

## 3. 設計

### 3.1 `src/keymap.rs`: バインド表(表はここ)

```rust
#[derive(PartialEq, Eq, Debug)] // §5 の表テストに必須
pub enum Cmd {
    PrefixCtrlX, PrefixMeta,          // プレフィックス検出(main が次 byte を read)
    PrefixArrow,                      // Meta 0x5b: main が t を read して 4 系統へ
    Quit, SelfInsert, QuoteInsert, Newline,
    Cancel, SetMark,
    BeginningOfLine, EndOfLine, BackwardChar, ForwardChar,
    NextLine, PreviousLine, BeginningOfBuffer, EndOfBuffer,
    ScrollPageDown, ScrollPageUp,     // C-v(0x16)/ M-v(0x76): screenful 移動
    BackwardWord, ForwardWord,
    DeleteBackward, DeleteForward, OpenLine, TransposeChars,
    KillLine, KillRegion, Yank,       // T-9: Undo=C-/(0x1f)。C-w=KillRegion(0x17 素)
    GotoLine, Isearch,                // C-s/C-r は 1 つの isearch(backward) ループ
    FindBackward,                     // 0x12: 同一ループの backward 方向(arm は isearch(true))
    FindFile, Save, WriteFile, InsertFile, KillBuffer, SwitchBuffer(bool), // 0x02=list形式
    DeleteOtherWindows, SplitBelow, SplitRight, OtherWindow,
    Arrow(usize),           // main が 0x5b の後の t を read して 0=U,1=D,2=R,3=L
    Unbound(u8, u8),        // (prefix, byte): C-x 未バインドの msg 生成専用
    MetaUnbound,            // 無言(現状維持)。素 byte の無言着地は main 側 `_ => {}`
}
// 素 map の未バインドは Unbound を使わない(無言)。Unbound は C-x 専用。

pub fn plain_key(b: u8) -> Cmd;   // match b: ... => Cmd::X, 0x18 => Cmd::PrefixCtrlX(内部), ...
pub fn ctrl_x_key(n: u8) -> Cmd;  // match n: 上記表、_ => Cmd::Unbound(0x18, n)
pub fn meta_key(n: u8) -> Cmd;    // match n: ... _ => Cmd::MetaUnbound
```

`plain_key` が `PrefixCtrlX`/`PrefixMeta` を返すことで main のディスパッチは
**プレフィックス read_byte の位置をそのまま維持**する(プロンプトを出す
コマンドの途中で read する現状の順序は変えない)。
矢印は**読み取り順 `ESC → read n → n==0x5b なら read t → t∈{A,B,C,D}` が不変**
(M-3 の isearch ESC-`[` 消費が同一順序に依存)。`meta_key(0x5b)` は
`Cmd::PrefixArrow` を返し、**main が t を read** して 0x41..0x44 を Arrow へ
写す(未バインド t は無言)。isearch 内の ESC 経路(受容+矢印移動)は
C-s メソッド内部に現状のまま残す。
match は網羅(`_ =>`)なので**新 variant の追加は main の dispatch match で
コンパイルエラーになる**(keymap 側は `_` 必須)。

### 3.2 `main.rs`: `Ed` 集約 + 1 コマンド 1 メソッド

```rust
struct Ed {
    bufs: Vec<buffer::Buffer>,
    tree: window::Tree,
    active: usize,
    msg: String,
    killed: String,
    prev_cmd: u8,
    moved: bool,
}
impl Ed {
    fn bi(&self) -> usize;                      // 現 leaf の buffer index
    fn sync_in(&mut self);                      // M-15 クランプ(leaf→buffer)
    fn sync_out(&mut self);                     // !moved なら sync_leaf
    fn confirm_exit(&self) -> bool;             // 現状の free fn をメソッド化
    // --- commands (各 5〜40 行) ---
    fn quit(&mut self) -> bool;                 // true = break 指示
    fn other_window(&mut self);                 // ★内部で sync_leaf → flatten →
                                                //   cycle、moved=true(現状の loop 内syncと同一位置)
    fn find_file(&mut self);
    fn save(&mut self); fn write_file_cmd(&mut self); fn insert_file(&mut self);
    fn kill_buffer(&mut self); fn switch_buffer(&mut self, listform: bool);
    fn isearch(&mut self);                      // C-s 専用ループ全体
    fn find_backward(&mut self); fn goto_line(&mut self);
    fn kill_line(&mut self);                    // prev_cmd 蓄積ロジック
    fn arrow(&mut self, dir: usize);
    fn self_insert(&mut self, b: u8); fn quote_insert(&mut self);
    ...
}
```

`Tree` を `draw_all` が消費・返却する問題はフィールド生やし替えで解消:

```rust
let t = std::mem::replace(&mut self.tree, window::new_tree(0)); // 使用済み slot
self.tree = screen::draw_all(&self.bufs, t, self.active, self.msg.to_string());
```

(`new_tree(0)` は即破棄のダミー。`msg.to_string()` は現状の呼び出し式と同一)

ループは:

```rust
let mut ed = Ed::startup(args); // 起動引数バファ(M-9 は msg 設定、main は () のまま)
// 起動は main() の現状手順(args 読み→bufs 構築)を Ed 構築に移すだけ。? は使わない
loop {
    ed.frame();
    ed.sync_in();
    let b = read byte, EOF -> break;
    match keymap::plain_key(b) {
        Cmd::PrefixCtrlX => { let n = terminal::read_byte();
            match keymap::ctrl_x_key(n) { ... ed.xxx() ... } }
        Cmd::PrefixMeta  => { let n = terminal::read_byte();
            match keymap::meta_key(n) {
                Cmd::PrefixArrow => { let t = terminal::read_byte();
                    match t { 0x41..0x44 => ed.arrow(t), _ => {} } } // 無言
                Cmd::Arrow(..) => 使いなし(PrefixArrow 経由のみ),
                ... (b/f/</>)  _ => {} /* Meta 未バインド無言 */
            } }
        Cmd::SelfInsert => ed.self_insert(b),
        ...
    }
    ed.sync_out();
    ed.prev_cmd = b;
}
```

C-c fall through は「confirm_exit false → msg=Cancelled、素 map では
0x03 は Unbound 相当で no-op」として素 map match の `_ => {}`(無言)で
同一に保つ。

### 3.3 不変条件(レビューで監査する点)

1. **read 順序**: プロンプト(read_line 系)を伴うコマンドはディスパッチ
   内で read する(現状と同一)。プレフィックスの read_byte はプレフィックス
   検出直後 1 回のみ。
2. **moved**: 現状 true を立てる 5 箇所(window/buffer 表示切替)のみ。
   sync_out は `!moved` のときだけ。
3. **prev_cmd**: トップレベル byte `b` のみ。プレフィックス内 byte は触れない。
4. **msg 意味**: 各コマンド末の msg 文字列は現状の全 pattern を逐語維持
   (e2e が grep で固定している文言多数)。
5. **C-k 蓄積**: prev_cmd==0x0b の push_str/置換。
6. **EOF**: read_byte_eof の EOF は break。read_byte(プレフィックス)は 0。
7. 出力 byte 列・画面順序は不変(`frame()` = draw_all 呼び、呼び出し頻度同)。

## 4. 対象外(Non-goals)

- キーバインドの**変更・追加・設定化**は一切しない(表の内容は現状の写像)。
- Emacs のような動的 keymap(キーマップ Stack、`M-x` 記述)は作らない。
  「表 1 個 + enum 網羅 match」まで。
- `Ctrl-x k` 追加などの機能変更は禁止(この fix は pure 構造変更)。

## 5. テスト戦略

- 既存 e2e 89 シナリオ全通過(全キー経路を踏む)+release smoke 6/6。
- unit(`keymap.rs`): バインド表のアサート(例 `plain_key(0x02)==Cmd::BackwardChar`,
  `ctrl_x_key(0x62)==Cmd::SwitchBuffer(false)`、`ctrl_x_key(0x7a)==Unbound(0x18,0x7a)`,
  `meta_key(0x7a)==MetaUnbound`, 0x20..0x7f 範囲の SelfInsert 判定,
  矢印 `meta_key(0x5b)` は Arrow ではない=矢印は 2 段 read であることを明記)。
- `RUSTFLAGS=-D warnings`/clippy `--all-targets` 0(`#[allow]` 不使用)。

## 6. リスクと対策

| リスク | 対策 |
|---|---|
| `moved`/`prev_cmd`/C-c fall through の陥没 | §3.3 不変条件をゲート reviewer が HEAD と 1:1 監査 |
| `draw_all` の tree 生やし替え(Dummy tree 混入) | `frame()` 内のみ使用、ループから `ed.tree` を直接触る箇所を残さない |
| `Ed` の borrowck(例 bi と tree の同時借用) | `bi` は usize(Copy)、`bi()` ヘルパーで毎回取り出す(現状の `let bi` と同一パターン) |
| isearch ループの移動 | ループ全体を 1 メソッドに**丸ごと**移す(内部ロジック不変) |
| msg 文言の取りこぼし | e2e の status grep が全 pattern を固定済み。全 msg 文字列を diff で機械比較 |

## 6.5 実装時の必守リスト(Design ゲートMajor/Minor 織り込み済み)

1. C-x o はハンドラ**内部**で sync_leaf → flatten → cycle、moved=true。
2. 矢印は `ESC → n → (0x5b なら) t` の read 順不変。Meta 未バインドは無言。
3. 素 byte 未バインド(0x03 含む)は無言。`Unbound` は C-x 専用。
4. loop 骨格=frame→sync_in→read_byte_eof(EOF break)→dispatch→
   sync_out(moved で skip)→prev_cmd=b。クランプは leaf/buffer 双方へ。
5. moved=true は 5 箇所のみ(C-x o、C-x C-f 再利用/新規、C-x b attach/新規)。
6. プレフィックス EOF=byte 0(C-x \0 の Undefined 文言)、C-q EOF=NUL 1 挿入
   (Design Doc A)、self-insert EOF mid-UTF-8=無挿入。
7. field-swap は `msg.to_string()`(現状式)。tree 直接操作は frame() 内のみ。
8. msg 文言は全 string 逐語維持(e2e grep)。isearch はループごと 1 メソッド
   (ESC-`[` 消費含む)。confirm_exit=read_line_eof、C-x k=read_line の区持維持。
9. Cmd は main 側 match 網羅(`_` 無)、byte 側 map のみ `_ =>`。
10. ゲート=e2e 89/89・smoke 6/6・unit・`RUSTFLAGS=-D warnings`・clippy 0・
    `#[allow]` 禁止。

## 7. ドキュメント整合

fix #27 の範囲で `design/architecture.md` を 2 箇所実装と一致させる:
(a)「Key input」節の「keymap tree で解決」→「`src/keymap.rs` の単一バインド表
(`plain_key`/`ctrl_x_key`/`meta_key` の 3 関数、`Cmd` enum 網羅 match)で解決」。
(b) 末尾 "Current implementation (as-built)" の**「Key dispatch is a flat if-else
chain … main.rs」bullet**(architecture.md:82-84、D-3/fix #11 で追加)を
キーマップ表+1 コマンド 1 メソッドの記述へ書き換える。ここを放置すると
M-11 が解消するはずの不一致を本 fix が再導入する。
`design/keybindings.md` のバインド一覧は内容変更なし(表と照合して一致確認)。
本 Design Doc は review/TODO-261024 の「Design Doc 対応」に D として登録し、
M-11 エントリと学習記録から参照する(前例 A/B/C に倣う)。
