// List a .SchLib's symbols: name, description, designator, parts, then (for the symbols asked for) every pin with
// number, name, electrical type, location (mils, the end on the body) and orientation, the parameters and the footprint links.
var
    SD : IServerDocument;
    Lib : ISch_Lib;
    Comp : ISch_Component;
    It, CIt : ISch_Iterator;
    Obj : ISch_GraphicalObject;
    N : Integer;
    S : String;
const
    // ---- edit: placeholders ----
    LibPath = 'C:\Projects\Board\Board.SchLib';
    Detail = '*';                    // symbols to list pin by pin: '*' = all, else one name; '' = names only
function TypeName(E : Integer) : String;
begin
    if E = eElectricInput then Result := 'in'
    else if E = eElectricOutput then Result := 'out'
    else if E = eElectricIO then Result := 'io'
    else if E = eElectricPower then Result := 'pwr'
    else if E = eElectricOpenCollector then Result := 'oc'
    else if E = eElectricOpenEmitter then Result := 'oe'
    else if E = eElectricHiZ then Result := 'hiz'
    else Result := 'pas';
end;
SD := Client.GetDocumentByPath(LibPath);
if SD = nil then SD := Client.OpenDocument('SCHLIB', LibPath);
Lib := SchServer.GetSchDocumentByPath(LibPath);
if Lib = nil then begin Emit('cannot open ' + LibPath); Exit; end;
N := 0;
It := Lib.SchLibIterator_Create;
It.AddFilter_ObjectSet(MkSet(eSchComponent));
Comp := It.FirstSchObject;
while Comp <> nil do
begin
    N := N + 1;
    Emit('symbol ' + Comp.LibReference + ' | ' + Comp.ComponentDescription + ' | ' + Comp.Designator.Text + ' | parts ' + IntToStr(Comp.PartCount));
    if (Detail = '*') or SameText(Detail, Comp.LibReference) then
    begin
        CIt := Comp.SchIterator_Create;
        CIt.AddFilter_ObjectSet(MkSet(ePin, eParameter, eImplementation));
        Obj := CIt.FirstSchObject;
        while Obj <> nil do
        begin
            if Obj.ObjectId = ePin then
                Emit('  pin ' + Obj.Designator + ' ' + Obj.Name + ' ' + TypeName(Obj.Electrical) + ' at ' + IntToStr(CoordToMils(Obj.Location.X)) + ',' + IntToStr(CoordToMils(Obj.Location.Y)) + ' rot ' + IntToStr(Obj.Orientation * 90) + ' len ' + IntToStr(CoordToMils(Obj.PinLength)) + ' part ' + IntToStr(Obj.OwnerPartId))
            else if Obj.ObjectId = eParameter then
                Emit('  param ' + Obj.Name + ' = ' + Obj.Text)
            else if Obj.ObjectId = eImplementation then
            begin
                S := '  model ' + Obj.ModelType + ' ' + Obj.ModelName;
                if Obj.IsCurrent then S := S + ' (current)';
                if Obj.DatafileLinkCount > 0 then S := S + ' in ' + Obj.DatafileLink(0).Location;
                Emit(S);
            end;
            Obj := CIt.NextSchObject;
        end;
        Comp.SchIterator_Destroy(CIt);
    end;
    Comp := It.NextSchObject;
end;
Lib.SchIterator_Destroy(It);
Emit('symbols ' + IntToStr(N));
