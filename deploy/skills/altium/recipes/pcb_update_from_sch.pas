// Update the board from the schematic (Design > Import Changes, Altium's ECO) without the person: BpExpect has the hub
// work Altium's own dialogs (Component Links: match by designator; Engineering Change Order: Validate, Execute, Close).
// Altium places new footprints beside the board, in a room per sheet, with their nets; it also removes parts and nets
// the schematic no longer has. Footprints come from the libraries the project and Altium know (the symbol's footprint
// link).
// It checks itself: the schematic and the board are compared before and after (designators, footprints, pin nets, as
// sch_pcb_diff.pas does). Only an update that leaves no difference is saved (and the saved file checked); one that left
// some (a footprint no library has, a part Altium refused) fails the run with the differences and the board unsaved:
// fix the cause and run it again, or undo it in Altium (Edit > Undo takes the whole ECO back).
var
    Prj : IProject;
    SD : IServerDocument;
    Board : IPCB_Board;
    I, D0, D1, Age0, DiffN : Integer;
    Ok, ShowDiffs : Boolean;
const
    // ---- edit: placeholders ----
    PrjPath = 'C:\Projects\Board\Board.PrjPcb';
    PcbPath = 'C:\Projects\Board\Board.PcbDoc';
    SkipSinglePinNets = True;   // a net of one pin is usually not on the board (Altium's ECO option says the same)
function IsCurrent(Path : String) : Boolean;
var B : IPCB_Board;
begin
    Result := False;
    B := PCBServer.GetCurrentPCBBoard;
    if B <> nil then Result := SameText(B.FileName, Path);
end;
procedure Diff(S : String);
begin
    DiffN := DiffN + 1;
    if ShowDiffs and (DiffN <= 40) then Emit('  ' + S);
end;
// the differences between the compiled schematic and the board (Show: emit up to 40 of them)
function Differences(Show : Boolean) : Integer;
var
    Flat : IDocument; Net : INet; Pin : IPin; Cmp : IComponent;
    It : IPCB_BoardIterator; Pad : IPCB_Pad; PComp : IPCB_Component;
    SchPins, PcbPads, SchParts, PcbParts : TStringList;
    J, K : Integer; N, A, B : String;
begin
    DiffN := 0; ShowDiffs := Show;
    Result := 0;
    Prj.DM_Compile;
    Flat := Prj.DM_DocumentFlattened;
    if Flat = nil then begin Result := -1; Exit; end;
    SchPins := TStringList.Create; PcbPads := TStringList.Create; SchParts := TStringList.Create; PcbParts := TStringList.Create;
    for J := 0 to Flat.DM_ComponentCount - 1 do
    begin
        Cmp := Flat.DM_Components(J);
        SchParts.Values[Cmp.DM_PhysicalDesignator] := Cmp.DM_Footprint;
    end;
    for J := 0 to Flat.DM_NetCount - 1 do
    begin
        Net := Flat.DM_Nets(J);
        if SkipSinglePinNets and (Net.DM_PinCount < 2) then Continue;
        for K := 0 to Net.DM_PinCount - 1 do
        begin
            Pin := Net.DM_Pins(K);
            SchPins.Values[Pin.DM_PhysicalPartDesignator + '.' + Pin.DM_PinNumber] := Net.DM_NetName;
        end;
    end;
    It := Board.BoardIterator_Create;
    It.AddFilter_ObjectSet(MkSet(eComponentObject));
    It.AddFilter_LayerSet(AllLayers);
    It.AddFilter_Method(eProcessAll);
    PComp := It.FirstPCBObject;
    while PComp <> nil do
    begin
        if PComp.Name.Text <> '' then PcbParts.Values[PComp.Name.Text] := PComp.Pattern;
        PComp := It.NextPCBObject;
    end;
    Board.BoardIterator_Destroy(It);
    It := Board.BoardIterator_Create;
    It.AddFilter_ObjectSet(MkSet(ePadObject));
    It.AddFilter_LayerSet(AllLayers);
    It.AddFilter_Method(eProcessAll);
    Pad := It.FirstPCBObject;
    while Pad <> nil do
    begin
        if (Pad.Component <> nil) and (Pad.Net <> nil) then PcbPads.Values[Pad.Component.Name.Text + '.' + Pad.Name] := Pad.Net.Name;
        Pad := It.NextPCBObject;
    end;
    Board.BoardIterator_Destroy(It);
    for J := 0 to SchParts.Count - 1 do
    begin
        N := SchParts.Names[J]; A := SchParts.ValueFromIndex[J];
        if PcbParts.IndexOfName(N) < 0 then Diff('only in schematic: ' + N + ' (' + A + ')')
        else if not SameText(PcbParts.Values[N], A) then Diff('footprint ' + N + ': schematic ' + A + ', board ' + PcbParts.Values[N]);
    end;
    for J := 0 to PcbParts.Count - 1 do
        if SchParts.IndexOfName(PcbParts.Names[J]) < 0 then Diff('only on board: ' + PcbParts.Names[J] + ' (' + PcbParts.ValueFromIndex[J] + ')');
    for J := 0 to SchPins.Count - 1 do
    begin
        N := SchPins.Names[J]; A := SchPins.ValueFromIndex[J]; B := PcbPads.Values[N];
        if (PcbParts.IndexOfName(Copy(N, 1, Pos('.', N) - 1)) >= 0) and (A <> B) then
        begin
            if B = '' then B := '(no net)';
            Diff('net ' + N + ': schematic ' + A + ', board ' + B);
        end;
    end;
    SchPins.Free; PcbPads.Free; SchParts.Free; PcbParts.Free;
    Result := DiffN;
end;

Prj := GetWorkspace.DM_OpenProject(PrjPath, True);
if Prj = nil then begin BpFail('cannot open ' + PrjPath); Exit; end;
// the board must be this project's: the ECO updates the board of the project the focused document belongs to
Ok := False;
for I := 0 to Prj.DM_LogicalDocumentCount - 1 do
    if SameText(Prj.DM_LogicalDocuments(I).DM_FullPath, PcbPath) then Ok := True;
if not Ok then begin BpFail(PcbPath + ' is not a document of ' + PrjPath); Exit; end;
SD := Client.GetDocumentByPath(PcbPath);
if SD = nil then SD := Client.OpenDocument('PCB', PcbPath);
Board := PCBServer.GetPCBBoardByPath(PcbPath);
if Board = nil then begin BpFail('not open: ' + PcbPath); Exit; end;
// the ECO updates the board in front: make this one current first, and check
if not IsCurrent(PcbPath) then Client.ShowDocument(SD);
if not IsCurrent(PcbPath) then begin BpFail('could not make ' + PcbPath + ' the current board'); Exit; end;
D0 := Differences(False);
if D0 < 0 then begin BpFail('the project does not compile (no flattened netlist)'); Exit; end;
Emit('differences before: ' + IntToStr(D0));
Age0 := FileAge(PcbPath);
if D0 = 0 then Emit('the board already matches the schematic: no update needed')
else
begin
// Component Links comes only when parts are not linked yet (a first import); the ECO dialog always
    BpMayExpect('Component Links', 'click Perform Update');
    BpExpect('Engineering Change Order', 'click Validate Changes; wait 1; click Execute Changes; wait 2; click Close');
    ResetParameters;
    AddStringParameter('ObjectKind', 'Project');
    AddStringParameter('Action', 'UpdateMe');
    RunProcess('WorkspaceManager:Compare');
    D1 := Differences(True);
    Emit('differences after: ' + IntToStr(D1));
    if D1 <> 0 then
    begin
        // a partial update: what Altium could apply is on the board, unsaved; nothing is saved over the file
        BpFail('the update is partial: ' + IntToStr(D1) + ' differences remain (listed above; a footprint no library has?). The board in Altium holds the rest of the update, unsaved. Fix the cause and run this again, or undo it in Altium');
        Exit;
    end;
end;
// a board that matches: saved (and checked) if Altium holds it unsaved; until it is saved, the ECO's parts exist only in Altium's memory
if SD.Modified = False then begin Emit('nothing unsaved: ' + PcbPath); Exit; end;
if not SD.DoFileSave('') then begin BpFail('saving ' + PcbPath + ' failed'); Exit; end;
if SD.Modified <> False then begin BpFail(PcbPath + ' is still modified after saving'); Exit; end;
if FileAge(PcbPath) = Age0 then begin BpFail(PcbPath + ' did not change on disk after saving'); Exit; end;
Emit('saved ' + PcbPath);
