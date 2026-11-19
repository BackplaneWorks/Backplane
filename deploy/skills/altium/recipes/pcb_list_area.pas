// List the board objects (tracks, vias, arcs, pads, components) inside a rectangle, with layer and net.
var
    SD : IServerDocument;
    Board : IPCB_Board;
    It : IPCB_SpatialIterator;
    Obj : IPCB_Primitive;
    S, NetS : String;
const
    // ---- edit: placeholders ----
    PcbPath = 'C:\Projects\Board\Board.PcbDoc';
    Left = -22; Bottom = -24; Right = -13; Top = -20;   // the area, mm from the board origin
function MM(C : Integer) : String;
begin
    Result := BpFmt('0.###', CoordToMMs(C));
end;
SD := Client.GetDocumentByPath(PcbPath);
if SD = nil then SD := Client.OpenDocument('PCB', PcbPath);
Board := PCBServer.GetPCBBoardByPath(PcbPath);
if Board = nil then begin Emit('not open: ' + PcbPath); Exit; end;
It := Board.SpatialIterator_Create;
It.AddFilter_ObjectSet(MkSet(eTrackObject, eViaObject, eArcObject, ePadObject, eComponentObject));
It.AddFilter_LayerSet(AllLayers);
It.AddFilter_Area(Board.XOrigin + MMsToCoord(Left), Board.YOrigin + MMsToCoord(Bottom), Board.XOrigin + MMsToCoord(Right), Board.YOrigin + MMsToCoord(Top));
Obj := It.FirstPCBObject;
while Obj <> nil do
begin
    if Obj.Net <> nil then NetS := Obj.Net.Name else NetS := '(no net)';
    case Obj.ObjectId of
        eTrackObject: S := 'track ' + Layer2String(Obj.Layer) + ' ' + MM(Obj.X1 - Board.XOrigin) + ',' + MM(Obj.Y1 - Board.YOrigin) + ' -> ' + MM(Obj.X2 - Board.XOrigin) + ',' + MM(Obj.Y2 - Board.YOrigin) + ' w ' + MM(Obj.Width);
        eViaObject: S := 'via ' + MM(Obj.x - Board.XOrigin) + ',' + MM(Obj.y - Board.YOrigin) + ' d ' + MM(Obj.Size) + ' hole ' + MM(Obj.HoleSize);
        ePadObject: S := 'pad ' + Obj.Name + ' ' + MM(Obj.x - Board.XOrigin) + ',' + MM(Obj.y - Board.YOrigin);
    else
        S := Obj.ObjectIDString + ' ' + Obj.Descriptor;
    end;
    Emit(S + ' | ' + NetS);
    Obj := It.NextPCBObject;
end;
Board.SpatialIterator_Destroy(It);
