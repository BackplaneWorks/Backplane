<p align="right">
  <a href="README.md"><img src="docs/media/readme/language-en.svg" alt="English" width="92" height="28"></a>
</p>

1. Backplaneは、エージェントによるハードウェア開発のための、完全にオープンソースのワークスペースです。
2. Backplaneは[KiCadのフォーク](https://github.com/i2cjak/Backplane_KiCad)です。
3. Backplaneは、実際の電気エンジニアが開発・保守しています。

<p align="center">
  <img src="docs/media/readme/royalblue-viewers-200.png" alt="Backplaneの3D・PCB・回路図ビューアに表示されたRoyalBlue54L">
</p>

```sh
curl -fsSL https://github.com/i2cjak/backplane-bend/releases/latest/download/install.sh | sh
backplane
```

<p align="center">
  <img src="docs/media/readme/agent-setup-200.png" alt="Backplaneのプロバイダー・モデル・推論レベルの選択メニュー">
</p>

Backplaneの本当の価値は、KiCadとFreeCADのビューアにあります。エージェントの作業を一つひとつ確認し、特定の部品をクリックして、ワークバーで参照できます。Backplaneには、CAD専用スキル一式と、LLMからの操作を支援する機能を追加したKiCadフォークが付属しています（たとえば、回路図用のIPC API）。

<p align="center">
  <img src="docs/media/readme/cad/pad-workbar-detail-200.png" alt="RoyalBlue54Lのパッドを選択し、その詳細をワークバーで参照">
</p>

<p align="center">
  <img src="docs/media/readme/cad/cm5-mech-200.png" alt="エージェントの作業と並んで、Backplaneの機械設計ビューアに表示されたケースのアセンブリ">
</p>

<p align="center">
  <img src="docs/media/readme/cad/kicad-freecad-setup-200.png" alt="KiCad IPCとFreeCAD連携の設定">
</p>

Backplaneには、エージェントハーネスを連携させるための便利な基本機能があります。プロジェクトにはスレッドがあり（なんて斬新なアイデアでしょう）、ボットと、ボット・プロジェクト・スレッドをまとめる「uberbot」があります。どのハーネスからでも、別のハーネスのサブエージェントを作成でき、すべてのハーネスがスレッド間でメッセージを送れます。

<p align="center">
  <img src="docs/media/readme/orchestration/overview.png" alt="プロジェクト・スレッド・常駐ボットを一覧できるuberbot">
</p>

<p align="center">
  <img src="docs/media/readme/orchestration/delegation.png" alt="ClaudeとGrokのサブエージェントを連携させるスレッド">
</p>

特定の仕事を担当する常駐エージェント（catbots）も付属しています。Webhookなど、期待する機能はひと通りあります。それに加えて、やり取りのための便利なメールボックスと、ボットが永続的なWebアプリを作成できる「Space」もあります。

<p align="center">
  <img src="docs/media/readme/orchestration/webhooks.png" alt="catbotのWebhooksタブ">
</p>

<p align="center">
  <img src="docs/media/readme/mailroom-200.png" alt="会話の一覧とメッセージを表示したcatbotのMailroom">
</p>

<p align="center">
  <img src="docs/media/readme/orchestration/space.png" alt="catbotのSpaceに保存されたコントロールデスク">
</p>

<p align="center">
  <img src="docs/media/readme/orchestration/space-webapp.png" alt="catbotの永続的なSpaceから配信されるWebアプリ" width="480">
</p>

## その他の機能

- スレッド・ボット・プロジェクトをつなぐ、クリック可能なリンク。

  <p align="center">
    <img src="docs/media/readme/features/link-cards-2x.png" alt="スレッド・ボット・プロジェクトへのリンクチップ">
  </p>

- `>`、`#`、`%`、`@`で、ほかのBackplane項目を参照。

  <p align="center">
    <img src="docs/media/readme/reference-menus-200.png" alt="スレッド・ルーム・プロジェクト・ボットの参照メニュー">
  </p>

- TUI用のインラインターミナル。

  <p align="center">
    <img src="docs/media/readme/features/inline-terminal-2x.png" alt="インラインターミナルで実行中のvi">
  </p>

- `/btw`。

  <p align="center">
    <img src="docs/media/readme/features/btw-autocomplete-2x.png" alt="入力欄に表示された、別の質問をするためのbtwコマンド">
  </p>

Backplaneでは、好きなエージェントを使えます。Grokでも、Codexでも、Claudeでも、Antigravityでも、Museでも、何でもどうぞ。OpenCodeにも対応しています！

## ライセンス

MIT。[LICENSE](LICENSE)をご覧ください。[KiCadフォーク](https://github.com/i2cjak/Backplane_KiCad)には、KiCadのライセンスが引き続き適用されます。STEPコンバーターには、OpenCascadeと`occt-import-js`（LGPL-2.1）が含まれています。[サードパーティーのライセンス](tools/step2glb/README.md#license)をご覧ください。

<p align="center">
  <img src="docs/media/readme/cad/cats-footer-200.png" alt="Backplaneの猫たち" width="216" height="72">
</p>
