// Open a project by path (no harm if it is open already) and list its documents: kind, path, loaded/modified.
// With OpenSheets, also load its schematics and board in the background (not shown, the person's view does not change),
// so SchServer.GetSchDocumentByPath / PCBServer.GetPCBBoardByPath find them.
var
    Prj : IProject;
    D : IDocument;
    SD : IServerDocument;
    I : Integer;
    S : String;
const
    // ---- edit: placeholder path ----
    PrjPath = 'C:\Projects\Board\Board.PrjPcb';
    OpenSheets = True;
Prj := GetWorkspace.DM_OpenProject(PrjPath, True);
if Prj = nil then begin Emit('cannot open ' + PrjPath); Exit; end;
Emit('project ' + Prj.DM_ProjectFullPath + ', ' + IntToStr(Prj.DM_LogicalDocumentCount) + ' documents');
for I := 0 to Prj.DM_LogicalDocumentCount - 1 do
begin
    D := Prj.DM_LogicalDocuments(I);
    SD := Client.GetDocumentByPath(D.DM_FullPath);
    if (SD = nil) and OpenSheets and ((D.DM_DocumentKind = 'SCH') or (D.DM_DocumentKind = 'PCB')) then
        SD := Client.OpenDocument(D.DM_DocumentKind, D.DM_FullPath);
    if SD = nil then S := 'not loaded'
    else if SD.Modified <> False then S := 'loaded, MODIFIED (unsaved)'
    else S := 'loaded';
    Emit(D.DM_DocumentKind + ' | ' + D.DM_FullPath + ' | ' + S);
end;
