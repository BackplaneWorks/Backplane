// List a schematic's components: designator, comment, library ref and library, footprint, location, rotation, parameters.
// Designators are as drawn (C1); compiled ones of a multichannel design (C1_CH1) come from project_netlist.pas.
var
    SD : IServerDocument;
    Sch : ISch_Document;
    It, PIt : ISch_Iterator;
    Comp : ISch_Component;
    Par : ISch_Parameter;
    Impl : ISch_Implementation;
    S, Fp : String;
const
    // ---- edit: placeholders ----
    SchPath = 'C:\Projects\Board\Sheet1.SchDoc';
SD := Client.GetDocumentByPath(SchPath);
if SD = nil then SD := Client.OpenDocument('SCH', SchPath);
Sch := SchServer.GetSchDocumentByPath(SchPath);
if Sch = nil then begin Emit('not a schematic: ' + SchPath); Exit; end;
It := Sch.SchIterator_Create;
It.AddFilter_ObjectSet(MkSet(eSchComponent));
Comp := It.FirstSchObject;
while Comp <> nil do
begin
    Fp := '';
    PIt := Comp.SchIterator_Create;
    PIt.AddFilter_ObjectSet(MkSet(eImplementation));
    Impl := PIt.FirstSchObject;
    while Impl <> nil do
    begin
        if (Impl.ModelType = 'PCBLIB') and Impl.IsCurrent then Fp := Impl.ModelName;
        Impl := PIt.NextSchObject;
    end;
    Comp.SchIterator_Destroy(PIt);
    S := Comp.Designator.Text + ' | ' + Comp.Comment.Text + ' | lib ' + Comp.LibReference + ' (' + Comp.SourceLibraryName + ') | fp ' + Fp
        + ' | at ' + IntToStr(CoordToMils(Comp.Location.X)) + ',' + IntToStr(CoordToMils(Comp.Location.Y)) + ' mil rot ' + IntToStr(Comp.Orientation * 90);
    PIt := Comp.SchIterator_Create;
    PIt.AddFilter_ObjectSet(MkSet(eParameter));
    Par := PIt.FirstSchObject;
    while Par <> nil do
    begin
        S := S + ' | ' + Par.Name + '=' + Par.Text;
        Par := PIt.NextSchObject;
    end;
    Comp.SchIterator_Destroy(PIt);
    Emit(S);
    Comp := It.NextSchObject;
end;
Sch.SchIterator_Destroy(It);
