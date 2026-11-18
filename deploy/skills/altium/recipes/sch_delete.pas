// Delete schematic objects: a component by designator, net labels by text, power ports and wires at a point.
var
    SD : IServerDocument;
    Sch : ISch_Document;
    It : ISch_Iterator;
    Obj : ISch_GraphicalObject;
    Doomed : TInterfaceList;
    I : Integer;
const
    // ---- edit: placeholders ----
    SchPath = 'C:\Projects\Board\Sheet1.SchDoc';
    Designator = 'C9';        // a component to delete ('' for none)
    LabelText = 'NET1';    // net labels with this text
    AtX = 2000; AtY = 2000;   // power ports at this point, and wires that start or end here (mils)
function Hits(O : ISch_GraphicalObject) : Boolean;
var P1, P2 : TLocation;
begin
    Result := False;
    case O.ObjectId of
        eSchComponent: Result := (Designator <> '') and (O.Designator.Text = Designator);
        eNetLabel: Result := O.Text = LabelText;
        ePowerObject: Result := (CoordToMils(O.Location.X) = AtX) and (CoordToMils(O.Location.Y) = AtY);
        eWire:
            begin
                P1 := O.Vertex(1); P2 := O.Vertex(O.VerticesCount);
                Result := ((CoordToMils(P1.X) = AtX) and (CoordToMils(P1.Y) = AtY)) or ((CoordToMils(P2.X) = AtX) and (CoordToMils(P2.Y) = AtY));
            end;
    end;
end;
SD := Client.GetDocumentByPath(SchPath);
if SD = nil then SD := Client.OpenDocument('SCH', SchPath);
Sch := SchServer.GetSchDocumentByPath(SchPath);
if Sch = nil then begin Emit('not open: ' + SchPath); Exit; end;
// collect first: removing objects while iterating skips some
Doomed := TInterfaceList.Create;
It := Sch.SchIterator_Create;
It.AddFilter_ObjectSet(MkSet(eSchComponent, eNetLabel, ePowerObject, eWire));
Obj := It.FirstSchObject;
while Obj <> nil do
begin
    if Hits(Obj) then Doomed.Add(Obj);
    Obj := It.NextSchObject;
end;
Sch.SchIterator_Destroy(It);
SchServer.ProcessControl.PreProcess(Sch, '');
for I := 0 to Doomed.Count - 1 do
begin
    Obj := Doomed.Items[I];
    Emit('delete ' + Obj.GetState_DescriptionString);
    Sch.RemoveSchObject(Obj);
    SchServer.RobotManager.SendMessage(Sch.I_ObjectAddress, c_BroadCast, SCHM_PrimitiveRegistration, Obj.I_ObjectAddress);
end;
SchServer.ProcessControl.PostProcess(Sch, '');
Sch.GraphicallyInvalidate;
Emit('deleted ' + IntToStr(Doomed.Count));
Doomed.Free;
