// Bring the schematic onto the board without the ECO (pcb_update_from_sch.pas works Altium's own Update PCB and is the
// first choice; this one needs no dialog at all and takes footprints from the .PcbLib files you name): compile the project, place every
// schematic component that is not on the board yet (its footprint from the .PcbLib files listed below, in a row below
// the board for you to move), link it to its schematic part, and give every pad the net the schematic gives its pin
// (nets made as needed). Run it again after schematic changes: components already on the board are left where they
// are and only their pad nets are updated. Components whose footprint is in none of the libraries are listed and skipped.
// Then place the parts with pcb_move_component.pas and route with pcb_add_track_via.pas; Altium draws the ratsnest.
var
    Prj : IProject;
    Flat : IDocument;
    SD : IServerDocument;
    Board : IPCB_Board;
    Libs : TStringList;
    I, J, Placed, Kept, Skipped, Padded : Integer;
    X : Double;
const
    // ---- edit: placeholders ----
    PrjPath = 'C:\Projects\Board\Board.PrjPcb';
    PcbPath = 'C:\Projects\Board\Board.PcbDoc';
    PcbLibs = 'C:\Projects\Board\lib\Parts.PcbLib';   // footprint libraries, ';' between them, searched in order
    RowYmm = -10;                                     // the row new parts go in, mm from the board origin
function IsCurrent(Path : String) : Boolean;
var B : IPCB_Board;
begin
    Result := False;
    B := PCBServer.GetCurrentPCBBoard;
    if B <> nil then Result := SameText(B.FileName, Path);
end;
function FindNet(Name : String) : IPCB_Net;
var It : IPCB_BoardIterator; N : IPCB_Net;
begin
    Result := nil;
    It := Board.BoardIterator_Create;
    It.AddFilter_ObjectSet(MkSet(eNetObject));
    It.AddFilter_LayerSet(AllLayers);
    It.AddFilter_Method(eProcessAll);
    N := It.FirstPCBObject;
    while N <> nil do
    begin
        if Result = nil then if SameText(N.Name, Name) then Result := N;
        N := It.NextPCBObject;
    end;
    Board.BoardIterator_Destroy(It);
end;
function GetNet(Name : String) : IPCB_Net;
begin
    Result := FindNet(Name);
    if Result <> nil then Exit;
    Result := PCBServer.PCBObjectFactory(eNetObject, eNoDimension, eCreate_Default);
    Result.Name := Name;
    Board.AddPCBObject(Result);
    PCBServer.SendMessageToRobots(Board.I_ObjectAddress, c_Broadcast, PCBM_BoardRegisteration, Result.I_ObjectAddress);
end;
function FindFootprint(Name : String) : IPCB_LibComponent;
var K : Integer; Lib : IPCB_Library;
begin
    Result := nil;
    for K := 0 to Libs.Count - 1 do
    begin
        if Result = nil then
        begin
            if Client.GetDocumentByPath(Libs[K]) = nil then Client.OpenDocument('PCBLIB', Libs[K]);
            Lib := PCBServer.GetPCBLibraryByPath(Libs[K]);
            if Lib <> nil then Result := Lib.GetComponentByName(Name);
        end;
    end;
end;
function PlaceFootprint(C : IComponent) : Boolean;
var F : IPCB_LibComponent; Comp : IPCB_Component; GI : IPCB_GroupIterator; P : IPCB_Primitive; R : TCoordRect;
begin
    Result := False;
    F := FindFootprint(C.DM_Footprint);
    if F = nil then begin Emit('SKIP ' + C.DM_PhysicalDesignator + ': footprint ' + C.DM_Footprint + ' is in none of the libraries'); Exit; end;
    Comp := PCBServer.PCBObjectFactory(eComponentObject, eNoDimension, eCreate_Default);
    Comp.Pattern := C.DM_Footprint;
    Comp.Layer := eTopLayer;
    // a library footprint's primitives sit around its own origin (the one its library shows is moved to the library's
    // origin): start the component there, so the move below takes them all along
    Comp.x := F.x;
    Comp.y := F.y;
    GI := F.GroupIterator_Create;
    GI.AddFilter_ObjectSet(AllObjects);
    P := GI.FirstPCBObject;
    while P <> nil do begin Comp.AddPCBObject(P.Replicate); P := GI.NextPCBObject; end;
    F.GroupIterator_Destroy(GI);
    // on the board and registered first, then moved: a component moved before it is registered leaves its pads behind
    Board.AddPCBObject(Comp);
    PCBServer.SendMessageToRobots(Board.I_ObjectAddress, c_Broadcast, PCBM_BoardRegisteration, Comp.I_ObjectAddress);
    Comp.Name.Text := C.DM_PhysicalDesignator;
    Comp.Comment.Text := C.DM_Comment;
    Comp.SourceDesignator := C.DM_PhysicalDesignator;
    Comp.SourceUniqueId := '\' + C.DM_UniqueId;
    Comp.SourceLibReference := C.DM_LibraryReference;
    Comp.MoveToXY(Board.XOrigin + MMsToCoord(X), Board.YOrigin + MMsToCoord(RowYmm));
    R := Comp.BoundingRectangle;
    X := X + CoordToMMs(R.Right - R.Left) + 2;
    Emit('placed ' + C.DM_PhysicalDesignator + ' ' + C.DM_Footprint + ' at ' + BpFmt('0.##', CoordToMMs(Comp.x - Board.XOrigin)) + ',' + BpFmt('0.##', RowYmm) + ' mm');
    Result := True;
end;
function FindPad(Comp : IPCB_Component; Name : String) : IPCB_Pad;
var GI : IPCB_GroupIterator; P : IPCB_Pad;
begin
    Result := nil;
    GI := Comp.GroupIterator_Create;
    GI.AddFilter_ObjectSet(MkSet(ePadObject));
    P := GI.FirstPCBObject;
    while P <> nil do
    begin
        if Result = nil then if SameText(P.Name, Name) then Result := P;
        P := GI.NextPCBObject;
    end;
    Comp.GroupIterator_Destroy(GI);
end;
procedure SetPadNets(N : INet);
var K : Integer; Pin : IPin; Comp : IPCB_Component; Pad : IPCB_Pad; Net : IPCB_Net;
begin
    if N.DM_PinCount = 0 then Exit;
    Net := GetNet(N.DM_NetName);
    for K := 0 to N.DM_PinCount - 1 do
    begin
        Pin := N.DM_Pins(K);
        Comp := Board.GetPcbComponentByRefDes(Pin.DM_PhysicalPartDesignator);
        if Comp <> nil then
        begin
            Pad := FindPad(Comp, Pin.DM_PinNumber);
            if Pad = nil then Emit('NO PAD ' + Pin.DM_PhysicalPartDesignator + '-' + Pin.DM_PinNumber + ' for net ' + N.DM_NetName)
            else if Pad.Net <> nil then
            begin
                if Pad.Net.Name <> Net.Name then Emit('KEPT ' + Pin.DM_PhysicalPartDesignator + '-' + Pin.DM_PinNumber + ' on ' + Pad.Net.Name + ' (the schematic says ' + Net.Name + '; change it in Altium or delete the part and run again)');
            end
            else
            begin
                PCBServer.SendMessageToRobots(Pad.I_ObjectAddress, c_Broadcast, PCBM_BeginModify, c_NoEventData);
                Pad.Net := Net;
                // the net keeps a list of its members: without this the pad loses its net when the board is saved and opened again
                Net.AddPCBObject(Pad);
                PCBServer.SendMessageToRobots(Pad.I_ObjectAddress, c_Broadcast, PCBM_EndModify, c_NoEventData);
                Padded := Padded + 1;
            end;
        end;
    end;
end;
Prj := GetWorkspace.DM_OpenProject(PrjPath, True);
if Prj = nil then begin Emit('cannot open ' + PrjPath); Exit; end;
Prj.DM_Compile;
Flat := Prj.DM_DocumentFlattened;
if Flat = nil then begin Emit('the project did not compile'); Exit; end;
SD := Client.GetDocumentByPath(PcbPath);
if SD = nil then SD := Client.OpenDocument('PCB', PcbPath);
Board := PCBServer.GetPCBBoardByPath(PcbPath);
if Board = nil then begin Emit('not open: ' + PcbPath); Exit; end;
// board edits register with the undo of Altium's CURRENT board: make this one current first
if not IsCurrent(PcbPath) then Client.ShowDocument(SD);
if not IsCurrent(PcbPath) then begin Emit('could not make ' + PcbPath + ' the current board'); Exit; end;
Libs := TStringList.Create;
Libs.Delimiter := ';';
Libs.StrictDelimiter := True;
Libs.DelimitedText := PcbLibs;
X := 0; Placed := 0; Kept := 0; Skipped := 0; Padded := 0;
PCBServer.PreProcess;
for I := 0 to Flat.DM_ComponentCount - 1 do
begin
    if Board.GetPcbComponentByRefDes(Flat.DM_Components(I).DM_PhysicalDesignator) <> nil then Kept := Kept + 1
    else if PlaceFootprint(Flat.DM_Components(I)) then Placed := Placed + 1
    else Skipped := Skipped + 1;
end;
for J := 0 to Flat.DM_NetCount - 1 do SetPadNets(Flat.DM_Nets(J));
PCBServer.PostProcess;
Board.GraphicallyInvalidate;
Emit('components placed ' + IntToStr(Placed) + ', already on the board ' + IntToStr(Kept) + ', skipped ' + IntToStr(Skipped) + '; pads given nets ' + IntToStr(Padded));
// save at once: unsaved parts live only in Altium's memory
Emit('saved ' + BoolToStr(SD.DoFileSave(''), True));
