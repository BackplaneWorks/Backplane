// List a board's components (designator, footprint, comment, x/y in mm from the board origin, rotation, layer)
// and its nets with their pads.
var
    SD : IServerDocument;
    Board : IPCB_Board;
    It : IPCB_BoardIterator;
    Comp : IPCB_Component;
    Pad : IPCB_Pad;
    Nets : TStringList;
    I : Integer;
const
    // ---- edit: placeholders ----
    PcbPath = 'C:\Projects\Board\Board.PcbDoc';
function MM(C : Integer) : String;
begin
    Result := FormatFloat('0.###', CoordToMMs(C));
end;
SD := Client.GetDocumentByPath(PcbPath);
if SD = nil then SD := Client.OpenDocument('PCB', PcbPath);
Board := PCBServer.GetPCBBoardByPath(PcbPath);
if Board = nil then begin Emit('not open: ' + PcbPath); Exit; end;
Emit('origin ' + MM(Board.XOrigin) + ',' + MM(Board.YOrigin) + ' mm (absolute); positions below are relative to it');
It := Board.BoardIterator_Create;
It.AddFilter_ObjectSet(MkSet(eComponentObject));
It.AddFilter_LayerSet(AllLayers);
It.AddFilter_Method(eProcessAll);
Comp := It.FirstPCBObject;
while Comp <> nil do
begin
    Emit(Comp.Name.Text + ' | ' + Comp.Pattern + ' | ' + Comp.Comment.Text + ' | ' + MM(Comp.x - Board.XOrigin) + ',' + MM(Comp.y - Board.YOrigin) + ' mm | rot '
        + FormatFloat('0.#', Comp.Rotation) + ' | ' + Layer2String(Comp.Layer));
    Comp := It.NextPCBObject;
end;
Board.BoardIterator_Destroy(It);
// nets with their pads (designator.pad), from each pad's net
Nets := TStringList.Create;
It := Board.BoardIterator_Create;
It.AddFilter_ObjectSet(MkSet(ePadObject));
It.AddFilter_LayerSet(AllLayers);
It.AddFilter_Method(eProcessAll);
Pad := It.FirstPCBObject;
while Pad <> nil do
begin
    if (Pad.Net <> nil) and (Pad.Component <> nil) then
        Nets.Values[Pad.Net.Name] := Nets.Values[Pad.Net.Name] + ' ' + Pad.Component.Name.Text + '.' + Pad.Name;
    Pad := It.NextPCBObject;
end;
Board.BoardIterator_Destroy(It);
Nets.Sort;
for I := 0 to Nets.Count - 1 do Emit('net ' + Nets.Names[I] + ':' + Nets.ValueFromIndex[I]);
Emit('nets with pads ' + IntToStr(Nets.Count));
Nets.Free;
