// Compile a project and list its nets with their pins (physical designators, e.g. R1_CH1 in a multichannel design)
// and the compiler/ERC messages with their objects. Compiling changes no file.
var
    Prj : IProject;
    Flat : IDocument;
    Net : INet;
    Pin : IPin;
    V : IViolation;
    O : IDMObject;
    K : Integer;
    I, J : Integer;
    S : String;
const
    // ---- edit: placeholders ----
    PrjPath = 'C:\Projects\Board\Board.PrjPcb';
Prj := GetWorkspace.DM_OpenProject(PrjPath, True);
Prj.DM_Compile;
Flat := Prj.DM_DocumentFlattened;
if Flat = nil then begin Emit('no flattened netlist: compile failed?'); Exit; end;
Emit('nets ' + IntToStr(Flat.DM_NetCount));
for I := 0 to Flat.DM_NetCount - 1 do
begin
    Net := Flat.DM_Nets(I);
    S := Net.DM_NetName + ':';
    for J := 0 to Net.DM_PinCount - 1 do
    begin
        Pin := Net.DM_Pins(J);
        S := S + ' ' + Pin.DM_PhysicalPartDesignator + '.' + Pin.DM_PinNumber;
    end;
    Emit(S);
end;
Emit('violations ' + IntToStr(Prj.DM_ViolationCount));
for I := 0 to Prj.DM_ViolationCount - 1 do
begin
    V := Prj.DM_Violations(I);
    // DM_ErrorLevel: 0 no report, 1 warning, 2 error, 3 fatal
    S := IntToStr(V.DM_ErrorLevel) + ' | ' + V.DM_ShortDescriptorString + ' | ' + V.DM_DetailString;
    for K := 0 to V.DM_RelatedObjectCount - 1 do
    begin
        O := V.DM_RelatedObjects(K);
        S := S + ' | ' + O.DM_ObjectKindString + ' ' + O.DM_LongDescriptorString + ' (' + O.DM_OwnerDocumentName + ')';
    end;
    Emit(S);
end;
