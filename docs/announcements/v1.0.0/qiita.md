---
title: Rust と Tauri で Microsoft Project 風のスケジューラーを作り、v1.0 を公開した
tags:
  - Rust
  - Tauri
  - React
  - TypeScript
  - OSS
private: false
updated_at: ''
id: null
organization_url_name: null
slide: false
ignorePublish: false
---

デスクトップで動くプロジェクト計画ツール **ProjectDesk** の最初の正式版（v1.0.0）を公開しました。Microsoft Project と同じ考え方でタスクの依存関係から日程を組み立て、クリティカル パス、リソースの過負荷、EVM などを計算します。MIT ライセンスのオープンソースで、macOS / Windows / Linux 向けのインストーラーを用意しています。

この記事では、何ができるかを簡単に紹介したあと、作る過程で決めた設計と、ハマったところを書きます。

<!-- 画像は Qiita のエディターに images/projectdesk-gantt.jpg をドラッグして差し替えてください -->
![ProjectDesk の画面。左にタスク表、中央にガント チャート、下にタスクの詳細、右に EVM 指標とバーンダウン チャート](images/projectdesk-gantt.jpg)
*英語表示でサンプル プロジェクトを開いたところ（タスク名はサンプル データのベトナム語です）。*

:::note info
**画面の表示言語はベトナム語と英語です。** 初回はシステムの言語に合わせて選ばれ（ベトナム語以外のシステムでは英語）、「View → Language」メニューで切り替えられます。日本語 UI はまだありません。
:::

## できること

- **スケジューリング**：FS / SS / FF / SF の依存関係とラグから CPM でクリティカル パスを計算。制約、期限、手動スケジュール、タスクの分割、繰り返しタスク
- **カレンダー**：週の稼働時間、休日、例外日。プロジェクト用とタスク・リソースごとのカレンダー
- **リソース**：最大単位と単価。同じ時間帯の重なりから過負荷を検出し、平準化で解消
- **進捗管理**：ベースライン 0〜10、実績の開始日・終了日が日程を動かす（Project と同じ挙動）
- **レポート**：EVM（PV、EV、AC、SPI、CPI、EAC など）、バーンダウン、S カーブ
- **ビュー**：ガント、進捗ガント、リソース シート、タスク使用状況、ネットワーク図。PDF 書き出し
- **言語**：ベトナム語と英語。英語表示では日付は `2026-01-05 08:00`、所要時間は `7.5 days` のように表示し、用語は Microsoft Project の英語版に合わせています
- **ファイル**：`.pdproj`（SQLite）に元に戻す履歴ごと保存。30 秒ごとの自動保存と異常終了後の復元。Microsoft Project とは MSPDI（XML）で交換

## 構成

| 層 | 技術 | 役割 |
|---|---|---|
| エンジン | Rust（Tauri 非依存のクレート） | 日程、スラック、クリティカル パス、コスト、EVM の計算すべて |
| シェル | Tauri 2 | ウィンドウ、ネイティブ メニュー、ファイル ダイアログ、自動保存 |
| UI | React 19 + TypeScript | コマンドを送り、返ってきた結果を描くだけ |
| 保存 | SQLite | コマンド ログと基準のプロジェクトを保存 |

## 設計で決めたこと

### 1. 計算はエンジンだけが行う

UI は日付を一切計算しません。編集はすべて `Command` としてエンジンに送り、エンジンは再スケジュールした結果の `Snapshot` を返します。ガントのバーの位置も、ネットワーク図のレイアウトもエンジンが決めます。

こうしておくと、計算のテストを Rust 側だけで完結できます。エンジンには 170 件を超える自動テストがあり、実績日付まわりなどは手で計算した日付と突き合わせています。

### 2. 元に戻すはコマンド ログの再生

編集はすべて serde でタグ付けした enum として記録します（抜粋）。

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    RenameProject { name: String },
    SetProjectStart { start: i64 },
    SetStatusDate { date: Option<i64> },
    AddTask { name: String, after_uid: Option<u32> },
    // …
}
```

元に戻す・やり直すは、操作を逆算するのではなく「基準のプロジェクト + `commands[..cursor]`」を再生して作り直します。逆操作を個別に書かなくてよいので、コマンドを増やしても元に戻すが壊れません。その代わり、**すべてのコマンドが決定的で、基準から再生できること**が条件になります。

再生が遅くならないように、25 コマンドごとにスケジュール済みの状態をチェックポイントとして持ち、最も近いものから再生します。メモリには最大 24 個まで、現在に近いほど密に残します。

### 3. 決定的でない処理は「結果」を記録する

リソースの平準化はヒューリスティックなので、将来アルゴリズムを改善すると結果が変わります。そのまま再生すると、昔のファイルを開いたときに日程がずれてしまいます。

そこで、平準化の「要求」ではなく「決まった結果」をログに残しています（抜粋・簡略化）。

```rust
pub fn apply(&mut self, command: Command) -> Result<Snapshot, EngineError> {
    // 平準化の要求ではなく、決まった遅延をログに残す
    let command = match command {
        Command::LevelResources => command::leveled(&self.project)?, // => ApplyLeveling { delays }
        other => other,
    };
    // 複製に適用 → 再スケジュール → 両方成功したときだけ確定
    let mut trial = self.project.clone();
    command::apply_one(&mut trial, &command)?;
    schedule(&mut trial)?;
    // …
}
```

適用は複製に対して行い、スケジュールまで成功したときだけ確定します。途中で失敗しても状態は壊れません。

### 4. 時間は「プロジェクトの壁時計」で持つ

日時は Unix ミリ秒ですが、中身はタイムゾーン変換をしない「プロジェクトの壁時計の時刻」です。所要時間、ラグ、作業量はすべて**稼働分**で持ち、カレンダーが両者を変換します。標準カレンダーは 8:00〜12:00 と 13:00〜17:00 なので、1 日は 480 分です。

### 5. 保存で壊さない

`.pdproj` は SQLite で、`meta`（バージョン、カーソル、基準のプロジェクト）、`command_log`、`burndown` などのテーブルを持ちます。保存は隣に一時ファイルを作って 1 トランザクションで書き、元のファイルの権限を引き継いでから rename で置き換えます。書き込み中にアプリが落ちても、元のファイルは無事です。

ログの一部が壊れていても開けるようにしてあり、読める最後の状態まで復元して、何が失われたかを表示します。

### 6. 文言は型付きのカタログに置く

画面の文言は領域ごとのカタログ（`Toolbar.i18n.ts` など）に、ベトナム語と英語を並べて書いています。英語側の型をベトナム語側から作るので、キーの抜けや引数の違いはコンパイル エラーになります。

```ts
export function defineMessages<T extends Messages>(vi: T, en: NoInfer<T>): Catalog<T> {
  return { vi, en };
}

// 使い方の例
export const exampleMessages = defineMessages(
  { undo: "Hoàn tác", taskCount: (n: number) => `${n} công việc` },
  { undo: "Undo", taskCount: (n: number) => `${n} ${n === 1 ? "task" : "tasks"}` },
);
```

日付や所要時間の書式も言語に従います。英語では日付を ISO 形式だけ受け付けるので、`1/5/2026` を 1 月 5 日と 5 月 1 日のどちらと読むかで迷うことはありません。ネイティブ メニューやエラー メッセージは Rust 側で同じ 2 言語を持ち、メニューから切り替えるとその場で作り直します。

## ハマったところ

### macOS でファイル ダイアログが固まる

Tauri のダイアログ プラグインには `blocking_pick_file` がありますが、同期コマンドは macOS のメイン スレッドで動くため、そこでブロックするとパネルが run loop を取り戻せず固まります。非ブロッキングの `pick_file` をチャネルで待つ形にしました。

```rust
async fn await_pick<R: Runtime>(dialog: FileDialogBuilder<R>) -> Option<FilePath> {
    let (tx, mut rx) = tauri::async_runtime::channel(1);
    dialog.pick_file(move |path| {
        let _ = tx.try_send(path);
    });
    rx.recv().await.unwrap_or(None)
}
```

### ベトナム語の重ね記号がずれる

ベトナム語には「ế」「ố」のように記号を 2 つ重ねる文字があります。フォント指定を `-apple-system` だけにしていたところ、Chromium 系の WebView（Windows の WebView2 など）で記号が文字から離れて表示されました。macOS の WebKit では問題が出ないので、気づくのが遅れました。

`BlinkMacSystemFont` をスタックに加え、CSS と canvas（ガントとネットワーク図）で同じ定義を使うようにして解決しています。

```css
--font-ui: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif;
```

## インストール

[GitHub の Releases](https://github.com/hoanghainh1188/project-decks/releases) から入手できます。macOS は Apple Silicon 用の `.dmg`、Windows は `.msi` または `-setup.exe`、Linux は `.AppImage` / `.deb` / `.rpm` です。

:::note warn
インストーラーにはまだコード署名がありません。macOS では「アプリケーション」フォルダーの ProjectDesk を右クリックして「開く」を選んでください。それでも開けない場合は次を実行します。

```bash
xattr -dr com.apple.quarantine /Applications/ProjectDesk.app
```

Windows では SmartScreen で「詳細情報」→「実行」を選びます。
:::

## 現時点での制限

- 画面はベトナム語と英語のみ（日本語は未対応）
- `.mpp` は読めない（MSPDI で交換）
- 未署名、macOS は Apple Silicon のみ
- 1 人・1 ウィンドウ、ダーク モード未対応

## おわりに

不具合の報告や要望は [GitHub の Issues](https://github.com/hoanghainh1188/project-decks/issues) にお願いします。設計についての質問やツッコミも歓迎です。

- リポジトリ：https://github.com/hoanghainh1188/project-decks

*Microsoft Project は Microsoft Corporation の商標です。ProjectDesk は Microsoft とは関係のない独立したソフトウェアです。*
