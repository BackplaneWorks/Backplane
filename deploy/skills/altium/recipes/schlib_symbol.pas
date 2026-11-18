// Create a schematic symbol in a .SchLib (the library is made when the file does not exist; a symbol of the same name is
// replaced): a rectangular body, pins on its four sides with numbers, names and electrical types, parameters, and a
// footprint link. Pins sit on a 100 mil grid, so wires drawn to their ends connect. Read it back with schlib_list.pas,
// save it with save_document.pas.
var
    SD : IServerDocument;
    Lib : ISch_Lib;
    Comp, Old : ISch_Component;
    It : ISch_Iterator;
    Rect : ISch_Rectangle;
    Pin : ISch_Pin;
    Par : ISch_Parameter;
    Impl : ISch_Implementation;
    Rows, Fields : TStringList;
    I, NL, NR, NT, NB, Rows2, Cols, W, H, K, Slot, TopM, X0, LenL, LenR, LenT : Integer;
    Side, Kind : String;
    Made : Boolean;
    Dflt : ISch_Component;
const
    // ---- edit: placeholders ----
    LibPath = 'C:\Projects\Board\Board.SchLib';
    SymName = 'LM1084-ADJ';                        // the symbol's name (Library Reference)
    Description = 'LDO regulator, 5 A, adjustable';
    DesignatorText = 'U?';
    CommentText = 'LM1084-ADJ';
    // pins: number|name|type|side, ';' between pins, listed top to bottom (left/right sides) or left to right (top/bottom).
    // type: in, out, io, pas, pwr, oc (open collector), oe (open emitter), hiz. side: L, R, T, B.
    Pins = '3|VIN|pwr|L;1|ADJ|in|B;2|VOUT|pwr|R;4|TAB|pas|R';
    WidthMils = 0;                                 // body width; 0 = from the pins on top/bottom (at least 600)
    PinLenMils = 200;
    // parameters: name=value, ';' between them (shown hidden on the sheet)
    Params = 'Manufacturer=Texas Instruments;Manufacturer Part Number=LM1084IS-ADJ/NOPB';
    Footprint = 'TO-263-3';                        // footprint name in the .PcbLib; '' for none
    FootprintLib = 'Board.PcbLib';                 // the .PcbLib that holds it (a name the project finds, or a full path)
    PrjPath = '';                                  // a .PrjPcb to add a new library to ('' = leave the project alone)
function PinType(T : String) : Integer;
begin
    T := LowerCase(T);
    if T = 'in' then Result := eElectricInput
    else if T = 'out' then Result := eElectricOutput
    else if T = 'io' then Result := eElectricIO
    else if T = 'pwr' then Result := eElectricPower
    else if T = 'oc' then Result := eElectricOpenCollector
    else if T = 'oe' then Result := eElectricOpenEmitter
    else if T = 'hiz' then Result := eElectricHiZ
    else Result := eElectricPassive;
end;
// Location is the pin's end on the body; the pin points away from the body by its orientation, its free end is the
// electrical one
procedure AddPin(Num, Name, T, S : String; N : Integer);
begin
    Pin := SchServer.SchObjectFactory(ePin, eCreate_Default);
    Pin.Designator := Num;
    Pin.Name := Name;
    Pin.Electrical := PinType(T);
    Pin.PinLength := MilsToCoord(PinLenMils);
    if S = 'L' then begin Pin.Orientation := eRotate180; Pin.Location := Point(0, MilsToCoord(H - TopM - 100 * N)); end
    else if S = 'R' then begin Pin.Orientation := eRotate0; Pin.Location := Point(MilsToCoord(W), MilsToCoord(H - TopM - 100 * N)); end
    else if S = 'T' then begin Pin.Orientation := eRotate90; Pin.Location := Point(MilsToCoord(X0 + 100 * (N - 1)), MilsToCoord(H)); end
    else begin Pin.Orientation := eRotate270; Pin.Location := Point(MilsToCoord(X0 + 100 * (N - 1)), 0); end;
    Pin.OwnerPartId := Comp.CurrentPartID;
    Pin.OwnerPartDisplayMode := Comp.DisplayMode;
    Comp.AddSchObject(Pin);
end;
Made := False;
SD := Client.GetDocumentByPath(LibPath);
if (SD = nil) and FileExists(LibPath) then SD := Client.OpenDocument('SCHLIB', LibPath);
if SD = nil then
begin
    // a new library has no file until it is renamed and saved (DoFileSave alone on a new document opens a Save As box)
    SD := Client.OpenNewDocument('SCHLIB', '', ExtractFileName(LibPath), False);
    if SD = nil then begin Emit('could not make ' + LibPath); Exit; end;
    SD.DoSafeChangeFileNameAndSave(LibPath, 'SCHLIB');
    Made := True;
end;
Lib := SchServer.GetSchDocumentByPath(LibPath);
if Lib = nil then begin Emit('cannot open ' + LibPath); Exit; end;
Rows := TStringList.Create; Rows.Delimiter := ';'; Rows.StrictDelimiter := True; Rows.DelimitedText := Pins;
Fields := TStringList.Create; Fields.Delimiter := '|'; Fields.StrictDelimiter := True;
// the body's size from the pins on each side and the length of their names (about 60 mil a character)
NL := 0; NR := 0; NT := 0; NB := 0; LenL := 0; LenR := 0; LenT := 0;
for I := 0 to Rows.Count - 1 do
begin
    Fields.DelimitedText := Rows[I];
    if Fields.Count <> 4 then begin Emit('pin "' + Rows[I] + '": number|name|type|side expected'); Exit; end;
    Side := UpperCase(Trim(Fields[3]));
    K := Length(Trim(Fields[1]));
    if Side = 'L' then begin NL := NL + 1; if K > LenL then LenL := K; end
    else if Side = 'R' then begin NR := NR + 1; if K > LenR then LenR := K; end
    else begin if Side = 'T' then NT := NT + 1 else NB := NB + 1; if K > LenT then LenT := K; end;
end;
Rows2 := NL; if NR > Rows2 then Rows2 := NR;
Cols := NT; if NB > Cols then Cols := NB;
// top and bottom pins write their names inward: room for them above and below the side pins
TopM := 0; if NT > 0 then TopM := 100 * ((LenT * 60 + 99) div 100);
H := 100 * (Rows2 + 1) + TopM; if NB > 0 then H := H + 100 * ((LenT * 60 + 99) div 100);
if H < 300 then H := 300;
W := WidthMils;
if W = 0 then
begin
    W := 100 * (Cols + 1);
    K := 100 * (((LenL + LenR) * 60 + 200 + 99) div 100);
    if K > W then W := K;
end;
if W < 600 then W := 600;
// top and bottom pins centred on the body, on the grid
X0 := 100 * ((W - 100 * (Cols - 1)) div 200);
// a symbol of the same name: replaced (collected first, then removed)
Old := nil;
Dflt := nil;
It := Lib.SchLibIterator_Create;
It.AddFilter_ObjectSet(MkSet(eSchComponent));
Comp := It.FirstSchObject;
while Comp <> nil do
begin
    if SameText(Comp.LibReference, SymName) then Old := Comp;
    if Made then Dflt := Comp;   // a new library's empty Component_1 goes
    Comp := It.NextSchObject;
end;
Lib.SchIterator_Destroy(It);
SchServer.ProcessControl.PreProcess(Lib, '');
if Old <> nil then
begin
    Lib.RemoveSchComponent(Old);
    Emit('replaced the old ' + SymName);
end;
Comp := SchServer.SchObjectFactory(eSchComponent, eCreate_Default);
Comp.CurrentPartID := 1;
Comp.DisplayMode := 0;
Comp.PartCount := 1;          // a new symbol says 0 parts otherwise
Comp.LibReference := SymName;
Comp.ComponentDescription := Description;
Comp.Designator.Text := DesignatorText;
Comp.Comment.Text := CommentText;
Rect := SchServer.SchObjectFactory(eRectangle, eCreate_Default);
Rect.Location := Point(0, 0);
Rect.Corner := Point(MilsToCoord(W), MilsToCoord(H));
Rect.LineWidth := eSmall;
Rect.IsSolid := True;
Rect.OwnerPartId := Comp.CurrentPartID;
Rect.OwnerPartDisplayMode := Comp.DisplayMode;
Comp.AddSchObject(Rect);
NL := 0; NR := 0; NT := 0; NB := 0;
for I := 0 to Rows.Count - 1 do
begin
    Fields.DelimitedText := Rows[I];
    Side := UpperCase(Trim(Fields[3]));
    if Side = 'L' then begin NL := NL + 1; Slot := NL; end
    else if Side = 'R' then begin NR := NR + 1; Slot := NR; end
    else if Side = 'T' then begin NT := NT + 1; Slot := NT; end
    else begin NB := NB + 1; Slot := NB; end;
    AddPin(Trim(Fields[0]), Trim(Fields[1]), Trim(Fields[2]), Side, Slot);
end;
Rows.DelimitedText := Params;
for I := 0 to Rows.Count - 1 do
begin
    K := Pos('=', Rows[I]);
    if K > 0 then
    begin
        Par := SchServer.SchObjectFactory(eParameter, eCreate_Default);
        Par.Name := Trim(Copy(Rows[I], 1, K - 1));
        Par.Text := Trim(Copy(Rows[I], K + 1, 1000));
        Par.IsHidden := True;
        Par.Location := Point(0, MilsToCoord(-100 * (I + 1)));
        Par.OwnerPartId := 0;
        Comp.AddSchObject(Par);
    end;
end;
if Footprint <> '' then
begin
    Impl := Comp.AddSchImplementation;
    Impl.ClearAllDatafileLinks;
    Impl.Description := 'Footprint';
    Impl.ModelName := Footprint;
    Impl.ModelType := 'PCBLIB';
    Impl.AddDataFileLink(Footprint, FootprintLib, 'PCBLIB');
    Impl.IsCurrent := True;
end;
Lib.AddSchComponent(Comp);
SchServer.RobotManager.SendMessage(nil, c_BroadCast, SCHM_PrimitiveRegistration, Comp.I_ObjectAddress);
Lib.CurrentSchComponent := Comp;
if Dflt <> nil then Lib.RemoveSchComponent(Dflt);
SchServer.ProcessControl.PostProcess(Lib, '');
Lib.GraphicallyInvalidate;
if Made then
begin
    SD.DoFileSave('');
    if PrjPath <> '' then
    begin
        GetWorkspace.DM_OpenProject(PrjPath, True).DM_AddSourceDocument(LibPath);
        Emit('added ' + ExtractFileName(LibPath) + ' to ' + ExtractFileName(PrjPath) + ' (save the project to keep it)');
    end;
    Emit('made ' + LibPath + ': ' + BoolToStr(FileExists(LibPath), True));
end;
Emit('symbol ' + Comp.LibReference + ': body ' + IntToStr(W) + ' x ' + IntToStr(H) + ' mil, pins ' + IntToStr(NL + NR + NT + NB) + ', footprint ' + Footprint + '; modified=' + BoolToStr(SD.Modified, True));
Rows.Free; Fields.Free;
