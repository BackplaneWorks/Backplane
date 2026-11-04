import Base
import ./json.bend as J

# Git has verified the linked checkout and returned the corresponding main
# project folder. An ordinary folder stays exactly where the caller put it.
def Project.root(linked: Bool, cwd: String, main: String) -> String:
  Bool.pick(String, linked, main, cwd)

def Project.absolute(+path: String) -> Bool:
  Bool.or(String.starts_with(path, "/"), Bool.or(String.starts_with(path, "\\\\"), String.starts_with(String.drop(path, 1n), ":")))

# Only absolute thread configuration entries override the main configuration.
# A relative entry copied into a worktree must never redirect discovery there.
def Project.setting(+main: J.Json, +thread: J.Json, +key: String) -> J.KV:
  +value = J.Json.get_str(thread, key, "")
  J.KV{key, J.Str{Bool.pick(String, Project.absolute(value), value, J.Json.get_str(main, key, ""))}}

def Project.config(+main: J.Json, +thread: J.Json) -> J.Json:
  J.Json.obj([Project.setting(main, thread, "project"), Project.setting(main, thread, "pcb"), Project.setting(main, thread, "schematic")])

def Project.design(+path: String) -> Bool:
  Bool.or(String.ends_with(path, ".kicad_pro"), Bool.or(String.ends_with(path, ".kicad_pcb"), String.ends_with(path, ".kicad_sch")))

# Explicit thread-copy operations and absolute configuration overrides retain
# their paths. Other KiCad paths inside the linked project map to main.
def Project.path(+copy: Bool, +cwd: String, +main: String, +path: String) -> String:
  Bool.pick(String, Bool.not(Project.design(path)), path,
    Bool.pick(String, Project.absolute(path),
      Bool.pick(String, Bool.and(Bool.not(copy), String.starts_with(path, cwd ++ "/")), main ++ String.drop(path, String.length(cwd)), path),
      Bool.pick(String, copy, cwd, main) ++ "/" ++ path))

# A deliberate main choice skips the prompt; a thread copy is opt-in only.
def Project.choose(+choice: String, changed: Bool) -> Bool:
  Bool.and(changed, Bool.not(Bool.or(String.eq(choice, "main"), String.eq(choice, "thread"))))

# The known read commands of the selected runtime. Unknown/new commands,
# document lifecycle, jobs and transaction commands require a write choice.
# Do not infer read access from a Get/List prefix alone.
def Project.reads() -> List<&2, String>:
  ["kiapi.backplane.CheckAssemblyCollisions",
   "kiapi.backplane.GetAssembly",
   "kiapi.backplane.GetBoardStructure",
   "kiapi.backplane.GetCapabilities",
   "kiapi.backplane.GetConstructionLayerState",
   "kiapi.backplane.GetViewerConstructionState",
   "kiapi.backplane.ListAssemblyComponents",
   "kiapi.backplane.ListAssemblyTabs",
   "kiapi.board.commands.CheckPadstackPresenceOnLayers",
   "kiapi.board.commands.GetActiveLayer",
   "kiapi.board.commands.GetBoardBoundingBox",
   "kiapi.board.commands.GetBoardDesignRules",
   "kiapi.board.commands.GetBoardEditorAppearanceSettings",
   "kiapi.board.commands.GetBoardEnabledLayers",
   "kiapi.board.commands.GetBoardLayerByName",
   "kiapi.board.commands.GetBoardLayerName",
   "kiapi.board.commands.GetBoardOrigin",
   "kiapi.board.commands.GetBoardPlotSettings",
   "kiapi.board.commands.GetBoardStackup",
   "kiapi.board.commands.GetConnectedItems",
   "kiapi.board.commands.GetCustomDesignRules",
   "kiapi.board.commands.GetEmbeddedFiles",
   "kiapi.board.commands.GetGraphicsDefaults",
   "kiapi.board.commands.GetItems",
   "kiapi.board.commands.GetItemsByNet",
   "kiapi.board.commands.GetItemsByNetClass",
   "kiapi.board.commands.GetNetClassForNets",
   "kiapi.board.commands.GetNets",
   "kiapi.board.commands.GetPadShapeAsPolygon",
   "kiapi.board.commands.GetVisibleLayers",
   "kiapi.common.commands.GetBoundingBox",
   "kiapi.common.commands.GetCurrentVariant",
   "kiapi.common.commands.GetDocumentModifiedState",
   "kiapi.common.commands.GetItems",
   "kiapi.common.commands.GetItemsById",
   "kiapi.common.commands.GetItemsFromLibrary",
   "kiapi.common.commands.GetKiCadBinaryPath",
   "kiapi.common.commands.GetLibraryItems",
   "kiapi.common.commands.GetLibraryStatuses",
   "kiapi.common.commands.GetLibraryTable",
   "kiapi.common.commands.GetNetClassAssignments",
   "kiapi.common.commands.GetNetClasses",
   "kiapi.common.commands.GetOpenDocuments",
   "kiapi.common.commands.GetPageSettings",
   "kiapi.common.commands.GetPaths",
   "kiapi.common.commands.GetPluginSettingsPath",
   "kiapi.common.commands.GetSelection",
   "kiapi.common.commands.GetTextAsShapes",
   "kiapi.common.commands.GetTextExtents",
   "kiapi.common.commands.GetTextVariables",
   "kiapi.common.commands.GetTitleBlockInfo",
   "kiapi.common.commands.GetVariants",
   "kiapi.common.commands.GetVersion",
   "kiapi.common.commands.Ping",
   "kiapi.schematic.commands.GetItems",
   "kiapi.schematic.commands.GetSchematicHierarchy",
   "kiapi.schematic.commands.GetSchematicNetlist"]

def Project.readonly(ty: String) -> Bool:
  List.contains(~String, ~String.eq, Project.reads(), ty)

def Project.refuse_write(linked: Bool, changed: Bool, read: Bool, explicit: Bool) -> Bool:
  Bool.and(Bool.and(linked, changed), Bool.and(Bool.not(read), Bool.not(explicit)))

def Project.write_error(+main: String, +thread: String) -> String:
  "KiCad write refused: this thread's design differs from main. Main: " ++ main ++ "; this thread: " ++ thread ++
  ". Pass thread_copy:true to write this thread's copy, or main:true to explicitly write main. Reads continue to default to main."
