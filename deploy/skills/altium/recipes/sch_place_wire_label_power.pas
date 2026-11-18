// Place a wire between two points, a net label on it and a power port at its end.
var
    SD : IServerDocument;
    Sch : ISch_Document;
    Wire : ISch_Wire;
    Lbl : ISch_NetLabel;
    Pwr : ISch_PowerObject;
const
    // ---- edit: placeholders ----
    SchPath = 'C:\Projects\Board\Sheet1.SchDoc';
    X1 = 1000;  Y1 = 2000;   // wire start, mils
    X2 = 2000;  Y2 = 2000;   // wire end, mils
    NetName = 'NET1';
    PowerNet = 'GND';
procedure AddToSheet(Obj : ISch_GraphicalObject);
begin
    Sch.RegisterSchObjectInContainer(Obj);
    SchServer.RobotManager.SendMessage(Sch.I_ObjectAddress, c_BroadCast, SCHM_PrimitiveRegistration, Obj.I_ObjectAddress);
end;
SD := Client.GetDocumentByPath(SchPath);
if SD = nil then SD := Client.OpenDocument('SCH', SchPath);
Sch := SchServer.GetSchDocumentByPath(SchPath);
if Sch = nil then begin Emit('not open: ' + SchPath); Exit; end;
SchServer.ProcessControl.PreProcess(Sch, '');
// a wire: a polyline of vertices, numbered from 1
Wire := SchServer.SchObjectFactory(eWire, eCreate_Default);
Wire.Location := Point(MilsToCoord(X1), MilsToCoord(Y1));
Wire.InsertVertex := 1;
Wire.SetState_Vertex(1, Point(MilsToCoord(X1), MilsToCoord(Y1)));
Wire.InsertVertex := 2;
Wire.SetState_Vertex(2, Point(MilsToCoord(X2), MilsToCoord(Y2)));
AddToSheet(Wire);
// a net label on the wire (its hot spot is its Location)
Lbl := SchServer.SchObjectFactory(eNetLabel, eCreate_Default);
Lbl.Location := Point(MilsToCoord(X1 + 200), MilsToCoord(Y1));
Lbl.Text := NetName;
AddToSheet(Lbl);
// a power port at the wire's end
Pwr := SchServer.SchObjectFactory(ePowerObject, eCreate_Default);
Pwr.Location := Point(MilsToCoord(X2), MilsToCoord(Y2));
Pwr.Style := ePowerGndPower;
Pwr.Text := PowerNet;
Pwr.ShowNetName := True;
Pwr.Orientation := eRotate270;
AddToSheet(Pwr);
SchServer.ProcessControl.PostProcess(Sch, '');
Sch.GraphicallyInvalidate;
Emit('placed wire, net label ' + NetName + ', power port ' + PowerNet + '; modified=' + BoolToStr(SD.Modified, True));
