// Add a track (on a layer, with a net) between two points and a via at its end.
var
    SD : IServerDocument;
    Board : IPCB_Board;
    Track : IPCB_Track;
    Via : IPCB_Via;
    Net : IPCB_Net;
const
    // ---- edit: placeholders ----
    PcbPath = 'C:\Projects\Board\Board.PcbDoc';
    NetName = 'GND';                 // '' for no net
    X1 = -20; Y1 = -22;              // track start, mm from the board origin
    X2 = -15; Y2 = -22;              // track end; the via goes here
    WidthMM = 0.25;
    ViaMM = 0.6; ViaHoleMM = 0.3;
function FindNet(B : IPCB_Board; Name : String) : IPCB_Net;
var It : IPCB_BoardIterator; N : IPCB_Net;
begin
    Result := nil;
    It := B.BoardIterator_Create;
    It.AddFilter_ObjectSet(MkSet(eNetObject));
    It.AddFilter_LayerSet(AllLayers);
    It.AddFilter_Method(eProcessAll);
    N := It.FirstPCBObject;
    while (N <> nil) and (Result = nil) do
    begin
        if N.Name = Name then Result := N;
        N := It.NextPCBObject;
    end;
    B.BoardIterator_Destroy(It);
end;
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
Net := nil;
if NetName <> '' then
begin
    Net := FindNet(Board, NetName);
    if Net = nil then begin Emit('no net ' + NetName); Exit; end;
end;
PCBServer.PreProcess;
Track := PCBServer.PCBObjectFactory(eTrackObject, eNoDimension, eCreate_Default);
Track.X1 := Board.XOrigin + MMsToCoord(X1);
Track.Y1 := Board.YOrigin + MMsToCoord(Y1);
Track.X2 := Board.XOrigin + MMsToCoord(X2);
Track.Y2 := Board.YOrigin + MMsToCoord(Y2);
Track.Layer := eTopLayer;
Track.Width := MMsToCoord(WidthMM);
if Net <> nil then Track.Net := Net;
Board.AddPCBObject(Track);
PCBServer.SendMessageToRobots(Board.I_ObjectAddress, c_Broadcast, PCBM_BoardRegisteration, Track.I_ObjectAddress);
Via := PCBServer.PCBObjectFactory(eViaObject, eNoDimension, eCreate_Default);
Via.x := Board.XOrigin + MMsToCoord(X2);
Via.y := Board.YOrigin + MMsToCoord(Y2);
Via.Size := MMsToCoord(ViaMM);
Via.HoleSize := MMsToCoord(ViaHoleMM);
Via.LowLayer := eBottomLayer;
Via.HighLayer := eTopLayer;
if Net <> nil then Via.Net := Net;
Board.AddPCBObject(Via);
PCBServer.SendMessageToRobots(Board.I_ObjectAddress, c_Broadcast, PCBM_BoardRegisteration, Via.I_ObjectAddress);
PCBServer.PostProcess;
Board.GraphicallyInvalidate;
Emit('track ' + Layer2String(Track.Layer) + ' net ' + NetName + ' and via added; modified=' + BoolToStr(SD.Modified, True));
