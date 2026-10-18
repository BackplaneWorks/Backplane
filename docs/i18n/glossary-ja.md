# Japanese UI copy

Use concise, natural Japanese. Translate the app's controls and descriptions;
keep people's messages, agent output, names, code, paths and identifiers intact.
Use polite wording for explanatory sentences, and short nouns or verbs for controls.
Avoid pronouns when the context already identifies the person or agent.

| English | Japanese | Usage |
| --- | --- | --- |
| thread | スレッド | Conversation with an agent. |
| bot | ボット | Persistent agent identity; never ロボット. |
| agent | エージェント | The model performing work. |
| you (author label) | 自分 | Not あなた in labels. |
| project | プロジェクト | Keep user-assigned project names intact. |
| room | ルーム | Shared conversation space. |
| message | メッセージ | Content posted to a thread or room. |
| inbox | 受信トレイ | Incoming messages. |
| mailroom | メール | Bot mail workspace. Provisional: confirm with the mail lead what the screen holds. |
| settings | 設定 | Both device and hub settings. |
| app settings | アプリ設定 | Settings stored on this device. |
| language | 言語 | UI locale setting. |
| automatic | 自動 | Follow the device's language. |
| appearance | 外観 | Device theme setting. |
| system | システム | Appearance follows the OS. |
| light / dark | ライト / ダーク | Theme choices. |
| home | ホーム | Tab name. |
| new thread | 新しいスレッド | Creation action. |
| open | 開く | Open a page, file, or viewer. |
| close | 閉じる | Close a view. |
| save | 保存 | Persist edits. |
| cancel | キャンセル | Cancel an operation. |
| delete | 削除 | Remove the selected item. |
| remove | 削除 / 解除 | 削除 for an item (一覧から削除 when only the list entry goes); 解除 for a link, pin or assignment. |
| copy / paste | コピー / 貼り付け | Clipboard actions. |
| send | 送信 | Post a message. |
| stop | 停止 | Stop current work. |
| retry | 再試行 | Repeat a failed operation. |
| search | 検索 | Search controls. |
| routine | ルーティン | Recurring automation. |
| notification | 通知 | OS or in-app notification. |
| permission | 権限 / 許可 / 承認 | 権限 the access itself; 許可 granting it; 承認 approving an action an agent proposes. |
| connected / disconnected | 接続済み / 未接続 | Connection state. |
| running | 実行中 | An active job or agent. |
| waiting | 待機中 | Queued or waiting for an event; 承認待ち / 回答待ち when the person must act. |
| completed / failed | 完了 / 失敗 | Job outcome. |
| just now | たった今 | Relative timestamp. |
| due now | 実行時刻です | A routine's run time has come (not a deadline); never まもなく. |
| in under a minute | まもなく | Future relative time. |

Use Japanese counters naturally: スレッド1件, ファイル2件, 3分前, 2時間後.
Use Japanese quotation marks in confirmations: 「{title}」を削除しますか？
Use Arabic digits, grouping commas, 24-hour time, and dates such as 10月15日 or
2025年10月15日. Keep KiCad, Git, GitHub, Backplane and model names as written.
Technical strings such as PR, API, URL, IME and UTF-8 stay in Latin letters.

Spacing and punctuation: no space between Japanese and Latin letters or digits in
labels (APIキー, Backplaneの設定, スレッド1件, 3分前). Sentences use 、 and 。; labels
end without 。. Quotes use 「」; prose uses full-width ？ and （）. Clocks, paths, URLs and
code keep ASCII punctuation (09:05). Relative times: 前 for the past, 後 for the future,
neither for a duration (2時間5分). Weekdays: 月 in calendar headings, 月曜 alone, （月）
beside a date.

"Turn" depends on the call site: ターン for a conversation turn, 実行 for a run.
Open questions for the product owner: "due now" (実行時刻です assumed) and mailroom
(メール assumed). Reviewed independently on 2026-10-16.
