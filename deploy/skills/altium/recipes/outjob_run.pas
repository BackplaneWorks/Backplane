// Run one output container of an OutJob (Gerber/NC drill/ODB 'Folder Structure', or 'PDF') headlessly.
// The OutJob is added to the project if needed and brought to the front (the OutJob processes act on the focused OutJob).
// Files go where the OutJob says (default: <project folder>\Project Outputs for <project>). Save edited documents first.
var
    Prj : IProject;
    SD : IServerDocument;
    I, T0, N : Integer;
    InPrj : Boolean;
    Files : TStringList;
    F : String;
const
    // ---- edit: placeholders ----
    PrjPath = 'C:\Projects\Board\Board.PrjPcb';
    OutJobPath = 'C:\Projects\Board\Fabrication.OutJob';
    Container = 'Folder Structure';   // the OutJob's output container to run: a 'Folder Structure' (files) or a 'PDF' one
if not FileExists(OutJobPath) then begin BpFail('no such OutJob: ' + OutJobPath); Exit; end;
Prj := GetWorkspace.DM_OpenProject(PrjPath, True);
if Prj = nil then begin BpFail('cannot open ' + PrjPath); Exit; end;
InPrj := False;
for I := 0 to Prj.DM_LogicalDocumentCount - 1 do
    if SameText(Prj.DM_LogicalDocuments(I).DM_FullPath, OutJobPath) then InPrj := True;
if not InPrj then begin Prj.DM_AddSourceDocument(OutJobPath); Emit('added ' + OutJobPath + ' to the project'); end;
SD := Client.GetDocumentByPath(OutJobPath);
if SD = nil then SD := Client.OpenDocument('OUTPUTJOB', OutJobPath);
if SD = nil then begin BpFail('cannot open ' + OutJobPath); Exit; end;
// the OutJob processes act on the focused OutJob: bring it to the front
Client.ShowDocument(SD);
if not SameText(GetWorkspace.DM_FocusedDocument.DM_FullPath, OutJobPath) then begin BpFail('the OutJob did not get focus'); Exit; end;
// files written from now on, under the project's folder, are this run's output (a run that wrote none failed)
T0 := DateTimeToFileDate(Now) - 2;
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
Files := TStringList.Create;
FindFiles(ExtractFilePath(PrjPath), '*.*', faAnyFile, True, Files);
N := 0;
for I := 0 to Files.Count - 1 do
begin
    F := Files[I];
    if (Pos('\PROJECT LOGS FOR ', UpperCase(F)) > 0) or (Pos('\HISTORY\', UpperCase(F)) > 0) or (Pos('\__PREVIEWS\', UpperCase(F)) > 0) then Continue;
    if (FileAge(F) >= T0) and not DirectoryExists(F) then
    begin
        N := N + 1;
        if N <= 30 then Emit('  wrote ' + F);
    end;
end;
Files.Free;
if N = 0 then BpFail('ran ' + Container + ' of ' + OutJobPath + ', but no output file was written under ' + ExtractFilePath(PrjPath) + ' (an output with no data, a container name the OutJob does not have, or an output path outside the project)')
else Emit('ran ' + Container + ' of ' + OutJobPath + ': ' + IntToStr(N) + ' files written (default folder: ' + Prj.DM_GetOutputPath + ')');
