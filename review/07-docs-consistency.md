# レビュー: ドキュメントと実装の整合 (README.md / design/*.md)

コード挙動・実測と突き合わせた結果。design/ は仕様書として丁寧に書かれて
いるが、実装が先行してズレた箇所が複数ある(実装側の問題というより
ドキュメントの更新漏れ)。

## 高(記述が実装と矛盾し、ユーザー/開発者を誤誘導する)

### D-1. 「EOF は無条件に終了」(README:148, design/keybindings.md:13) はミニバッファ経路で偽
実測で EOF 時の無限ループを確認済み(04-terminal.md T-1)。
「closing stdin / EOF exits unconditionally」は「メインループがキー読み取り中」
の条件付き。修正後は「終了確認プロンプト中の EOF はキャンセル」に書くべき。

### D-2. design/README.md:24-26 の "Not implemented" 列表が陳腐化
- 「Modes, syntax highlighting」→ **シンタックスハイライトは実装済み**
  (highlight.rs、README:149-156)。design/README.md は冒頭から矛盾。
- 「Full minibuffer command framework」→ Tab 補完・候補選択・カーソル移動・
  ファイル/バッファ補完まで実装済み(read_line_c)。"一部実装" に更新を。
- 「Undo/redo … Not implemented」は事実(実装なし)で正しい。ただし C-x k の
  無確認破棄(M-5)と相まって「やり直しが効かない」ことを design/README.md の
  风险注記として明記すべき。

### D-3. design/architecture.md の記述が複数、実装と一致しない
| 記述 | 実態 |
|------|------|
| :9 gap buffer「償却 O(1) の挿入」 | 挿入は O(n)(01-buffer.md B-1) |
| :50 「keymap tree でバイト列を解決」 | 平坦な if-else 連鎖(main.rs:75-389、M-11) |
| :59 「`curses` が毎フレーム描画」 | 手書き ANSI print!。curses は size 取得のみ(S-4) |
| :61 「status/echo line at the **top**」 | 最下行(screen.rs:190、README:137) |

architecture.md は「設計の理想」を残す方針なら「現状の実装」節を分けて
明記するのが誠実。

### D-4. README:129 「All windows show the same buffer text」が C-x b/C-x C-f の挙動と矛盾
split 直後は全窓同一バッファだが、`C-x b`/`C-x C-f` は **アクティブ窓だけ**
のバッファを切り替える(main.rs:133,200)ので、以降は窓ごとに別バッファ表示。
README:132-133 の e2e 参照(s120-s124 は per-window buffers を検証済み)と
同じ文で「同一テキスト」と言うのは誤り。「split 直後は同一、切替後は窓ごと」
に書き直す。

## 中

### D-5. design/keybindings.md:27-28 は `C-<`/`C->` を載せたまま
実装も README も `M-<`/`M->`(main.rs:315-318、README:175)。
design/TODO.md:13 にも「raw tty は C-</C-> を送れないので Meta 縛り」とある
ので、keybindings.md 側の更新漏れ。

### D-6. design/keybindings.md の軽微な冗長
- `C-d` が Movement(:29)と Editing(:39)に二重掲載。
- 凡例(:3)の `C-\` は未バインド。
- Minibuffer 節(:86-95)は実装(read_line_c)と良く一致している。good。

### D-7. README:47 の「(termsize)」は推移依存の直呼び
size 検出は `curses::canvas::get_size()`(screen.rs:12)。直接依存は
curses のみで、termsize は Cargo.lock 上の推移依存。curses に書くか
「curses(size 検出のみ)」と注記を。

### D-8. README の Tab 補完の射程説明は正確だが前提が書かれていない
README:107-113/196 の「path and buffer-name prompts で Tab 補完」は
read_line_files/read_line_buffers の呼び分けと一致(良好)。
C-s/C-r/C-l の minibuffer では補完が効かない(素の read_line)ことも
一行添えると親切。

### D-9. README:74-79 「missing files open empty」は実装と一致
read_file が全エラーを無視して空文字列(main.rs:7-12)なので文言どおり。
ただしエラー種の区別なし(M-9)は Emacs ユーザーの期待と違うため、
「ファイルが存在しない場合は空で開く(ディレクトリ/権限エラーも同様)」と
注記を。

### D-10. README:31-34 の環境記述は開発環境スナップショット
開発マシン・ローカル LLM・モデル名の記述はリポジトリの不変情報としては
脆い(モデル ID は opencode.json と二重管理で、現在 opencode.json の方は
未コミット変更中)。開発環境記録は別ドキュメント(例 docs/dev-env.md か
CHANGELOG 的運用)に逃がすことを検討したい。

## 低(正確で良好な点 = 更新時の基準)

- README:164-198 のキーバインド表は main.rs のディスパッチと全項目一致を
  確認(C-s 増分検索の wrap/bell/C-g 復元、C-x C-b の Enter 巡回、
  Enter-on-candidate の挙動まで正確)。
- README:157-160 のレンダリング堅牢性(制御文字空白化・カーソル非ステータス)は
  screen.rs:231-233,192-195 と一致。
- README:137-142 のステータス行フォーマット、README:134-136 のスクロール
  挙動(端を 1 行ずつ追従)は screen.rs と一致。
- design/TODO.md の Notes(13 行)は実装と一致。
- README:36-39 のツールチェーン(rustc 1.99.1)は本実測環境と一致。
