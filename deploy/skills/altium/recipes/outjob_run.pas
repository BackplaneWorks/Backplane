// Run one output container of an OutJob (Gerber/NC drill/ODB 'Folder Structure', or 'PDF') headlessly.
// The OutJob is added to the project if needed and brought to the front (the OutJob processes act on the focused OutJob).
// Files go where the OutJob says (default: <project folder>\Project Outputs for <project>). Save edited documents first.
var
    Prj : IProject;
    SD : IServerDocument;
    I : Integer;
    InPrj : Boolean;
const
    // ---- edit: placeholders ----
    PrjPath = 'C:\Projects\Board\Board.PrjPcb';
    OutJobPath = 'C:\Projects\Board\Fabrication.OutJob';
    Container = 'Folder Structure';   // the OutJob's output container to run: a 'Folder Structure' (files) or a 'PDF' one
Prj := GetWorkspace.DM_OpenProject(PrjPath, True);
InPrj := False;
for I := 0 to Prj.DM_LogicalDocumentCount - 1 do
    if SameText(Prj.DM_LogicalDocuments(I).DM_FullPath, OutJobPath) then InPrj := True;
if not InPrj then begin Prj.DM_AddSourceDocument(OutJobPath); Emit('added ' + OutJobPath + ' to the project'); end;
SD := Client.GetDocumentByPath(OutJobPath);
if SD = nil then SD := Client.OpenDocument('OUTPUTJOB', OutJobPath);
if SD = nil then begin Emit('cannot open ' + OutJobPath); Exit; end;
// the OutJob processes act on the focused OutJob: bring it to the front
Client.ShowDocument(SD);
if not SameText(GetWorkspace.DM_FocusedDocument.DM_FullPath, OutJobPath) then begin Emit('the OutJob did not get focus'); Exit; end;
ResetParameters;
if Container = 'PDF' then
begin
    AddStringParameter('Action', 'PublishToPDF');
    AddStringParameter('ObjectKind', 'OutputBatch');
    AddStringParameter('OutputMedium', Container);
    AddStringParameter('DisableDialog', 'True');
    RunProcess('WorkspaceManager:Print');
end
else
begin
    AddStringParameter('Action', 'Run');
    AddStringParameter('ObjectKind', 'OutputBatch');
    AddStringParameter('OutputMedium', Container);
    RunProcess('WorkspaceManager:GenerateReport');
end;
Emit('ran ' + Container + ' of ' + OutJobPath);
