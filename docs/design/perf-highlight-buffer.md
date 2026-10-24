# Design Doc B: 描画・検索・ハイライト性能 (O(n²) 排除)

対象 TODO タスク: **H-1, S-1, B-1, B-2, H-2, H-3**(同実装で **B-4** も先に解消)
出典: `review/01-buffer.md`、`review/03-screen-window.md`、`review/05-highlight.md`
受入目標: **451KB .rs ファイルの起動時 1 フレーム < 1 秒**(現状実測 12.9 秒)、
`cargo test` 全通過・`cargo build` 警告ゼロ・e2e 68+3 全 PASS 維持。

## 1. 現状の計測事実

- `heq_at(cs: Vec<char>, ..)` が呼出毎に全文 clone → `styles` が O(n²)/呼出
  (highlight.rs:15-28)。`draw_all` が**毎フレーム全バッファ**に実行 (screen.rs:142)。
- `draw_all` は `buf.text()` を同一バッファに 3+ 回、`split_lines` 全走査、
  全面 print (screen.rs:93,140,142,166)。
- gap buffer は gap を前確保せず、挿入は `Vec::insert` で 1 文字 O(n)
  (buffer.rs:20-31,62-69)。`insert-file` (main.rs:167) は全文これを n 回。
- `find_forward/backward` は毎回 `text()` を生成 (buffer.rs:283,313)。

## 2. 設計

### 2.1 buffer.rs — gap 前確保(B-1)と検索の直接スキャン(B-2)

- `Buffer::new`: text を chars に詰めた後、**末尾に「サイズ GAP=8192」の gap** を
  予約する: `chars.resize(n + GAP, '\0')`; **`gap_start = n; gap_end = n + GAP`**
  (gap の「窓」が `[n, n+GAP)`、サイズ GAP。`gap_start = gap_end` はサイズ 0 で
  len() が GAP 分過大になるため不可 — レビュー F-1)。point/mark は 0 のまま。
  len()/to_index()/move_gap は無修正(不変条件「gap 内は padding、live text で
  ない」は struct コメントに明記)。最初の挿入地点へ gap を運ぶ 1 回ぶんの移動は
  Occur(実測: 451KB で slide 2.6ms/debug)。
- `insert_char`: 前確保済み gap(`gap_start < gap_end`)の消費経路(`chars[gap_start]
  = c; gap_start += 1`)が毎回数える → O(1)。
- **再充填(レビュー F-2)**: gap 枯渇(`gap_start == gap_end`)で挿入が必要なとき、
  **挿入地点で in-place に gap を作る**:
  `chars.splice(gap_start..gap_start, std::iter::repeat('\0').take(GAP))`
  (右セグメントを後ろへ押し広げる)、`gap_end = gap_start + GAP` としてから
  通常の消費経路で 1 文字書く。**末尾 resize を使わない**(末尾に積んだ '\0' は
  挿入地点の gap にならない)。1 文字 `Vec::insert` を GAP 回繰り返す型は
  O(n·GAP) なので禁止。挿入ごとに償却 O(1)(GAP 回に 1 回の O(n))。
- **latent bug(F-4)**: `insert_newline_after` の `Vec::insert` 分岐に
  `gap_start += 1` が無く、挿入した `'\n'` が gap 窓に呑まれて**黙殺**される
  (fresh load 直後の C-o が no-op になる潜在バグ。既存テストは先行 delete で
  gap が非空なため素通り)。本タスクで `gap_start += 1` を追加し、fresh 直後の
  C-o と再充填境界での C-o のテストを追加(テスト名 C-o1/C-o2)。
- `find_forward(self, pat, from) -> Option<usize>`(B-4 を先取り): `text()` を
  作らず `char_at(i + k)` で直接比較。guard は `from + m <= len()`。
  戻り値は match 末位置(`Some(i + m)`)で現状と同じ。`find_backward` も同型
  (後方スキャン、`i` から 0 まで、`Some(i)`)。呼び出し側(main.rs C-s×3, C-r×1)の
  `hit.len() > 0`/`hit[0]` を `if let Some(..)` に置換。
- テスト(buffer.rs tests): gap 予約後 text() 同一性、挿入で gap が縮み枯渇で
  in-place 再充填されること(挿入のみ列で `GAP-1 <= chars.len()-len() <= GAP`)、
  find 前後方・折返し境界、`find_backward` 終端クランプ、空バッファ delete_forward、
  **fresh 直後の C-o** と**再充填境界の C-o**(F-4)。
  **注意(F-11): 変更後 `chars.len() = len() + gap`。テストのテキスト長検証は
  `len()`/`text()` を使い `chars.len()` を使わない。gap サイズ検証は導出量
  `chars.len() - len()` で行う。**

### 2.2 highlight.rs — スライス参照(H-1)とヒューリスティクス(H-2/H-3)

- シグネチャを所有権を渡さない形へ:
  - `heq_at(cs: &[char], i: usize, w: &str) -> bool`
  - `hchars_of(s: &str) -> Vec<char>`(detect の被参照実装用)
  - `detect(name: &str) -> usize`(H-3: 拡張子を ASCII 小文字化して比較;
    呼び出し側は `detect(buf.name.clone())` → `detect(&buf.name)`)
  - `styles(cs: &[char], lang: usize) -> Vec<usize>`、`md_styles(cs: &[char], st)`
  - `kw(lang, w: &str)`(借用語で呼び出し側 `w: Vec<char>` → String 生成廃止可)
- 内部の `cs.clone()` 引数呼び出しを全てスライス参照に置換(H-1 の本体、
  7 箇所: 70-90,183,193,196,198,212,215)。
- **H-2(lifetime 誤色)**: `lang == 1` の `'` は次を満たすときのみ文字列開始:
  (a) 閉じ `'` が同一行内に存在(現行スキャナは `'\n'` で break 済みなので gratis)
  (b) 開き/閉じの間隔が 2 文字以上(j >= i+2)
  (c) **内容が string 的**: 空白・`<`・`(`・`;`・`&` を含まない
  (条件不足なら `'` を 1 文字としてスキップ=フォールスループト)。
  固定フィクスチャ(テストで pins): `fn f<'de>(x: &'de str) {}` → どの `'` も
  string(2) にならない(ペア lifetime が誤爆する「2 文字以上」単独ルールは
  レビュー F-5 で不可と確認)。char リテラル `'a'` は string(2) 装飾(許容)。
- 数値スキャン(H-5)と HTML 属性(H-6)は Low のまま本ドキュメント対象外。

### 2.3 screen.rs — フレーム内キャッシュ(S-1、S-10 を含む)

- フレーム毎に**バッファ index ごとに 1 回だけ**以下を計算するキャッシュを作る
  (`Vec<Option<BufView>>` を draw_all のローカルに。BufView = `{ chars: Vec<char>,
  lines: Vec<String>, offs: Vec<usize>, toks: Vec<usize> }`):
  - `chars = buf.text().chars().collect()`(text() 呼び出し 1 回)
  - `lines`/`offs` は chars から 1 回、`toks = styles(&chars, lang)` 1 回
  - 同一バッファを複数窓が共有する場合(分割直後)は 1 回分。
  - **メモリ(ゲートレビュー注記)**: `toks` は**文字数ぶん**の `Vec<usize>`(styles は
    1 文字 1 エントリを push する実装)。 BufView ≈ 15B/char
    (chars 4B + toks 8B + lines UTF-8 1〜3B + 行あたり String/offs 24B+8B) なので
    451KB ファイルで **約 7〜9MB**。draw_all ローカルなので**フレーム終了時に
    drop され常駐しない**。許容(Emacs 相当のエディタがフレーム中に 1 副本時
    レプリカを持つのは通常運転)。
- `point_pos` は chars(スライス)基準の関数に刷新し、**leaf ごとに 1 回だけ**
  計算してスクロール処理(screen.rs:93 相当)とカーソル処理(screen.rs:166 相当)で
  **局所変数に保持して共有**する(S-10)。実装が 2 箇所それぞれで再計算する形に
  すると S-10 が未了になるので、キャッシュの受け口を 1 つ明文化する。
- `line_offsets` は chars 版(行数ぶん、+1 加算は現状通り)。
- **スコープ外(明記)**: 可視行範囲のみの styles 計算(状態追跡が必要)、
  差分描画(dirty 行のみ print)。これらは現状の O(n) 1 パスで目標を達成できる
  見込みのため据え置き。未達なら追加 Design Doc。

### 2.4 main.rs / buffer.rs — 呼び出し適合と挿入 API

- **`Buffer::insert_string(self: &mut Buffer, s: &str)` を追加(ゲートレビュー注記)**:
  1 文字ずつ `insert_char` を呼ぶループを API に集約する。実装は
  `for c in s.chars() { self.insert_char(c) }` でよい(gap 償却 O(1) により
  全体 O(len)。**s が gap 残量より大きい**ときに 1 回 splice でまとめて
  開ける最適化は必須ではない — 必須は「ループを集約した API の存在」)。
  使用者:
  - `insert-file` C-x C-i(main.rs:178 の `for c in content.chars() { insert_char }`)
  - yank C-y(main.rs:362-363 の `for c in killed.chars() { insert_char }`)
  どちらもループ本体を `bufs[bi].insert_string(&x)` に置換(modified フラグ設定は
  呼び出し側維持のまま)。
- find 呼び出し 4 箇所の Option 化(B-4): **`find_forward`/`find_backward` の
  呼び出しのみ**(main.rs:251, 275, 276, 299)。`find_buffer`(main.rs:21)は
  バッファ名の一致列挙を返す**別関数で `Vec<usize>` のまま**(空=不一致の
  意味が「複数一致の列」なので Option にしない — ゲートレビュー注記)。

## 3. テスト計画

- unit: 2.1 の追加テスト + 新規 `highlight` tests(detect 拡張子マトリクス:
  `FILE.RS`→1, `a.b.rs`→1, `x.md`→4, `*demo*`→0; styles: rust lifetime が
  keyword 扱い行で string(2)にならないこと; json `'` 対象外; md 見出し)。
- 性能ゲート(手動・数値、**受け入れビルドは debug** — F-6。現状の 12.9s バグは
  debug で顕在化しており、release で計測すると regression を素通りするため):
  `time (./target/debug/emur /tmp/big_perf.rs </dev/null >/dev/null)` —
  451KB(451,559 バイト生成ファイル・再生成可)で **< 1s**(現状 12.9s)。
  設計見込みは **60〜100ms 程度**(styles O(n) 1 パス + 描画 O(n))なので
  ゲートには約 10 倍のヘッドルームがある。100KB 相当でも計測記録。
- 残存コストの明記(ゲートレビュー注記): フレーム毎に `text()`+`split_lines`+`styles` の
  O(n) が**残る**(可視範囲限定と差分描画はスコープ外、§2.3)。debug で
  451KB 1 フレーム約 17ms(text()+split_lines 分)が見込まれ、キー毎に全画面
  再描画の現状挙動は変わらない。目標(1 フレーム < 1s)には入るため許容。
- e2e 68+3 全 PASS、`cargo build`/`test` 警告ゼロ・全通過。

## 4. リスク

- gap 予約により `chars.len() != len()` が常態化 → `text()`/`char_at`/`swap_at`
  以外の直接添字(buffers.rs 内 gap 操作関数)が不変条件に依存。テストで固定。
  **padding 不変条件(ゲートレビュー注記)**: gap 内の `'\0'` はライブテキストではなく、
  `text()`/`char_at`/`split_lines` 系は**位置 `pos < len()` に対してのみ**
  `to_index` 変換で gap を飛び越える。`chars[pos]` を gap 窓の添字と混用する
  新コードを書かないこと(型では守れない。code review + unit test で固定)。
- **性能カバレッジの穴(ゲートレビュー注記)**: ゲート計測の `big_perf.rs` は ASCII のみ。
  CJK 混在(text() の UTF-8 → chars 変換を含む)の性能は本ゲートで保証されない。
  計測は本タスクの範囲外とし、CJK 表示そのものは Design Doc C(S-2/T-7)で扱う。
  数値のみの追跡として、100KB CJK テンポラリでの手計測を 1 回記録する。
- `styles(&chars, ..)` は `String` 生成済みの chars を受ける。screen 側は 1 回
  だけ text() を生成するため H-1 と相殺されて O(n) になる。
- 拡張子小文字化は「.RS が rust になる」変更。意図した挙動(README 側は D-2 で更新)。
- detect の `lang == 1` 共有(C 系と Rust)は分割しない(H-2 はヒューリスティックで対応)。
  分割は Low(H-9)として TODO に残す。
