// Change a schematic component's comment and one parameter, and move it (its pins do not drag wires along).
var
    SD : IServerDocument;
    Sch : ISch_Document;
    It, PIt : ISch_Iterator;
    Comp, Found : ISch_Component;
    Par : ISch_Parameter;
const
    // ---- edit: placeholders ----
    SchPath = 'C:\Projects\Board\Sheet1.SchDoc';
    Designator = 'C1';
    NewComment = '1u';
    ParamName = 'VDC';
    ParamValue = '25V';
    MoveDxMils = 100;
    MoveDyMils = 0;
SD := Client.GetDocumentByPath(SchPath);
if SD = nil then SD := Client.OpenDocument('SCH', SchPath);
Sch := SchServer.GetSchDocumentByPath(SchPath);
if Sch = nil then begin Emit('not open: ' + SchPath); Exit; end;
Found := nil;
It := Sch.SchIterator_Create;
It.AddFilter_ObjectSet(MkSet(eSchComponent));
Comp := It.FirstSchObject;
while (Comp <> nil) and (Found = nil) do
begin
    if Comp.Designator.Text = Designator then Found := Comp;
    Comp := It.NextSchObject;
end;
Sch.SchIterator_Destroy(It);
if Found = nil then begin Emit('no component ' + Designator); Exit; end;
SchServer.ProcessControl.PreProcess(Sch, '');
// the comment
SchServer.RobotManager.SendMessage(Found.I_ObjectAddress, c_BroadCast, SCHM_BeginModify, c_NoEventData);
Found.Comment.Text := NewComment;
SchServer.RobotManager.SendMessage(Found.I_ObjectAddress, c_BroadCast, SCHM_EndModify, c_NoEventData);
// a parameter
PIt := Found.SchIterator_Create;
PIt.AddFilter_ObjectSet(MkSet(eParameter));
Par := PIt.FirstSchObject;
while Par <> nil do
begin
    if Par.Name = ParamName then
    begin
        SchServer.RobotManager.SendMessage(Par.I_ObjectAddress, c_BroadCast, SCHM_BeginModify, c_NoEventData);
        Par.Text := ParamValue;
        SchServer.RobotManager.SendMessage(Par.I_ObjectAddress, c_BroadCast, SCHM_EndModify, c_NoEventData);
        Emit('set ' + ParamName + '=' + ParamValue);
    end;
    Par := PIt.NextSchObject;
end;
Found.SchIterator_Destroy(PIt);
// move (the component's designator, comment and parameters move with it)
SchServer.RobotManager.SendMessage(Found.I_ObjectAddress, c_BroadCast, SCHM_BeginModify, c_NoEventData);
Found.MoveByXY(MilsToCoord(MoveDxMils), MilsToCoord(MoveDyMils));
SchServer.RobotManager.SendMessage(Found.I_ObjectAddress, c_BroadCast, SCHM_EndModify, c_NoEventData);
SchServer.ProcessControl.PostProcess(Sch, '');
Sch.GraphicallyInvalidate;
Emit(Designator + ' comment=' + Found.Comment.Text + ' at ' + IntToStr(CoordToMils(Found.Location.X)) + ',' + IntToStr(CoordToMils(Found.Location.Y)) + ' mil; modified=' + BoolToStr(SD.Modified, True));
