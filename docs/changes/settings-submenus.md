Settings opens a category menu. Each page shows its title and a way back, resets
and clamps its scroll, and keeps draft fields and saved settings. KiCad path and
provider Usage shortcuts open their categories directly. Narrow native panes use
the window width. Tab/Shift+Tab select visible native controls; Enter activates;
PageUp/PageDown scroll; Escape leaves a field, then returns to the menu, then closes
Settings. Web Back restores the originating category button.

Phone hub settings use the same category state and existing shared row adapters.
Device Appearance and Notifications remain in phone App settings. Desktop-only
Voice/Writing pages are not offered as empty phone pages. Navigation never writes
settings, defaults, telemetry consent, or notification preferences.

Validation: 21 focused navigation/keyboard/retention checks, 23 existing menu
regressions, shared phone JavaScript bundle, and the full source/every-law gate
passed. Existing LAWS.bend and PROOF.bend are unchanged. The recorded check input
key in the gate runner environment is a04666bbb10938a36b55be5f7406b7fb177ca9fa. No full native app build, device app
build, install, deploy, or FPS acceptance was performed by this owner.

Fixtures: test/native/settings_frame.bend draws the actual dialog with real system
fonts at 100/150 percent; test/tools/settings_fixture.bend exports the actual web
settings tree. Native normal/narrow layouts and English/Japanese labels were
reviewed. Web category/Appearance/auth pages at 390px with the scaling owner's
150% CSS had no horizontal overflow or offscreen controls; Back restored focus.

The 150% native review requires the scaling owner's universal saturating
half-leading correction in Text.baseline. Without it, tall-font labels and notes
disappear. That exact authorized one-line fix was reviewed only in the separate
/tmp/bp-settings-font-review checkout: gfx/text.bend SHA256
9a36c0414bcdb827a13dc82d82d6bc5a3544d030bc38c626573525da6c067b12.
With it, category labels and explanatory notes are visible; English/Japanese
Appearance, CAD and auth pages fit at 800x700 without overlaps. Corrected PNGs are
in build/settings-review/native-corrected. This is a conditional component visual
pass; final composed app/Ty/cache/FPS acceptance remains with the other owners.
No renderer hunk is committed here.

Composition handoff (root owns integration):

- CEF PR #267 head 8fcd75618f4f265fe3025249ac119f5e938824a1: keep Set.browser in
  Set.page.cad, including requested engine versus actual engine and fallback reason.
  Browser panel tabs/address controls are separate from Settings.
- Telemetry d6367ae7aee70e102e07b2bb46c95372e7082b86: append a distinct Privacy
  category to Settings.pages and Settings.phone.pages. Add its title in
  Settings.label and index 12 to native Set.page.draw/Set.page.h,
  View.settings.rows.k, and Mob.settings.rows.k. Reuse Set.share at page-local y
  with Set.share.h, View.agy.rows(U.Share.rows(u)), and
  Mob.agy.rows(U.Share.rows(u)). Do not carry Set.nh/Set.share.below flat-list
  offsets. Keep share/share-reset/open-url actions and shared selected states;
  provider Usage remains separate. Do not write telemetry.asked on navigation.
- The scale owner authorized the byte-identical core/textscale.bend helper,
  SHA256 6f3179e85bce1ed2f56191bcc25621fcd83dc4ecf239b60ceed9cb1fdf883b5c.
  Appearance controls use Scale.of/options/label and the generic setting action
  for ui.text_scale. Native phones retain OS text sizing. Integrate the font/cache
  seam and the universal baseline correction once, from their owning thread.
