// Add a polygon pour (copper on a net over a rectangle, poured at once), or repour the board's polygons after edits.
// The pour keeps the board's clearance rules and connects to same-net pads by the polygon-connect rule (thermals by
// default). Read the result with pcb_list_area.pas (its regions) or pcb_drc.pas. Board in front required (checked).
var
    SD : IServerDocument;
    Board : IPCB_Board;
    Poly : IPCB_Polygon;
    Seg : TPolySegment;
    Net : IPCB_Net;
    It : IPCB_BoardIterator;
    Polys : TList;
    I, N : Integer;
    GIt : IPCB_GroupIterator;
    Prim : IPCB_Primitive;
const
    // ---- edit: placeholders ----
    PcbPath = 'C:\Projects\Board\Board.PcbDoc';
    Mode = 'add';                    // 'add' a polygon, or 'repour' every polygon on the board
    NetName = 'GND';
    LayerName = 'bottom';            // 'top' or 'bottom'
    X1 = 0.5;                        // the pour's rectangle, mm from the board origin (inside the board outline;
    Y1 = 0.5;                        // the board-outline clearance rule keeps copper off the edge anyway)
    X2 = 29.5;
    Y2 = 24.5;
    PolyName = 'GND_BOTTOM';
function IsCurrent(Path : String) : Boolean;
var B : IPCB_Board;
begin
    Result := False;
    B := PCBServer.GetCurrentPCBBoard;
    if B <> nil then Result := SameText(B.FileName, Path);
end;
function FindNet(Name : String) : IPCB_Net;
var I2 : IPCB_BoardIterator; N2 : IPCB_Net;
begin
    Result := nil;
    I2 := Board.BoardIterator_Create;
    I2.AddFilter_ObjectSet(MkSet(eNetObject));
    I2.AddFilter_LayerSet(AllLayers);
    I2.AddFilter_Method(eProcessAll);
    N2 := I2.FirstPCBObject;
    while N2 <> nil do
    begin
        if Result = nil then if SameText(N2.Name, Name) then Result := N2;
        N2 := I2.NextPCBObject;
    end;
    Board.BoardIterator_Destroy(I2);
end;
SD := Client.GetDocumentByPath(PcbPath);
if SD = nil then SD := Client.OpenDocument('PCB', PcbPath);
Board := PCBServer.GetPCBBoardByPath(PcbPath);
if Board = nil then begin Emit('not open: ' + PcbPath); Exit; end;
if not IsCurrent(PcbPath) then Client.ShowDocument(SD);
if not IsCurrent(PcbPath) then begin Emit('could not make ' + PcbPath + ' the current board'); Exit; end;
if Mode = 'repour' then
begin
    // collect, then rebuild
    Polys := TList.Create;
    It := Board.BoardIterator_Create;
    It.AddFilter_ObjectSet(MkSet(ePolyObject));
    It.AddFilter_LayerSet(AllLayers);
    It.AddFilter_Method(eProcessAll);
    Poly := It.FirstPCBObject;
    while Poly <> nil do begin Polys.Add(Poly); Poly := It.NextPCBObject; end;
    Board.BoardIterator_Destroy(It);
    PCBServer.PreProcess;
    for I := 0 to Polys.Count - 1 do
    begin
        Poly := Polys.Items(I);
        PCBServer.SendMessageToRobots(Poly.I_ObjectAddress, c_Broadcast, PCBM_BeginModify, c_NoEventData);
        Poly.Rebuild;
        PCBServer.SendMessageToRobots(Poly.I_ObjectAddress, c_Broadcast, PCBM_EndModify, c_NoEventData);
        Emit('repoured ' + Poly.Name + ' (' + Layer2String(Poly.Layer) + ', net ' + Poly.Net.Name + ')');
    end;
    PCBServer.PostProcess;
    Board.GraphicallyInvalidate;
    Emit('polygons repoured ' + IntToStr(Polys.Count) + '; modified=' + BoolToStr(SD.Modified, True));
    Polys.Free;
    Exit;
end;
Net := FindNet(NetName);
if Net = nil then begin Emit('no net ' + NetName); Exit; end;
PCBServer.PreProcess;
Poly := PCBServer.PCBObjectFactory(ePolyObject, eNoDimension, eCreate_Default);
if LowerCase(LayerName) = 'top' then Poly.Layer := eTopLayer else Poly.Layer := eBottomLayer;
Poly.Net := Net;
Poly.Name := PolyName;
Poly.PolyHatchStyle := ePolySolid;
Poly.PourOver := ePolygonPourOver_SameNet;
Poly.RemoveDead := True;
Poly.PointCount := 4;
for I := 0 to 4 do
begin
    N := I mod 4;
    Seg := TPolySegment;
    Seg.Kind := ePolySegmentLine;
    if (N = 0) or (N = 3) then Seg.vx := Board.XOrigin + MMsToCoord(X1) else Seg.vx := Board.XOrigin + MMsToCoord(X2);
    if N < 2 then Seg.vy := Board.YOrigin + MMsToCoord(Y1) else Seg.vy := Board.YOrigin + MMsToCoord(Y2);
    Poly.Segments[I] := Seg;
end;
Board.AddPCBObject(Poly);
PCBServer.SendMessageToRobots(Board.I_ObjectAddress, c_Broadcast, PCBM_BoardRegisteration, Poly.I_ObjectAddress);
Poly.Rebuild;
PCBServer.PostProcess;
Board.GraphicallyInvalidate;
// what it poured: its pieces (regions or tracks)
N := 0;
GIt := Poly.GroupIterator_Create;
GIt.AddFilter_ObjectSet(MkSet(eRegionObject, eTrackObject, eArcObject));
Prim := GIt.FirstPCBObject;
while Prim <> nil do begin N := N + 1; Prim := GIt.NextPCBObject; end;
Poly.GroupIterator_Destroy(GIt);
Emit('polygon ' + Poly.Name + ' on ' + Layer2String(Poly.Layer) + ' net ' + Poly.Net.Name + ' poured into ' + IntToStr(N) + ' pieces; modified=' + BoolToStr(SD.Modified, True));
