# Japanese UI catalog. Missing translations fail the release check.
common.cancel = キャンセル
common.delete = 削除
common.save = 保存
common.remove = 削除
thread.new = 新しいスレッド
thread.count = スレッド{n}件
files.changed = {n, plural, =0 {変更なし} other {変更ファイル#件}}
turn.by = {who, select, you {自分} agent {エージェント} other {不明}}
thread.delete.ask = 「{title}」を削除しますか？
settings.lang = 言語
settings.lang.auto = 自動
settings.lang.en = English
settings.lang.ja = 日本語

settings.agent.lang = エージェントも選択した言語で応答
settings.lang.on = オン
settings.lang.off = オフ
settings.lang.note = この端末の設定です。自動はシステムの言語に合わせます。
settings.agent.lang.note = エージェントとボットに、上の言語で応答するよう求めます。

# palette
palette.newthread = 新しいスレッド
palette.diff = 差分：このスレッドの変更
palette.terminal = ターミナル
palette.attach = ファイルを添付
palette.commit = コミットしてプッシュ
palette.pr = プルリクエストを開く
palette.revert = スレッドを元に戻す
palette.settle = スレッドを完了
palette.pin = スレッドをピン留め
palette.archive = スレッドをアーカイブ
palette.unarchive = スレッドのアーカイブを解除
palette.delete = スレッドを削除
palette.snooze.hour = 1時間スヌーズ
palette.snooze.tomorrow = 明日9:00までスヌーズ
palette.snooze.week = 1週間スヌーズ
palette.unsnooze = スヌーズを解除
palette.search = スレッドを検索
palette.findfile = ファイルを探す
palette.viewer = ビューア：基板、回路図、3D、Mech、ファイル、差分、ブラウザ
palette.kicad = KiCadで開く
palette.kicad10 = KiCad 10で開く（書き出したコピー）
palette.settings = 設定
palette.update = Backplaneを更新

# row menus
menu.snooze.hour = 1時間スヌーズ
menu.snooze.tomorrow = 明日までスヌーズ
menu.snooze.week = 1週間スヌーズ
menu.delete.confirm = もう一度クリックで削除
menu.unpin = ピン留めを解除
menu.pin = ピン留め
menu.rename = 名前を変更
menu.copylink = リンクをコピー
menu.bringback = 元に戻す
menu.archive = アーカイブ
menu.unarchive = アーカイブを解除
menu.wake = 再開
menu.settle = 完了
menu.look = 色とアイコン
menu.remove.confirm = もう一度クリックで削除
menu.project.remove = プロジェクトを削除
menu.file.open = 開く
menu.file.reveal = フォルダで表示
menu.file.copy = パスをコピー
# ja? uberbot term
menu.uber.demote = メインボットを解除
# ja? uberbot term
menu.uber.promote = メインボットにする

# settings: hub, tailscale, kicad, update
settings.tailscale.label = Tailscale：{state}
settings.tailscale.notconnected = 未接続
settings.tailscale.off.note = {listen}のみで待ち受け中です（--no-tailscaleで起動）。
settings.tailscale.reachable.note = tailnetでも利用できます。自分の端末はそのまま接続でき、それ以外はペアリングトークンが必要です。
settings.tailscale.unavailable.note = {listen}のみで待ち受け中です。Tailscaleにサインインしてから、Backplaneを再起動してください。
settings.tailscale.looking = Tailscaleを探しています。
settings.pair.none = Backplaneがtailnetで待ち受けを始めると表示されます。
settings.pair.qr = Backplaneアプリか、スマートフォンのカメラで読み取ってください。
settings.kicad.cli.missing = KiCad CLI：見つかりません
settings.kicad.cli.install = kicad.org/downloadからKiCadを、またはBackplane KiCadをインストールしてください
settings.freecad.missing = FreeCAD：見つかりません
settings.freecad.install = エージェントが機構部品のモデリングに使います。freecad.org/downloadsから入手してください
settings.kicad.ipc.missing = KiCad IPC：Backplane KiCadが必要です
settings.kicad.ipc.on = エージェントはBackplaneのヘッドレスサーバーを通じ、IPCでKiCadを編集します。
settings.kicad.ipc.install = 上のボタン（Linux x86-64）か、github.com/i2cjak/Backplane_KiCadからインストールしてください
settings.update.packaged = パッケージで更新されます。
settings.update.current = 最新です。1時間ごとに確認します。
# ja? the version row's offer, terse
settings.update.available = 更新可能：{version}
settings.update.waits = {plural, select, one {スレッド{count}件の作業が終わるまで、更新を待っています。} other {スレッド{count}件の作業が終わるまで、更新を待っています。}}

# composer
composer.steer = 誘導
composer.send = 送信
composer.stop = 停止
composer.ask = 質問
composer.give = 引き渡す

# project picker, files
proj.on = {name}（変更）
proj.exists = {dir}はすでにプロジェクトです
proj.add = {dir}をプロジェクトとして追加
proj.create = ここにプロジェクト「{name}」を作成
proj.mkdir = ここにフォルダ「{name}」を作成
proj.goto = {path}へ移動
proj.createat = プロジェクト{path}を作成
proj.addcand = {name}を追加
proj.browse = フォルダからプロジェクトを追加
fm.root = プロジェクト
btw.thinking = 考え中…

# queue, todos, mech page
queue.resume = 再開
queue.edit = 編集
queue.up = 上へ
queue.remove = 削除
queue.context = コンテキスト
queue.queued = 待機中
queue.held = 保留中
queue.handoff = {first}（次のメッセージと一緒に送られます）
todos.fewer = 表示を減らす
todos.all = すべて表示（{n}）
mech.rendering = レンダリング中…
mech.again = 再レンダリング
mech.create = レンダリングを作成
mech.nofreecad = このハブのマシンにFreeCADがありません。エージェントが部品をモデリングするのに必要です。freecad.org/downloadsから入手してください。

# notifications, machines, bots form
notify.needs = 要対応
notify.needs.note = 承認、質問、操作があるまでエージェントが止まっている事柄です。
notify.settled = 完了
notify.settled.note = スレッドに任せた作業が完了・失敗した、またはブランチがマージされたときです。作業ごとに1回。
notify.reminders = リマインダー
notify.reminders.note = 予約したリマインダーとダイジェストです。
notify.all = すべて
notify.all.note = これまでどおり、すべてのターンの終了とルームのやり取りも通知します。
notify.title = 通知する内容
notify.lead = スマートフォンとデスクトップの通知です。それ以外はアプリ内で未読のままになります。
machines.away = 不在：応答があるまで、プロジェクトは一覧に残り、読むことができます。
machines.sync = 同期済みです。プロジェクトとスレッドは変更のたびにここに表示されます。
machines.behind = {plural, select, one {同期中：プロジェクト{count}件を読み込み中です。残りは準備できています。} other {同期中：プロジェクト{count}件を読み込み中です。残りは準備できています。}}
machines.off = オフ。同期しません。
machines.linking = 接続中です。1～2分ほどでプロジェクトがここに表示されます。
machines.title = 自分のマシン
machines.lead.none = Tailscale上でBackplaneを動かしている他のマシンがここに表示されます。プロジェクトとスレッドは自動でこのサイドバーに同期されます。
machines.lead = これらのマシンのプロジェクトとスレッドは裏でこのサイドバーに同期されるので、切り替える必要はありません。分けておきたいマシンはオフにしてください。
machines.self = このマシン
# ja? uberbot term
bots.name.uber = メインボットの名前
bots.name = 名前

# approvals, api keys
ask.send.draft = この下書きを送信しますか？
ask.send.email = このメールを送信しますか？
ask.event.create = この予定を作成しますか？
ask.event.change = この予定を変更しますか？
ask.event.delete = この予定を削除しますか？
ask.allow = {tool}を許可しますか？
ask.plan = 提案されたプラン
ask.input = エージェントからの質問
apikeys.none = エージェントが使うサービスのキーを、このマシンのシークレットに保管します。エージェントはキーを読まずに使えます。まだありません。
apikeys.kept = エージェントはキーを読まずに使います：{n}件保管中。
apikeys.form = エージェントはキーを名前（例：OPENAI_API_KEY）で見つけます。キーは二度と表示されません。

# browser teaching
teach.watching = {name}が見て学習しています
teach.save = このタスクを保存
teach.youhave = ブラウザを操作中です
teach.teach = タスクを教える
teach.takeover = 引き継ぐ
teach.stopsave = 停止して保存
teach.discard = 破棄
teach.handback = 引き渡す
teach.choice.confirm = 確認
teach.choice.continue = 続ける
teach.choice.skip = このステップをスキップ
teach.choice.stop = タスクを停止

# time zone, navigation, tour
tz.source.set = 設定済み
tz.source.phone = スマートフォンから
tz.source.machine = このマシン
tz.source.none = まだ報告がありません
tz.invalid = タイムゾーン名ではありません（例：Europe/Helsinki）
nav.task.none = このタスクにはまだビューがありません
nav.pr.norepo = PRリンクにはリポジトリが必要です
# ja? uberbot term
tour.uber.first = {name}を自分のメインボットにする
tour.uber.make = {name}にする
# ja? uberbot term
tour.uber.create = 自分のメインボットを作成
tour.uber.new = 新しく作成

# first-run tour
tour.title.welcome = Backplaneへようこそ
tour.title.projects = プロジェクト
tour.title.threads = スレッド
tour.title.buttons = スレッドのボタン
tour.title.bots = ボット
tour.title.agents = エージェント
tour.title.keys = キーと音声
tour.title.machines = 他のマシン
tour.title.phones = スマートフォンとブラウザ
# ja? uberbot term
tour.title.uber = メインボット
tour.body.welcome.phone = どこに何があるかの簡単なツアーです。いつでもスキップできます。「その他」の「設定」からもう一度表示できます。
tour.body.welcome.web = どこに何があるかの簡単なツアーです。Enterで次へ進み（ボタンにフォーカスがあるときはそのボタンを押します）、Escでスキップできます。設定からもう一度表示できます。
tour.body.welcome.window = どこに何があるかの簡単なツアーです。Enterで次へ進み、Escでスキップできます。設定からもう一度表示できます。
tour.body.projects.phone = プロジェクトはフォルダで、たいていはKiCadプロジェクトです。プロジェクト一覧のフォルダ追加ボタンで追加します。スクラッチはプロジェクトなしのチャットを置く場所で、一時的なものです。{days}日間放置したスレッドは、ピン留めされていない限りフォルダごと削除されます。
tour.body.projects.desktop = プロジェクトはフォルダで、たいていはKiCadプロジェクトです。サイドバー上部の＋で追加します。スクラッチはプロジェクトなしのチャットを置く場所で、一時的なものです。{days}日間放置したスレッドは、ピン留めされていない限りフォルダごと削除されます。
tour.body.threads = スレッドは、プロジェクトで作業するエージェントとの会話です。Gitプロジェクトでは既定で別々のワークツリーを使い、Gitがない場合やローカルモードでは、スレッドはプロジェクトのフォルダを共有します。ツールの呼び出しは1行にまとめられ、委任した作業はサブエージェントの下に表示されます。
tour.body.buttons.phone = スレッドのツールバーメニューで、「完了」は進行中の一覧から外し、「スヌーズ」は後まで隠し、「フォーク」はそこから新しいスレッドを始めます。ビューアには基板、回路図、3D、ファイルが表示されます。ピン留め、アーカイブ、削除もここにあります。
tour.body.buttons.desktop = 「完了」はスレッドを進行中の一覧から外し、「スヌーズ」は後まで隠し、「フォーク」はこのスレッドから新しいスレッドを始めます。「ビューア」は基板、回路図、3D、ファイルを横に開きます。ピン留め、アーカイブ、削除もここにあります。
tour.body.bots.phone = ボットは、チャットをまたいで残るエージェントで、名前、猫、記憶を持ちます。一覧の「ボット」から追加し、メッセージで呼び出します：@scout BOMを見て
tour.body.bots.desktop = ボットは、チャットをまたいで残るエージェントで、名前、猫、記憶を持ちます。サイドバーの「ボット」から追加し、メッセージで呼び出します：@scout BOMを見て
tour.body.agents = Backplaneは、Claude Code、Codex、Grok、Muse Code、oh-my-piのコマンドラインプログラムを実行します。このハブを動かすマシンにいずれかをインストールし、サインインしてください。設定には各プログラムが見つかったかどうかが表示されますが、サインイン状態は表示されません。GoogleのAntigravityエージェントは設定からインストールとサインインができます。プロバイダーとモデルは、スレッドの入力欄で、新しいスレッド用には設定で選びます。
tour.body.keys.phone = 他のサービスのAPIキーは、ハブのマシン上のファイルに保管され、そこで動くエージェントなら読める可能性があります。出力しないよう指示してありますが、強制はされません。音声入力のOpenAIキーは、コンピューターまたはウェブページの設定で入力します。
tour.body.keys.desktop = 他のサービスのAPIキーは、ハブのマシン上のファイルに保管され、そこで動くエージェントなら読める可能性があります。出力しないよう指示してありますが、強制はされません。音声入力にはOpenAIキーが必要で、設定の「音声」で入力します。
tour.body.machines.phone = ホームフォルダごとに1つのハブが動きます。コンピューター上のハブはTailscale経由で互いを見つけ、プロジェクトとスレッドを裏で同期するので、どのハブからもすべて見えます。読み込み中のプロジェクトは薄く表示され、「同期中」と出ます。メッセージで>desktop:と書くと、desktopというマシンのスレッドを指します。「ハブ」ボタンには、このスマートフォンがペアリングしたハブが並びます。このスマートフォンも同じtailnetのTailscaleが必要です。
tour.body.machines.desktop = ホームフォルダごとに1つのハブが動きます。Backplaneの起動時にTailscaleにサインインしていれば、ハブはtailnetで待ち受け、2分おきにオンラインの他のマシンでBackplaneが動いているか確認します。Tailscaleが自分のマシンであることを保証するので、ハブは自動でリンクし、プロジェクトとスレッドを裏でこのサイドバーに同期します。切り替えは不要です。読み込み中のプロジェクトは薄く表示され、「同期中」と出ますが、残りはその間も使えます。メッセージで>desktop:と書くと、desktopというマシンのスレッドを指します。設定の「自分のマシン」には各マシンが表示され、マシンごとに同期をオフにできます。設定にはTailscaleの状態も表示されます：オン、未接続（サインインしてからBackplaneを再起動）、オフ（--no-tailscaleで起動）。
tour.body.phones.phone = ハブのコンピューターで、設定の「ペアリングリンク」から「QRコード」を開きます。こちらでは「ハブ」の「QRコードをスキャン」でペアリングします。このスマートフォンも同じtailnetのTailscaleが必要です。ブラウザでもハブのウェブアドレスを開けます。
tour.body.phones.desktop = スマートフォンをペアリングするには、Tailscaleをインストールして同じtailnetに参加します。次に設定の「ペアリングリンク」から「QRコード」を開き、Backplaneアプリかスマートフォンのカメラで読み取ります。リンクがあるときは設定に表示されます。tailnet上の端末のブラウザでも、このハブのウェブアドレスを開けます。
# ja? uberbot term
tour.body.uber.have.phone = {name}が自分のメインボットです。一覧の先頭でタップすると、すべてのステータスを確認できます。
# ja? uberbot term
tour.body.uber.have.desktop = {name}が自分のメインボットです。サイドバーの先頭でクリックすると、すべてのステータスを確認できます。
# ja? uberbot term
tour.body.uber.join = 別のマシンにメインボット「{name}」があります。
# ja? uberbot term
tour.body.uber.pick = メインボットは最初に話しかける猫です。すべてのプロジェクト、スレッド、ボット、マシンを把握し、「ステータス」タブで実行中のものや対応待ちのものを確認できます。いずれかのボットをメインボットにしてください。{name}が一番よく使われています。あとから変更できます。
# ja? uberbot term
tour.body.uber.none = メインボットは最初に話しかける猫です。すべてのプロジェクト、スレッド、ボット、マシンを把握し、「ステータス」タブで実行中のものや対応待ちのものを確認できます。今作成しましょう。名前はあとから変更できます。
tour.next = 次へ
tour.done = 完了
tour.counter = {n}／{total}
tour.back = 戻る
tour.skip = スキップ

# status plane
plane.lock.outside = ハブ外のpid {pid}（{cmd}）が保持
plane.lock.by = {title}が保持（{ago}）
plane.lock.row = {name} · {who} · 待機{n}件
plane.deploy.title = 再起動待ち
plane.deploy.sub = {line} · キャンセル
plane.machine.other = 別のマシン
plane.count.need = {n}件が要対応
plane.count.run = {n}件が実行中
plane.home.line = ホーム：{name}
plane.home.make = ここをホームにする

# routines
sched.zone.machine = このマシンの時刻
sched.zone.you = 自分の時刻
sched.kind.reminder = リマインダー
sched.kind.once = 1回
sched.kind.repeat = 繰り返し
sched.done = 完了 · {when}
sched.hint.next = {text} · 次回{when}
sched.hint.once = 1回 · {when}
sched.help.time = 読み取れる形式：2026-10-14T09:00+03:00、2026-10-14 09:00、in 20m、in 1h30m、in 3d、today 17:30、tomorrow 9am、monday 09:00、9am。

# folded tool runs and turns
fold.calls = {plural, select, one {ツール呼び出し{count}件} other {ツール呼び出し{count}件}}
fold.steps = {plural, select, one {設計変更{count}件} other {設計変更{count}件}}
fold.more = 、…
fold.worked = {took}作業 · {ago}
# ja? reads "たった今から" when the turn just started
fold.working = 作業中 · {ago}から

# bot tabs and moods
bots.tab.chat = チャット
bots.tab.space = スペース
bots.tab.browser = ブラウザ
bots.tab.tasks = タスク
bots.tab.memory = メモリ
bots.tab.routines = ルーティン
bots.tab.hooks = Webhook
bots.tab.status = ステータス
bots.tab.settings = 設定
bots.mood.sleep = 休止中
bots.mood.idle = 待機中
bots.mood.work = 作業中
bots.mood.think = 考え中
bots.mood.wait = 対応待ち
bots.mood.talk = 会話中
bots.mood.error = 失敗
bots.mood.away = 不在

# history bar and compare menu
history.step = 変更{step}
history.turn = ターン{turn}
history.branchpoint = 分岐点 · {branch}
history.workingcopy = 作業コピー
history.workingcopy.note = 現在のファイル
history.ref.head = チェックアウト中のコミット
history.ref.branch = ブランチ
history.ref.turn = このスレッドのターン終了時
history.ref.tag = タグ
history.ref.base = このブランチの分岐点
history.ref.commit = コミット
history.ref.start = このスレッドの開始前
history.notinrepo = このファイルはリポジトリにないため、比較できません。
history.live.note = 現在のファイルです。コミット、ブランチ、タグと比較できます。
history.live.ticks = 現在のファイルです。目盛りはエージェントの変更です。再生すると順に表示されます。
history.position = 変更{step}/{total}
history.position.first = 変更{step}/{total} · エージェントの編集前
history.comparing = 比較中
history.live = 現在
history.live.steps = {plural, select, one {現在 · 変更{count}件} other {現在 · 変更{count}件}}

# review panel
review.bom = BOM
review.checks = チェック
review.comments = コメント
review.release = リリース
review.bom.change = BOMの変更 · {old} → {new}
review.bom.working = BOM · 作業コピー
review.checks.of = チェック · {version}
review.comments.open = コメント · 未解決{count}件
review.tag = タグ{tag}
review.reading = 設計を読み込み中…
review.comments.none = コメントはまだありません。基板や回路図の場所を右クリック（スマートフォンでは長押し）すると、そこにコメントできます。
review.release.note = この設計にタグを付け、製造データ（ガーバー、ドリル、CPL、BOM、PDF、STEP）を.backplane/releasesに作成します。
review.empty = まだ何もありません。

# antigravity and google sign-in
agy.installed = Google純正のエージェント、Antigravity {version}をインストール済みです。Backplaneが実行し、サインインはエージェント自身が行います。
agy.old = Antigravity {old}がインストールされています。Backplaneは現在{version}に固定しています。
agy.download = 未インストールです。BackplaneがGoogleのエージェント（agy_acp_server {version}、ACPレジストリから数百MB）をダウンロードし、固定されたSHA-256を確認します。
agy.unsupported = このマシン向けのAntigravityはGoogleから提供されていません。
agy.install = インストール
agy.update = {version}に更新
agy.reinstall = 再インストール
agy.method = サインイン方法
agy.key.saved = APIキーに保存済みです。確認すると、使えるモデルが一覧表示されます。
agy.key.unsaved = まだ保存されていません。このマシンのAPIキーファイルに保存され、どのクライアントにも再表示されません。
agy.key.paste = キーを貼り付け
agy.key.save = キーを保存
agy.key.check = 確認
agy.gcp = Google Cloudプロジェクト
agy.gcp.unset = 未設定です。
agy.gcp.none = 未設定
agy.gcp.set = プロジェクト{project}、ロケーション{location}。
agy.gcp.project = プロジェクトID
agy.gcp.location = ロケーション（例：us-central1）
agy.account = アカウント
agy.account.signedin = {account}：サインイン済みです。サインイン情報はエージェントが保持し、Backplaneからは見えません。
agy.account.signedout = {account}：サインインしていません。
agy.account.other = 別のアカウント名
agy.account.add = アカウントを追加
agy.google.signin = Googleでサインイン
agy.signout = サインアウト
agy.signin = サインイン
agy.signin.back = Googleから移動した先のアドレスを貼り付け
agy.signin.open = Googleのサインインを開く
agy.signin.next = 続行
agy.signin.failed = サインインに失敗しました

# btw
btw.none = 回答がありませんでした（質問先のモデルは設定の「文章生成」で選べます）。

# routine schedules
sched.every.minute = 毎分
sched.every.minutes = {n}分ごと
sched.every.hour = 毎時
sched.every.hours = {n}時間ごと
sched.hourly = 毎時
sched.hourly.at = 毎時{minute}分
sched.daily = 毎日{time}
sched.weekdays = 平日{time}

# message times
stamp.finished = {clock}に完了
stamp.finished.took = {clock}に完了 · {took}

# subagent states
state.running = 実行中
state.starting = 起動中
state.done = 完了
state.failed = 失敗
state.cancelled = キャンセル済み
state.stopped = 停止
state.idle = 待機中
state.needsyou = 対応待ち
state.reporting = 報告中
state.waiting = 待機中
state.waitingon = 待機中
state.held = 保持中

# phone project picker
picker.hint.path = ~/パス、または絞り込みや作成に使う名前
picker.hint.find = プロジェクト、またはフォルダを追加する~/パス

# browser teaching steps
teach.step.navigate = {target}に移動
teach.step.click = {target}をクリック
teach.step.typein = {target}に入力
teach.step.select = {target}で選択
teach.step.press = {target}を押す
teach.step.scroll = {target}をスクロール
teach.step.wait = {target}を待つ
teach.step.check = {target}を確認

# phone sheets
sheet.delete = スレッドを削除しますか？
sheet.rename = スレッド名を変更

# phone sheets (projects)
sheet.remove = 「{name}」を一覧から削除しますか？
sheet.remove.note = スレッドは残ります。フォルダをもう一度追加すると元に戻ります。

# ---- i18n window ----
common.add = 追加
common.copy = コピー
common.close = 閉じる
common.create = 作成
common.refresh = 更新
common.set = 設定
common.run = 実行
common.hide = 隠す
common.prev = 前へ
common.more = さらに表示
common.earlier = 前を表示
common.key = キー
common.reload = 再読み込み
common.go = 移動
common.finish = 完了
sidebar.snoozed = スヌーズ中
sidebar.archived = アーカイブ済み
sidebar.nothing = 実行中の作業なし
sidebar.search.projects = プロジェクトを検索
thread.title.placeholder = スレッド名
# ja? sigils and their English kinds kept, as typed; A translated the words after them
composer.placeholder = 何でも質問：@bot、>thread、%project、#room、$skill、/btw
thread.subagents = サブエージェント
approval.implement = 実装
approval.dismiss = 閉じる
approval.draft = 下書きで回答
approval.allow = 許可
approval.session = セッション中許可
approval.deny = 拒否
empty.add = ＋でプロジェクトを追加してください。
empty.thread = スレッドを選ぶか、プロジェクト横の＋で作成してください。
settings.kicad.install = Backplane KiCadを導入
settings.voice = 音声
settings.voice.key = OpenAI APIキー
settings.voice.microphone = マイク
settings.voice.dictionary = 辞書
settings.voice.dictionary.note = 名前や専門用語。音声入力後に修正した単語も追加されます。
settings.voice.term = 用語を追加
settings.voice.pebble = PebbleリングのWebhookで録音を送ると、この辞書で文字起こしします。
settings.restart = 再起動後に続行
settings.restart.note = 再起動で中断したターンを自動で続行します。
settings.keys.empty = APIキーなし
settings.keys.add = APIキーを追加
settings.keys.view = APIキーを表示
settings.keys = APIキー
settings.usage = 使用量
settings.zone = タイムゾーン
settings.zone.placeholder = 例：Europe/Helsinki
settings.update.install = 更新をインストール
settings.update.check = 今すぐ確認
settings.update.now = 今すぐ更新
settings.pair = ペアリングリンク
settings.pair.code = QRコード
settings.sync = 同期
settings.tour = ツアー
settings.tour.welcome = 使い方ツアー
settings.tour.show = ツアーを表示
settings.tour.note = プロジェクト、スレッド、ボット、エージェント、マシンを紹介します。
settings.settle = 自動完了まで
settings.settle.note = この期間動きのないスレッドを完了済みに移動します。内容は保存されます。
settings.agents = エージェント
settings.agents.default = 新規スレッドのプロバイダーとモデル
settings.writing = 文章生成
settings.writing.model = タイトル・ブランチ名・コミット文のモデル
settings.writing.note = 小さなモデルで費用を抑えます。最初のメッセージからスレッド名を付けます。
settings.cad = KiCadとFreeCAD
settings.appearance = 外観
settings.theme = テーマ
settings.theme.note = ウィンドウとWebに適用します。未設定のスマートフォンは端末の設定に従います。
settings.theme.light = ライト
settings.theme.dark = ダーク
settings.sidebar = スレッド一覧
settings.sidebar.note = プロジェクト別、または対応待ちを優先した実行状況別に表示します。
settings.sidebar.active = 進行中
settings.network = ネットワークと更新
settings.channel = 更新チャンネル
settings.channel.note = 安定版はリリースのみ、ナイトリーは日次ビルドも受け取ります。
settings.channel.nightly = ナイトリー
settings.channel.stable = 安定版
viewer.mention = チャットで参照
viewer.sheet.open = シートを開く
viewer.help.scroll = ホイールでスクロール
viewer.help.browser = このスレッドのブラウザをライブ表示
viewer.help.solid = ドラッグで回転・Ctrlで移動・Shiftで拡大・Altでロール・1–7で視点・Fで全体表示
viewer.help.board = クリックで詳細・ドラッグで移動・ホイールで拡大・Fで全体表示
viewer.diff.pr = PRを作成
viewer.diff.empty = スレッド開始後の変更はありません。
viewer.board = 基板
viewer.schematic = 回路図
viewer.mech = Mech
viewer.diff = 差分
viewer.files = ファイル
viewer.review = レビュー
viewer.layers.all = すべて
viewer.layers.dim = 薄く
viewer.layers.only = 選択のみ
viewer.layers.reset = レイヤーをリセット
viewer.layers = レイヤー
viewer.page.loading = ページを読み込み中…
viewer.page.empty = エージェントが開いたページをここに表示します。
viewer.mech.loading = パーツを読み込み中…
viewer.mech.renders = レンダー
viewer.history.changes = 変更
viewer.history.compare = 比較
viewer.history.removed = 削除
viewer.history.changed = 変更
viewer.history.added = 追加
viewer.history.pick = 選択…
viewer.history.old = 旧
viewer.history.new = 新
viewer.history.ref = コミット・ブランチ・タグ
viewer.history.compare.new = 比較先（新）
viewer.history.compare.old = 比較元（旧）
viewer.files.empty = 空のフォルダ
viewer.files.loading = フォルダを読み込み中…
viewer.loading = 読み込み中…
viewer.picture.failed = この画像は表示できません
viewer.files.back = ‹ ファイル
review.reply = 返信
review.reopen = 再開
review.resolve = 解決済みにする
review.here = ここにコメント
review.note = メモ
review.passes = 合格
review.publish = GitHubで公開
review.force = チェック失敗時もリリース
palette.search.full = スレッドとメッセージを検索
palette.file.placeholder = 参照するファイルを検索
palette.command.placeholder = コマンドまたはスレッドを入力
picker.folder.placeholder = 検索・新規フォルダ名・パス
picker.project.placeholder = プロジェクト名またはパス
picker.keys.folder = Enterで選択　Tabで開く　Backspaceで上へ　Escで一覧
picker.keys.project = Enterで選択　Escで閉じる
thread.sending = 送信中…
composer.attach = 添付
composer.attach.placeholder = ファイルのパス（Enterで添付、Escで閉じる）
voice.starting = マイクを起動中…
voice.finishing = 処理中…
voice.fix = 修正…
voice.dictionary.add = 辞書に追加
voice.microphone = マイク
voice.spelling = 正しい表記（辞書に追加）
bots.persona.placeholder = 性格や役割
bots.room.name = ルーム名
bots.rooms = ルーム
bots.nothing = 対応待ちなし
bots.memory.forget = 忘れる
bots.memory.empty = 記憶はまだありません。学んだ好みや事実、手順などをここに残します。
routine.run = 今すぐ実行
routine.change = ルーティンを変更
routine.new = 新しいルーティン
# ja? the time examples stay English: they are the syntax the field parses
routine.time.placeholder = 時刻：daily 09:30、in 2h、tomorrow 9am
routine.prompt = 毎回行うこと
routine.add = ルーティンを追加
# ja? 取り消す for revoking a token or a peer link; 無効にする is the alternative
hooks.revoke = 取り消す
hooks.token = トークン
hooks.token.new = 新しいトークン
hooks.off = オフにする
hooks.post = 送信先
hooks.post.http = 送信先（HTTP。リングにはHTTPSが必要：tailscale serveを参照）
hooks.pebble.header = Pebbleアプリのヘッダー（Index 01設定のWebhook）。Transcriptionを送信
hooks.bearer = Bearerトークン
hooks.new = 新しいWebhook
hooks.note = 署名付きPOSTまたはBearerトークン付きPOSTでターンを開始します。本文は指示ではなくデータとして渡します。
hooks.name = 名前（例：GitHubプッシュ）
bots.delete = ボットを削除
bots.machines = マシン
bots.machines.note = リンクしたハブのボットと会話できます。招待でハブをリンクします。
bots.invite.address = 相手から接続できるこのハブのアドレス
bots.invite.make = 招待を作成
bots.invite.paste = 招待を貼り付け（bp1:…）
bots.invite.join = 参加
bots.invite.once = 招待は一度だけ表示します。相手のハブに貼り付けてください。
google.client.id = クライアントID（デスクトップアプリ）
google.client.secret = クライアントシークレット
google.client.save = クライアントを保存
google.signature = 署名
google.connect = Googleに接続
google.note = 接続してアクセスを許可すると、ボットがGmailとカレンダーを使えます。別のアカウントも追加できます。
google.client.own = 独自クライアント
google.callback = 別端末のブラウザの場合は移動先URLを貼り付け
# ja? "New face" changes the cat's look
bots.face.new = 見た目を変更
bots.persona = 性格・役割
bots.persona.example = 役に立つ猫
browser.address = アドレス
teach.note.placeholder = 注意点（例：2番目のタブを使う）
teach.note.add = メモを追加
teach.name.note = 名前を付けてください。毎回入力する値を指定できます。
teach.description = 処理内容（任意）
teach.runs = 最近の実行
teach.empty = タスクはまだありません。ブラウザタブかチャットで教えてください。
teach.run.note = 開いて実行できます。ルーティンからも実行できます。
thread.viewer = ビューア
room.delete = ルームを削除
room.empty = 投稿はまだありません。投稿はルームの全ボットに届きます。
room.write = ルームに投稿
plane.need = 対応待ち
plane.running = 実行中
plane.bots.working = 稼働ボット
plane.rooms.unread = 未読ルーム
plane.heading.need = 対応待ち
plane.heading.running = 実行中
plane.heading.locks = ロック
plane.heading.bots = ボット
plane.heading.rooms = ルーム
plane.heading.routines = ルーティン
image.loading = 読み込み中…
image.failed = 表示できません
# ja? the folded older projects row
sidebar.older = 以前のプロジェクト{count}件
sidebar.settled.count = 完了済み{count}件
thread.from = ← {title}から
image.loading.path = 読み込み中：{path}
image.failed.path = {path}を表示できません：{error}
pane.range = ・{count}件中{first}–{last}件
teach.paused = タスク一時停止：{task}（ステップ{step}）
queue.more = 　ほか{count}件
settings.kicad.automatic = 自動・{path}
settings.settle.days = {plural, select, other {{count}日}}
settings.model.default = 既定：{label}
settings.provider.missing = {provider}は未インストール
settings.kicad.available = Backplane KiCad {version}、IPC利用可能
settings.voice.key.note = GPT-Live-Transcribe用。このマシンに保存し、再表示しません。
settings.voice.key.saved = 保存済み：{key}。変更するには新しいキーを貼り付けてください。
settings.voice.microphone.note = 入力欄のマイクボタンで、このマシンの{device}を録音します。
settings.voice.default.mic = 既定のマイク{kind, select, none {} other {（{device}）}}
settings.agents.note = {status}。スレッドごとに保持します。入力欄で変更できます。
viewer.terminal.title = ターミナル{kind, select, none {} other {・{title}}}
viewer.layers.active = レイヤー・{layer}
viewer.render.failed = レンダー失敗：{error}
viewer.pdf.drawing = {page}ページを描画中…
viewer.pdf.page = {kind, select, unknown {{page}ページ} other {{total}ページ中{page}ページ}}
review.fails = 不合格：{reason}
review.release.making = リリース{tag}を作成中…
review.release.make = リリース{tag}を作成
terminal.pinned = {title}・上部に固定
terminal.inactive = {title}・停止中、クリックで再開
thread.reminder = リマインダー　{text}・{time}
voice.listening = 録音中{kind, select, none {} other {・{device}}}
voice.noaudio = {device}から音声が届いていません。接続を確認するか、設定の音声で別のマイクを選んでください。
voice.silent = {device}からの音声が無音です。ミュートやマイクの選択を確認してください。
voice.choose = {word}を使う
voice.heard = 「{word}」と認識{kind, select, many {（{count}件中1件目）} other {}}
voice.error = 音声入力：{error}
bots.memory.count = 記憶{count}件。好みなどの記憶は毎ターン渡されます。
routine.next = 次回{time}{relative, select, none {} other {・{rel}}}{zoned, select, none {} other {・{zone}}}
routine.empty = ルーティンはまだありません。定期または一度だけ、指定時刻（{zone}）に指示を渡します。
routine.note = 時刻は{zone}です。ハブ停止中の分も再開後に一度だけ実行します。
hooks.status = POST /hook/{id}・{state, select, revoked {失効済み} never {呼び出しなし} other {{count}回、最終：{age}}}
hooks.secret = /hook/{id}のシークレット。一度だけ表示します。HMAC-SHA256で署名してください。
bots.delete.named = {name}を削除
bots.persona.note = {name}の性格や動作方針。毎ターン開始時に読みます。
teach.steps = {clock}・{plural, select, other {{count}ステップ}}
bots.page.empty = {name}にページを開くよう頼んでください。
bots.remote.empty = {name}にメッセージを送ると、相手のマシンから返信します。
bots.remote.write = {name}にメッセージ
plane.more = ほか{count}件
# ja? 待機中 for idle threads (nothing running, nothing needed); アイドル is the alternative
plane.idle = 待機中{count}件
common.default = 既定
voice.default = 既定
viewer.solid = 3D
viewer.sheet.title = {name}{state, select, loop {（上のシートと重複）} other {}}
review.comment.meta = {author}・{place}{state, select, resolved {・解決済み} other {}}
review.page.title = {title}{state, select, busy {・読み込み中…} other {}}
viewer.empty.board = このプロジェクトにはまだ基板がありません
viewer.empty.schematic = このプロジェクトにはまだ回路図がありません
viewer.exporting = 3Dモデルを書き出し中…
viewer.browser.empty = エージェントが開いたページをここに表示します。
viewer.mesh.failed = STEPファイルをメッシュ化できません（大きなファイルにはFreeCADが必要です）
image.remote.failed = 相手のマシンから届きませんでした
remote.cache.failed = このフォルダは保存できません
remote.reply.failed = 相手のマシンからの応答が不正です
remote.request.failed = 相手のマシンが受け付けませんでした
remote.away = 相手のマシンは未接続です
remote.address.failed = HTTPアドレスではありません
remote.timeout = 相手から40秒間応答がありません。再接続中
remote.denied = 相手のマシンに接続を拒否されました（Tailnetの所有者を確認してください）
remote.handshake.failed = 接続できません：相手から5秒以内に応答がありません
viewer.export.failed = 3D書き出し失敗：{error}
remote.request.error = 相手のマシンが受け付けませんでした：{error}
remote.unreachable = 接続できません：{host}の名前解決または接続に失敗
tools.pinned = 固定中
tools.snooze = スヌーズ
tools.forkthread = スレッドを分岐
google.signature.placeholder = — {email}に代わりBackplaneから送信
viewer.kicad10 = KiCad 10のコピー
viewer.unread = このバージョンのファイルは読み込めません。
# ja? the footer's offer, as Settings' version row words it
sidebar.update = 更新可能：{version}
sidebar.path.placeholder = /path/to/project
viewer.files.range = {count}件中{first}–{last}件
viewer.item.pad = パッド{n}
viewer.item.pin = ピン{n}
viewer.item.via = ビア
viewer.item.track = トラック
viewer.item.footprint = フットプリント
viewer.item.zone = ゾーン
viewer.item.symbol = シンボル
viewer.item.label = ラベル
viewer.item.globallabel = グローバルラベル
viewer.item.hierlabel = 階層ラベル
viewer.item.sheetpin = シートピン
viewer.item.sheet = シート
viewer.item.netclassflag = ネットクラスフラグ
viewer.field.net = ネット
viewer.field.layer = レイヤー
viewer.field.width = 幅
viewer.field.ref = 参照
viewer.field.value = 値
viewer.field.footprint = フットプリント
viewer.field.size = サイズ
viewer.field.drill = ドリル
viewer.field.ktype = タイプ
viewer.field.shape = 形状
viewer.field.name = 名前
viewer.field.symbol = シンボル
viewer.field.lib = ライブラリ
viewer.field.text = テキスト
viewer.field.sheet = シート
viewer.field.file = ファイル
viewer.field.netclass = ネットクラス
