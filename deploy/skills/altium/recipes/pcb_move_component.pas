// Move a board component to an x/y (mm from the board origin) and set its rotation.
var
    SD : IServerDocument;
    Board : IPCB_Board;
    Comp : IPCB_Component;
const
    // ---- edit: placeholders ----
    PcbPath = 'C:\Projects\Board\Board.PcbDoc';
    Designator = 'C1';
    XMM = -11.7;       // new position, mm from the board origin
    YMM = -10.65;
    RotDeg = 0;        // absolute rotation, degrees
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
Comp := Board.GetPcbComponentByRefDes(Designator);
if Comp = nil then begin Emit('no component ' + Designator); Exit; end;
PCBServer.PreProcess;
PCBServer.SendMessageToRobots(Comp.I_ObjectAddress, c_Broadcast, PCBM_BeginModify, c_NoEventData);
Comp.MoveToXY(Board.XOrigin + MMsToCoord(XMM), Board.YOrigin + MMsToCoord(YMM));
Comp.Rotation := RotDeg;
PCBServer.SendMessageToRobots(Comp.I_ObjectAddress, c_Broadcast, PCBM_EndModify, c_NoEventData);
PCBServer.PostProcess;
Board.GraphicallyInvalidate;
Emit(Designator + ' now ' + FormatFloat('0.###', CoordToMMs(Comp.x - Board.XOrigin)) + ',' + FormatFloat('0.###', CoordToMMs(Comp.y - Board.YOrigin))
    + ' mm rot ' + FormatFloat('0.#', Comp.Rotation) + '; modified=' + BoolToStr(SD.Modified, True));
