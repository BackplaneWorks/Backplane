// List a schematic's wires (vertices), net labels, power ports, ports and junctions, in mils.
var
    SD : IServerDocument;
    Sch : ISch_Document;
    It : ISch_Iterator;
    Obj : ISch_GraphicalObject;
    I : Integer;
    S : String;
const
    // ---- edit: placeholders ----
    SchPath = 'C:\Projects\Board\Sheet1.SchDoc';
function Mils(P : TLocation) : String;
begin
    Result := IntToStr(CoordToMils(P.X)) + ',' + IntToStr(CoordToMils(P.Y));
end;
SD := Client.GetDocumentByPath(SchPath);
if SD = nil then SD := Client.OpenDocument('SCH', SchPath);
Sch := SchServer.GetSchDocumentByPath(SchPath);
It := Sch.SchIterator_Create;
It.AddFilter_ObjectSet(MkSet(eWire, eNetLabel, ePowerObject, ePort, eNoERC, eJunction));
Obj := It.FirstSchObject;
while Obj <> nil do
begin
    case Obj.ObjectId of
        eWire:
            begin
                S := 'wire';
                for I := 1 to Obj.VerticesCount do S := S + ' ' + Mils(Obj.Vertex(I));
            end;
        eNetLabel: S := 'netlabel ' + Obj.Text + ' at ' + Mils(Obj.Location);
        ePowerObject: S := 'power ' + Obj.Text + ' at ' + Mils(Obj.Location);
        ePort: S := 'port ' + Obj.Name + ' at ' + Mils(Obj.Location);
        eJunction: S := 'junction at ' + Mils(Obj.Location);
    else
        S := Obj.GetState_DescriptionString + ' at ' + Mils(Obj.Location);
    end;
    Emit(S);
    Obj := It.NextSchObject;
end;
Sch.SchIterator_Destroy(It);
