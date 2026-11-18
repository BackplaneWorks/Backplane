// Read the violations a board carries: the markers Altium's own DRC left (Tools > Design Rule Check, run by the
// person, with "Create Violations" on; the online DRC adds them as objects are edited when it is on). Read-only.
// For a check the script runs itself, see pcb_drc.pas.
var
    SD : IServerDocument;
    Board : IPCB_Board;
    It : IPCB_BoardIterator;
    V : IPCB_Violation;
    N : Integer;
    S : String;
const
    // ---- edit: placeholders ----
    PcbPath = 'C:\Projects\Board\Board.PcbDoc';
    MaxReport = 100;
function Desc(P : IPCB_Primitive) : String;
begin
    Result := '';
    if P = nil then Exit;
    Result := P.Descriptor;
    if P.Net <> nil then Result := Result + ' [' + P.Net.Name + ']';
end;
SD := Client.GetDocumentByPath(PcbPath);
if SD = nil then SD := Client.OpenDocument('PCB', PcbPath);
Board := PCBServer.GetPCBBoardByPath(PcbPath);
if Board = nil then begin Emit('not open: ' + PcbPath); Exit; end;
N := 0;
It := Board.BoardIterator_Create;
It.AddFilter_ObjectSet(MkSet(eViolationObject));
It.AddFilter_LayerSet(AllLayers);
It.AddFilter_Method(eProcessAll);
V := It.FirstPCBObject;
while V <> nil do
begin
    N := N + 1;
    if N <= MaxReport then
    begin
        S := '';
        if V.Rule <> nil then S := V.Rule.Name;
        Emit(S + ' | ' + V.Descriptor + ' | ' + Desc(V.Primitive1) + ' | ' + Desc(V.Primitive2));
    end;
    V := It.NextPCBObject;
end;
Board.BoardIterator_Destroy(It);
Emit('violations ' + IntToStr(N) + ' (the batch DRC lists most pairs twice)');
