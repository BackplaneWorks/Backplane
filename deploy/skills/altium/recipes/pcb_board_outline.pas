// Set the board's shape to a rectangle (optionally with its corners cut at 45 degrees) from the board origin: the outline is
// drawn on a mechanical layer (what fabs read) and the board shape defined from those lines (Design > Board Shape > Define
// from selected objects, no dialog). A new board from new_documents.pas has Altium's default 6000 x 4000 mil shape; run
// this before placing parts. Board in front required (checked). It reads the new shape back (its box from the origin).
var
    SD : IServerDocument;
    Board : IPCB_Board;
    BO : IPCB_BoardOutline;
    Track : IPCB_Track;
    Xs, Ys : TStringList;
    I, N, K : Integer;
const
    // ---- edit: placeholders ----
    PcbPath = 'C:\Projects\Board\Board.PcbDoc';
    WidthMM = 30;
    HeightMM = 25;
    ChamferMM = 0;           // corner cut; 0 = square corners
    MechLayer = 1;           // the mechanical layer the outline is drawn on
    LineMM = 0.1;
function IsCurrent(Path : String) : Boolean;
var B : IPCB_Board;
begin
    Result := False;
    B := PCBServer.GetCurrentPCBBoard;
    if B <> nil then Result := SameText(B.FileName, Path);
end;
function Mech(M : Integer) : Integer;
begin
    case M of
        1: Result := eMechanical1;  2: Result := eMechanical2;  3: Result := eMechanical3;  4: Result := eMechanical4;
        5: Result := eMechanical5;  6: Result := eMechanical6;  7: Result := eMechanical7;  8: Result := eMechanical8;
        9: Result := eMechanical9;  10: Result := eMechanical10; 11: Result := eMechanical11; 12: Result := eMechanical12;
        13: Result := eMechanical13; 14: Result := eMechanical14; 15: Result := eMechanical15;
    else Result := eMechanical16;
    end;
end;
procedure Corner(X, Y : Double);
begin
    Xs.Add(FloatToStr(X)); Ys.Add(FloatToStr(Y));
end;
SD := Client.GetDocumentByPath(PcbPath);
if SD = nil then SD := Client.OpenDocument('PCB', PcbPath);
Board := PCBServer.GetPCBBoardByPath(PcbPath);
if Board = nil then begin Emit('not open: ' + PcbPath); Exit; end;
if not IsCurrent(PcbPath) then Client.ShowDocument(SD);
if not IsCurrent(PcbPath) then begin Emit('could not make ' + PcbPath + ' the current board'); Exit; end;
// the corners, counter-clockwise from the origin
Xs := TStringList.Create; Ys := TStringList.Create;
if ChamferMM > 0 then
begin
    Corner(ChamferMM, 0); Corner(WidthMM - ChamferMM, 0); Corner(WidthMM, ChamferMM); Corner(WidthMM, HeightMM - ChamferMM);
    Corner(WidthMM - ChamferMM, HeightMM); Corner(ChamferMM, HeightMM); Corner(0, HeightMM - ChamferMM); Corner(0, ChamferMM);
end
else
begin
    Corner(0, 0); Corner(WidthMM, 0); Corner(WidthMM, HeightMM); Corner(0, HeightMM);
end;
N := Xs.Count;
// nothing else selected: the shape is made from the selection
ResetParameters;
AddStringParameter('Scope', 'All');
RunProcess('PCB:DeSelect');
PCBServer.PreProcess;
for I := 0 to N - 1 do
    begin
        K := (I + 1) mod N;
        Track := PCBServer.PCBObjectFactory(eTrackObject, eNoDimension, eCreate_Default);
        Track.X1 := Board.XOrigin + MMsToCoord(StrToFloat(Xs[I])); Track.Y1 := Board.YOrigin + MMsToCoord(StrToFloat(Ys[I]));
        Track.X2 := Board.XOrigin + MMsToCoord(StrToFloat(Xs[K])); Track.Y2 := Board.YOrigin + MMsToCoord(StrToFloat(Ys[K]));
        Track.Layer := Mech(MechLayer);
        Track.Width := MMsToCoord(LineMM);
        Board.AddPCBObject(Track);
        PCBServer.SendMessageToRobots(Board.I_ObjectAddress, c_Broadcast, PCBM_BoardRegisteration, Track.I_ObjectAddress);
        Track.Selected := True;
    end;
PCBServer.PostProcess;
ResetParameters;
AddStringParameter('MODE', 'BOARDOUTLINE_FROM_SEL_PRIMS');
RunProcess('PCB:PlaceBoardOutline');
ResetParameters;
AddStringParameter('Scope', 'All');
RunProcess('PCB:DeSelect');
Board.GraphicallyInvalidate;
BO := Board.BoardOutline;
Emit('board outline ' + IntToStr(N) + ' corners, ' + FloatToStr(WidthMM) + ' x ' + FloatToStr(HeightMM) + ' mm from the origin; box ' +
    FormatFloat('0.##', CoordToMMs(BO.BoundingRectangle.Left - Board.XOrigin)) + ',' + FormatFloat('0.##', CoordToMMs(BO.BoundingRectangle.Bottom - Board.YOrigin)) + ' - ' +
    FormatFloat('0.##', CoordToMMs(BO.BoundingRectangle.Right - Board.XOrigin)) + ',' + FormatFloat('0.##', CoordToMMs(BO.BoundingRectangle.Top - Board.YOrigin)) + '; modified=' + BoolToStr(SD.Modified, True));
Xs.Free; Ys.Free;
