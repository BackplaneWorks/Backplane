# Altium scripting reference (as verified on Altium Designer 26.11)

## The script the hub runs

Your script is split and wrapped: the leading `var`/`const`/`type` sections become globals, `procedure`/`function`
definitions follow the hub's helpers, the remaining statements run inside `try/except` (an exception comes back as
`error`, the lines already emitted stay). `Emit(S)` writes a line. `while`/`repeat` loops get a deadline guard. Open
`PreProcess` undo groups are closed at the end.

- Put `var` first, `const` after it. The hub drops the `var` keyword of your var section; anything after a `const`
  section that is not itself `const`/`type` is read as a typed constant ("Typed constants aren't supported").
- Routines may have their own `var` section. Do not use reserved words as names (`Register` broke a run: "Program name expected").
- `Exit` in the statements ends the script cleanly; `Continue` works in loops.
- Do not rely on `and`/`or` short-circuiting: guard nil in its own `if` (`if B <> nil then Result := B.FileName = P;`).
- A compile error shows Altium's "Continue execution?" box and then an Error box; the hub answers the first only, and a
  modal box left up makes every later run time out ("There is another instance ... busy"). Write scripts that compile.
- A method call on `nil` fails at run time as "Undeclared identifier: <method>".

## Documents and projects

```pascal
Prj := GetWorkspace.DM_OpenProject(PrjPath, True);   // opens or finds it; IProject
Prj.DM_LogicalDocumentCount / Prj.DM_LogicalDocuments(I)  // IDocument: DM_DocumentKind ('SCH','PCB','SCHLIB','PCBLIB','OUTPUTJOB'...), DM_FullPath
SD := Client.GetDocumentByPath(Path);                 // IServerDocument or nil when not loaded
SD := Client.OpenDocument('SCH', Path);               // loads it in the background (no tab shown); 'PCB', 'SCHLIB', 'OUTPUTJOB'
Client.ShowDocument(SD);                              // brings it to the front (changes the person's view: only when needed)
SD.Modified; SD.DoFileSave('');                       // '' = its own format; see save pitfall below
SD.Modified := False; Client.CloseDocument(SD);       // close dropping edits (only your own documents)
Sch := SchServer.GetSchDocumentByPath(Path);          // ISch_Document (also ISch_Lib for a .SchLib); needs it loaded
Board := PCBServer.GetPCBBoardByPath(Path);           // IPCB_Board; needs it loaded
Prj.DM_AddSourceDocument(Path);                       // add a file (e.g. an OutJob) to the project
```

Never use `GetWorkspace.DM_FocusedDocument`, `PCBServer.GetCurrentPCBBoard`, `SchServer.GetCurrentSchDocument` or
`Client.CurrentView` to choose what to edit: that is whatever the person has in front.

Saving: `SD.DoFileSave('')` writes the file. `SD.DoFileSave(SD.Kind)` (`'SCH'`), `SD.DoSafeChangeFileNameAndSave`,
`RunProcess('WorkspaceManager:SaveObject')` with `ObjectKind=Document|FileName=...` and
`Client.SendMessage('WorkspaceManager:SaveObject', ...)` all reported success and wrote nothing for a document loaded in
the background (the first two even clear `Modified`). After a save, compare the file's time if in doubt.

## Compiled project (netlist, ERC)

```pascal
Prj.DM_Compile;  Flat := Prj.DM_DocumentFlattened;   // nil when compile failed
Flat.DM_NetCount / DM_Nets(I): INet  .DM_NetName, .DM_PinCount, .DM_Pins(J): IPin .DM_PhysicalPartDesignator, .DM_PinNumber
Flat.DM_ComponentCount / DM_Components(I): IComponent .DM_PhysicalDesignator, .DM_Footprint
Prj.DM_ViolationCount / DM_Violations(I): IViolation .DM_ErrorLevel (0 none,1 warning,2 error,3 fatal),
    .DM_ShortDescriptorString, .DM_DetailString, .DM_RelatedObjectCount, .DM_RelatedObjects(K): IDMObject
    (.DM_ObjectKindString, .DM_LongDescriptorString, .DM_OwnerDocumentName)
```

Designators: a sheet's component says `C1`; the compiled (physical) one of a multichannel design says `C1_CH1`, and
that is what the board uses.

## Schematic objects

Units: `MilsToCoord`/`CoordToMils` (also `MMsToCoord`/`CoordToMMs`); `Point(X, Y)` builds a `TLocation`.

```pascal
It := Sch.SchIterator_Create; It.AddFilter_ObjectSet(MkSet(eSchComponent));
Obj := It.FirstSchObject; while Obj <> nil do begin ... Obj := It.NextSchObject; end;
Sch.SchIterator_Destroy(It);
// children of a component: Comp.SchIterator_Create with eParameter / eImplementation / ePin; Comp.SchIterator_Destroy(It)
```

Object kinds: `eSchComponent, eWire, eNetLabel, ePowerObject, ePort, eJunction, eNoERC, eParameter, eImplementation,
ePin, eSheetSymbol, eSheetEntry, eBus, eLabel`. `Obj.ObjectId`, `Obj.GetState_DescriptionString`.

Component: `Designator.Text`, `Comment.Text`, `LibReference`, `SourceLibraryName`, `Location`, `Orientation`
(`eRotate0/90/180/270`), `MoveByXY`, `MoveToXY`, `RotateBy90(Center, eRotate90)`. Footprint: the `eImplementation`
child with `ModelType = 'PCBLIB'` and `IsCurrent`, its `ModelName`. Wire: `VerticesCount`, `Vertex(I)` (from 1),
`InsertVertex := N` then `SetState_Vertex(N, Point)`. Net label / power port: `Text`, `Location`; power port
`Style := ePowerGndPower` (other styles exist), `ShowNetName`, `Orientation`.

Edits:

```pascal
SchServer.ProcessControl.PreProcess(Sch, '');                    // one undo group
// change: bracket each object's change
SchServer.RobotManager.SendMessage(Obj.I_ObjectAddress, c_BroadCast, SCHM_BeginModify, c_NoEventData);
Obj.Text := 'X';
SchServer.RobotManager.SendMessage(Obj.I_ObjectAddress, c_BroadCast, SCHM_EndModify, c_NoEventData);
// new: SchServer.SchObjectFactory(eWire, eCreate_Default), set it up, then
Sch.RegisterSchObjectInContainer(Obj);
SchServer.RobotManager.SendMessage(Sch.I_ObjectAddress, c_BroadCast, SCHM_PrimitiveRegistration, Obj.I_ObjectAddress);
// delete:
Sch.RemoveSchObject(Obj);
SchServer.RobotManager.SendMessage(Sch.I_ObjectAddress, c_BroadCast, SCHM_PrimitiveRegistration, Obj.I_ObjectAddress);
SchServer.ProcessControl.PostProcess(Sch, '');
Sch.GraphicallyInvalidate;
```

These messages are addressed to the document, so editing a background schematic does not touch the one in front
(checked). A library part: `Comp := SchServer.LoadComponentFromLibrary(LibRef, LibPath)` (a .SchLib or .IntLib),
register it, then `MoveToXY`; set `SourceLibraryName` (else `*`). A library's own symbols: open the .SchLib, then
`Lib.SchLibIterator_Create` with `eSchComponent`.

## Board objects

Units: `MMsToCoord`/`CoordToMMs`, `MilsToCoord`/`CoordToMils`. Coordinates are absolute; the origin the person sees is
`Board.XOrigin, Board.YOrigin`.

```pascal
It := Board.BoardIterator_Create;
It.AddFilter_ObjectSet(MkSet(eComponentObject)); It.AddFilter_LayerSet(AllLayers); It.AddFilter_Method(eProcessAll);
Obj := It.FirstPCBObject; ... It.NextPCBObject; Board.BoardIterator_Destroy(It);
SIt := Board.SpatialIterator_Create; SIt.AddFilter_ObjectSet(...); SIt.AddFilter_LayerSet(AllLayers);
SIt.AddFilter_Area(X1, Y1, X2, Y2); ... Board.SpatialIterator_Destroy(SIt);
Board.GetPcbComponentByRefDes('R1');
```

Kinds: `eComponentObject, eTrackObject, eArcObject, eViaObject, ePadObject, eFillObject, eRegionObject, ePolyObject,
eTextObject, eNetObject, eRuleObject, eViolationObject`. Layers: `eTopLayer, eBottomLayer, eMidLayer1.., eTopOverlay,
eBottomOverlay, eMechanical1..`; `Layer2String(L)`. Component: `Name.Text`, `Comment.Text`, `Pattern`, `x`, `y`,
`Rotation`, `Layer`, `MoveToXY`. Track: `X1 Y1 X2 Y2 Width Layer Net`. Via: `x y Size HoleSize LowLayer HighLayer Net`.
Pad: `Name`, `Component`, `Net`, `x`, `y`. Any primitive: `Descriptor`, `Net`, `InPolygon`, `BoundingRectangle`.

Edits:

```pascal
// 1. the board must be Altium's current board (see below)
PCBServer.PreProcess;
Obj := PCBServer.PCBObjectFactory(eTrackObject, eNoDimension, eCreate_Default);  // set it up
Board.AddPCBObject(Obj);
PCBServer.SendMessageToRobots(Board.I_ObjectAddress, c_Broadcast, PCBM_BoardRegisteration, Obj.I_ObjectAddress);
// change: PCBM_BeginModify / PCBM_EndModify to Obj.I_ObjectAddress around it
// delete: Board.RemovePCBObject(Obj) + the same PCBM_BoardRegisteration message
PCBServer.PostProcess;
Board.GraphicallyInvalidate;
```

`PCBM_BoardRegisteration` goes to the undo of `PCBServer.GetCurrentPCBBoard`, not of `Board`: with another board in
front, that board got the undo entry and was marked modified (checked with a decoy board). With the edited board
current, the edit is undoable in Altium (`PCB:Undo`, checked). Without the message the object is still added and saved,
just not undoable. So: `Client.ShowDocument(SD)` first when the board is not current, and check it became current.

## Rules and DRC

Rules: `eRuleObject` objects; `Rule.Name, RuleKind (eRule_Clearance, eRule_ShortCircuit, eRule_MaxMinWidth,
eRule_MaxMinHoleSize, eRule_HoleToHoleClearance, ...), Enabled, DRCEnabled, IsUnary, Descriptor, Scope1Includes(P),
CheckBinaryScope(P1, P2), ActualCheck(P1, P2 or nil)` -> an `IPCB_Violation` or nil (`Descriptor`, `Rule`,
`Primitive1`, `Primitive2`). `pcb_drc.pas` uses these and gave the same violations as the batch DRC for those rule kinds.
Leave a polygon's pour regions (`InPolygon` regions) out: Altium does not report them, their tracks it does.

`PCB:DesignRuleCheck` opens the modal Design Rule Checker (the script waits until it is closed; Run writes
`Project Outputs for <project>\Design Rule Check - <board>.html/.drc` and, with Create Violations, `eViolationObject`
markers on the board).

## Processes

`ResetParameters; AddStringParameter('Name', 'Value'); RunProcess('Server:Process');` runs a menu command; it acts on
the focused document unless its parameters name one. The commands behind Altium's menus are in
`<Altium>\System\*.rcs` (`Command=` and `Params=`). Ones used here:

- `WorkspaceManager:GenerateReport` `Action=Run|ObjectKind=OutputBatch|OutputMedium=<container>`: runs the focused
  OutJob's container (Gerber, NC drill, ODB into the project's output folder; ~3 s; no dialog).
- `WorkspaceManager:Print` `Action=PublishToPDF|ObjectKind=OutputBatch|OutputMedium=PDF|DisableDialog=True`: raised an
  Error box and wrote nothing in testing. An OutJob with `OpenOutput<N>=1` would open the result in another program.
- `WorkspaceManager:Compare` `ObjectKind=Project|Action=UpdateMe` (board in front; from a schematic
  `Action=UpdateOther|Index=1`): the ECO, modal.
- `PCB:DesignRuleCheck`: modal, see above.
