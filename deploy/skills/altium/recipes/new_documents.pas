// Create a project's new, empty schematic sheet and board, save them, add them to the project and save the project.
// The project file may be new: write a two-line .PrjPcb first ("[Design]" then "Version=1.0") if there is none.
// Each document is made only when its file does not exist yet.
var
    Prj : IProject;
    SD : IServerDocument;
    PD : IServerDocument;
const
    // ---- edit: placeholders ----
    PrjPath = 'C:\Projects\Board\Board.PrjPcb';
    SchPath = 'C:\Projects\Board\Board.SchDoc';   // '' for none
    PcbPath = 'C:\Projects\Board\Board.PcbDoc';   // '' for none
function InProject(Path : String) : Boolean;
var K : Integer;
begin
    Result := False;
    for K := 0 to Prj.DM_LogicalDocumentCount - 1 do
        if SameText(Prj.DM_LogicalDocuments(K).DM_FullPath, Path) then Result := True;
end;
procedure MakeDoc(Kind, Path : String);
var D : IServerDocument;
begin
    if Path = '' then Exit;
    if FileExists(Path) then begin Emit('exists, kept: ' + Path); end
    else
    begin
        // a new document has no file until it is renamed and saved (DoFileSave alone on it opens a Save As box)
        D := Client.OpenNewDocument(Kind, '', ExtractFileName(Path), False);
        if D = nil then begin Emit('could not make a ' + Kind + ' document'); Exit; end;
        D.DoSafeChangeFileNameAndSave(Path, Kind);
        D.DoFileSave('');
        if FileExists(Path) then Emit('made ' + Path) else Emit('NOT written: ' + Path);
    end;
    if not InProject(Path) then Prj.DM_AddSourceDocument(Path);
end;
Prj := GetWorkspace.DM_OpenProject(PrjPath, True);
if Prj = nil then begin Emit('cannot open project ' + PrjPath); Exit; end;
MakeDoc('SCH', SchPath);
MakeDoc('PCB', PcbPath);
PD := Client.GetDocumentByPath(PrjPath);
if PD = nil then PD := Client.OpenDocument('PCBPROJECT', PrjPath);
if PD <> nil then PD.DoFileSave('');
Emit('project documents: ' + IntToStr(Prj.DM_LogicalDocumentCount));
