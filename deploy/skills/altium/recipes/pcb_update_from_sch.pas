// Update the board from the schematic (Design > Import Changes, Altium's ECO) without the person: BpExpect has the hub
// work Altium's own dialogs (Component Links: match by designator; Engineering Change Order: Validate, Execute, Close).
// Altium places new footprints beside the board, in a room per sheet, with their nets; it also removes parts and nets
// the schematic no longer has. Footprints come from the libraries the project and Altium know (the symbol's footprint
// link). Check with sch_pcb_diff.pas before (what will change) and after (expect "differences 0").
var
    Prj : IProject;
    SD : IServerDocument;
    Board : IPCB_Board;
    C0, C1, N0, N1 : Integer;
const
    // ---- edit: placeholders ----
    PrjPath = 'C:\Projects\Board\Board.PrjPcb';
    PcbPath = 'C:\Projects\Board\Board.PcbDoc';
function IsCurrent(Path : String) : Boolean;
var B : IPCB_Board;
begin
    Result := False;
    B := PCBServer.GetCurrentPCBBoard;
    if B <> nil then Result := SameText(B.FileName, Path);
end;
function Count(Kind : Integer) : Integer;
var It : IPCB_BoardIterator; O : IPCB_Primitive;
begin
    Result := 0;
    It := Board.BoardIterator_Create;
    It.AddFilter_ObjectSet(MkSet(Kind));
    It.AddFilter_LayerSet(AllLayers);
    It.AddFilter_Method(eProcessAll);
    O := It.FirstPCBObject;
    while O <> nil do begin Result := Result + 1; O := It.NextPCBObject; end;
    Board.BoardIterator_Destroy(It);
end;
Prj := GetWorkspace.DM_OpenProject(PrjPath, True);
if Prj = nil then begin Emit('cannot open ' + PrjPath); Exit; end;
Prj.DM_Compile;
SD := Client.GetDocumentByPath(PcbPath);
if SD = nil then SD := Client.OpenDocument('PCB', PcbPath);
Board := PCBServer.GetPCBBoardByPath(PcbPath);
if Board = nil then begin Emit('not open: ' + PcbPath); Exit; end;
// the ECO updates the board in front: make this one current first, and check
if not IsCurrent(PcbPath) then Client.ShowDocument(SD);
if not IsCurrent(PcbPath) then begin Emit('could not make ' + PcbPath + ' the current board'); Exit; end;
C0 := Count(eComponentObject); N0 := Count(eNetObject);
// Component Links comes only when parts are not linked yet (a first import); the ECO dialog always
BpExpect('Component Links', 'click Perform Update');
BpExpect('Engineering Change Order', 'click Validate Changes; wait 1; click Execute Changes; wait 2; click Close');
ResetParameters;
AddStringParameter('ObjectKind', 'Project');
AddStringParameter('Action', 'UpdateMe');
RunProcess('WorkspaceManager:Compare');
C1 := Count(eComponentObject); N1 := Count(eNetObject);
// save at once: until it is saved, the ECO's parts exist only in Altium's memory, and a box or a closed document loses them
Emit('components ' + IntToStr(C0) + ' -> ' + IntToStr(C1) + ', nets ' + IntToStr(N0) + ' -> ' + IntToStr(N1) + '; saved ' + BoolToStr(SD.DoFileSave(''), True) + ', modified=' + BoolToStr(SD.Modified, True));
