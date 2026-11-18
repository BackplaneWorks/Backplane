// Delete board objects of chosen kinds inside an area, optionally only on one net.
var
    SD : IServerDocument;
    Board : IPCB_Board;
    It : IPCB_SpatialIterator;
    Obj : IPCB_Primitive;
    Doomed : TInterfaceList;
    I : Integer;
const
    // ---- edit: placeholders ----
    PcbPath = 'C:\Projects\Board\Board.PcbDoc';
    Left = -21; Bottom = -22.5; Right = -14.5; Top = -21.5;     // the area, mm from the board origin
    NetName = 'GND';                                          // only objects on this net ('' for any)
function IsCurrent(Path : String) : Boolean;
var B : IPCB_Board;
begin
    Result := False;
    B := PCBServer.GetCurrentPCBBoard;
    if B <> nil then Result := SameText(B.FileName, Path);
end;
SD := Client.GetDocumentByPath(PcbPath);
if SD = nil then SD := Client.OpenDocument('PCB', PcbPath);
Board := PCBServer.GetPCBBoardByPath(PcbPath);
if Board = nil then begin Emit('not open: ' + PcbPath); Exit; end;
// Edits register with the undo of Altium's CURRENT board, whatever board they touch: make this board the current one
// first, or the person's board in front gets the undo entries and is marked modified.
if not IsCurrent(PcbPath) then Client.ShowDocument(SD);
if not IsCurrent(PcbPath) then begin Emit('could not make ' + PcbPath + ' the current board'); Exit; end;
Doomed := TInterfaceList.Create;
It := Board.SpatialIterator_Create;
It.AddFilter_ObjectSet(MkSet(eTrackObject, eViaObject));   // the kinds to delete: also eArcObject, ePadObject, eFillObject, eRegionObject, eComponentObject...
It.AddFilter_LayerSet(AllLayers);
It.AddFilter_Area(Board.XOrigin + MMsToCoord(Left), Board.YOrigin + MMsToCoord(Bottom), Board.XOrigin + MMsToCoord(Right), Board.YOrigin + MMsToCoord(Top));
Obj := It.FirstPCBObject;
while Obj <> nil do
begin
    if (NetName = '') or ((Obj.Net <> nil) and (Obj.Net.Name = NetName)) then Doomed.Add(Obj);
    Obj := It.NextPCBObject;
end;
Board.SpatialIterator_Destroy(It);
PCBServer.PreProcess;
for I := 0 to Doomed.Count - 1 do
begin
    Obj := Doomed.Items[I];
    Emit('delete ' + Obj.Descriptor);
    Board.RemovePCBObject(Obj);
    PCBServer.SendMessageToRobots(Board.I_ObjectAddress, c_Broadcast, PCBM_BoardRegisteration, Obj.I_ObjectAddress);
end;
PCBServer.PostProcess;
Board.GraphicallyInvalidate;
Emit('deleted ' + IntToStr(Doomed.Count));
Doomed.Free;
