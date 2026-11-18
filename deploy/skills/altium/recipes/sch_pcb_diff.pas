// What 'Update PCB' would change, read-only: compares the compiled schematic (components, footprints, pin nets)
// with the board (components, patterns, pad nets) by designator. The update itself: pcb_update_from_sch.pas.
var
    Prj : IProject;
    Flat : IDocument;
    Net : INet;
    Pin : IPin;
    Cmp : IComponent;
    SD : IServerDocument;
    Board : IPCB_Board;
    It : IPCB_BoardIterator;
    Pad : IPCB_Pad;
    PComp : IPCB_Component;
    SchPins, PcbPads, SchParts, PcbParts : TStringList;
    I, J, Diffs : Integer;
    K, A, B : String;
const
    // ---- edit: placeholders ----
    PrjPath = 'C:\Projects\Board\Board.PrjPcb';
    PcbPath = 'C:\Projects\Board\Board.PcbDoc';
    SkipSinglePinNets = True;   // a net of one pin is usually not on the board (Altium's ECO option says the same)
procedure Diff(S : String);
begin
    Diffs := Diffs + 1;
    if Diffs <= 200 then Emit(S);
end;
Prj := GetWorkspace.DM_OpenProject(PrjPath, True);
Prj.DM_Compile;
Flat := Prj.DM_DocumentFlattened;
if Flat = nil then begin Emit('compile failed: no flattened netlist'); Exit; end;
SD := Client.GetDocumentByPath(PcbPath);
if SD = nil then SD := Client.OpenDocument('PCB', PcbPath);
Board := PCBServer.GetPCBBoardByPath(PcbPath);
if Board = nil then begin Emit('not open: ' + PcbPath); Exit; end;
SchPins := TStringList.Create; PcbPads := TStringList.Create; SchParts := TStringList.Create; PcbParts := TStringList.Create;
// schematic side: designator -> footprint, designator.pin -> net
for I := 0 to Flat.DM_ComponentCount - 1 do
begin
    Cmp := Flat.DM_Components(I);
    SchParts.Values[Cmp.DM_PhysicalDesignator] := Cmp.DM_Footprint;
end;
for I := 0 to Flat.DM_NetCount - 1 do
begin
    Net := Flat.DM_Nets(I);
    if SkipSinglePinNets and (Net.DM_PinCount < 2) then Continue;
    for J := 0 to Net.DM_PinCount - 1 do
    begin
        Pin := Net.DM_Pins(J);
        SchPins.Values[Pin.DM_PhysicalPartDesignator + '.' + Pin.DM_PinNumber] := Net.DM_NetName;
    end;
end;
// board side
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
Diffs := 0;
for I := 0 to SchParts.Count - 1 do
begin
    K := SchParts.Names[I]; A := SchParts.ValueFromIndex[I];
    if PcbParts.IndexOfName(K) < 0 then Diff('only in schematic: ' + K + ' (' + A + ')')
    else if not SameText(PcbParts.Values[K], A) then Diff('footprint ' + K + ': schematic ' + A + ', board ' + PcbParts.Values[K]);
end;
for I := 0 to PcbParts.Count - 1 do
    if SchParts.IndexOfName(PcbParts.Names[I]) < 0 then Diff('only on board: ' + PcbParts.Names[I] + ' (' + PcbParts.ValueFromIndex[I] + ')');
for I := 0 to SchPins.Count - 1 do
begin
    K := SchPins.Names[I]; A := SchPins.ValueFromIndex[I]; B := PcbPads.Values[K];
    if (PcbParts.IndexOfName(Copy(K, 1, Pos('.', K) - 1)) >= 0) and (A <> B) then
    begin
        if B = '' then B := '(no net)';
        Diff('net ' + K + ': schematic ' + A + ', board ' + B);
    end;
end;
for I := 0 to PcbPads.Count - 1 do
    if (SchPins.IndexOfName(PcbPads.Names[I]) < 0) and (SchParts.IndexOfName(Copy(PcbPads.Names[I], 1, Pos('.', PcbPads.Names[I]) - 1)) >= 0) then
        Diff('net ' + PcbPads.Names[I] + ': schematic (no net), board ' + PcbPads.ValueFromIndex[I]);
Emit('differences ' + IntToStr(Diffs));
SchPins.Free; PcbPads.Free; SchParts.Free; PcbParts.Free;
