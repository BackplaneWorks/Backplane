// Create a footprint in a .PcbLib (the library is made when the file does not exist; a footprint of the same name is
// replaced): SMD and through-hole pads (shapes, holes, rotation), silkscreen, assembly and courtyard lines, and a 3D
// body (an extruded block, or a STEP file embedded). Coordinates are mm from the footprint's origin (0,0 = its
// reference point; put it at the part's centre). Compute pad sizes yourself (IPC-7351 nominal); read the result back
// with pcblib_list.pas, save with save_document.pas.
var
    SD : IServerDocument;
    Lib : IPCB_Library;
    FP, Old : IPCB_LibComponent;
    Pad : IPCB_Pad;
    Track : IPCB_Track;
    Body : IPCB_ComponentBody;
    Model : IPCB_Model;
    Contour : IPCB_Contour;
    Rows, F : TStringList;
    I, NPads, NLines : Integer;
    OX, OY : Integer;
    Kind : String;
    Made : Boolean;
    Dflt : IPCB_LibComponent;
const
    // ---- edit: placeholders ----
    LibPath = 'C:\Projects\Board\Board.PcbLib';
    FpName = 'SOT-223';
    Description = 'SOT-223, 3 leads + tab, IPC nominal';
    HeightMM = 1.8;
    // pads: name|x|y|width|height|shape|hole|rotation (mm, degrees), ';' between pads.
    // shape: rect, round (round or oblong), roundrect (25 % corners), oct. hole 0 = SMD on the top layer; > 0 = through-hole, plated.
    Pads = '1|-2.3|-3.15|1.0|2.15|rect|0|0;2|0|-3.15|1.0|2.15|rect|0|0;3|2.3|-3.15|1.0|2.15|rect|0|0;4|0|3.15|3.3|2.15|rect|0|0';
    // lines: layer|x1|y1|x2|y2|width (mm), ';' between them. layer: silk (top overlay), assy, court, or mechN.
    // a layer name ending in "box" draws the rectangle x1,y1 - x2,y2.
    Lines = 'silkbox|-3.4|-1.9|3.4|1.9|0.15;assybox|-3.25|-1.75|3.25|1.75|0.1;courtbox|-3.6|-4.5|3.6|4.5|0.05';
    AssyLayer = 13;                    // the mechanical layers this library uses for assembly, courtyard and 3D bodies
    CourtLayer = 15;
    BodyLayer = 13;
    BodyBox = '-3.25|-1.75|3.25|1.75'; // the 3D body: an extruded block over this rectangle up to HeightMM; '' for none
    StandoffMM = 0;
    StepFile = '';                     // or a STEP file to embed instead of the block (full path)
    PrjPath = '';                      // a .PrjPcb to add a new library to ('' = leave the project alone)
function Mech(N : Integer) : Integer;
begin
    case N of
        1: Result := eMechanical1;  2: Result := eMechanical2;  3: Result := eMechanical3;  4: Result := eMechanical4;
        5: Result := eMechanical5;  6: Result := eMechanical6;  7: Result := eMechanical7;  8: Result := eMechanical8;
        9: Result := eMechanical9;  10: Result := eMechanical10; 11: Result := eMechanical11; 12: Result := eMechanical12;
        13: Result := eMechanical13; 14: Result := eMechanical14; 15: Result := eMechanical15;
    else Result := eMechanical16;
    end;
end;
function MM(S : String) : Integer;
begin
    Result := MMsToCoord(StrToFloat(Trim(S)));
end;
procedure Add(P : IPCB_Primitive);
begin
    FP.AddPCBObject(P);
    PCBServer.SendMessageToRobots(FP.I_ObjectAddress, c_Broadcast, PCBM_BoardRegisteration, P.I_ObjectAddress);
end;
procedure AddTrack(L : Integer; X1, Y1, X2, Y2, W : Integer);
begin
    Track := PCBServer.PCBObjectFactory(eTrackObject, eNoDimension, eCreate_Default);
    Track.X1 := OX + X1; Track.Y1 := OY + Y1; Track.X2 := OX + X2; Track.Y2 := OY + Y2;
    Track.Width := W;
    Track.Layer := L;
    Add(Track);
    NLines := NLines + 1;
end;
procedure AddPad(S : String);
var Shape : String; Hole : Double;
begin
    F.DelimitedText := S;
    if F.Count <> 8 then begin Emit('pad "' + S + '": name|x|y|width|height|shape|hole|rotation expected'); Exit; end;
    Pad := PCBServer.PCBObjectFactory(ePadObject, eNoDimension, eCreate_Default);
    Pad.Mode := ePadMode_Simple;
    Pad.Name := Trim(F[0]);
    Pad.X := OX + MM(F[1]);
    Pad.Y := OY + MM(F[2]);
    Pad.TopXSize := MM(F[3]);
    Pad.TopYSize := MM(F[4]);
    Shape := LowerCase(Trim(F[5]));
    if Shape = 'rect' then Pad.TopShape := eRectangular
    else if Shape = 'oct' then Pad.TopShape := eOctagonal
    else if Shape = 'roundrect' then Pad.TopShape := eRoundedRectangular
    else Pad.TopShape := eRounded;
    Hole := StrToFloat(Trim(F[6]));
    if Hole > 0 then
    begin
        Pad.Layer := eMultiLayer;
        Pad.HoleSize := MMsToCoord(Hole);
        Pad.Plated := True;
        Pad.MidXSize := Pad.TopXSize; Pad.MidYSize := Pad.TopYSize; Pad.MidShape := Pad.TopShape;
        Pad.BotXSize := Pad.TopXSize; Pad.BotYSize := Pad.TopYSize; Pad.BotShape := Pad.TopShape;
    end
    else
    begin
        Pad.Layer := eTopLayer;
        Pad.HoleSize := 0;
    end;
    Pad.Rotation := StrToFloat(Trim(F[7]));
    Add(Pad);
    NPads := NPads + 1;
end;
procedure AddLine(S : String);
var L : Integer; X1, Y1, X2, Y2, W : Integer;
begin
    F.DelimitedText := S;
    if F.Count <> 6 then begin Emit('line "' + S + '": layer|x1|y1|x2|y2|width expected'); Exit; end;
    Kind := LowerCase(Trim(F[0]));
    if Pos('silk', Kind) = 1 then L := eTopOverlay
    else if Pos('assy', Kind) = 1 then L := Mech(AssyLayer)
    else if Pos('court', Kind) = 1 then L := Mech(CourtLayer)
    else if Pos('mech', Kind) = 1 then L := Mech(StrToInt(Copy(Kind, 5, 2)))
    else begin Emit('line "' + S + '": unknown layer'); Exit; end;
    X1 := MM(F[1]); Y1 := MM(F[2]); X2 := MM(F[3]); Y2 := MM(F[4]); W := MM(F[5]);
    if Pos('box', Kind) > 0 then
    begin
        AddTrack(L, X1, Y1, X2, Y1, W); AddTrack(L, X2, Y1, X2, Y2, W); AddTrack(L, X2, Y2, X1, Y2, W); AddTrack(L, X1, Y2, X1, Y1, W);
    end
    else AddTrack(L, X1, Y1, X2, Y2, W);
end;
Made := False;
SD := Client.GetDocumentByPath(LibPath);
if (SD = nil) and FileExists(LibPath) then SD := Client.OpenDocument('PCBLIB', LibPath);
if SD = nil then
begin
    // a new library has no file until it is renamed and saved (DoFileSave alone on a new document opens a Save As box)
    SD := Client.OpenNewDocument('PCBLIB', '', ExtractFileName(LibPath), False);
    if SD = nil then begin Emit('could not make ' + LibPath); Exit; end;
    SD.DoSafeChangeFileNameAndSave(LibPath, 'PCBLIB');
    Made := True;
end;
Lib := PCBServer.GetPCBLibraryByPath(LibPath);
if Lib = nil then begin Emit('cannot open ' + LibPath); Exit; end;
// a new library's empty PCBComponent_1 goes once this footprint is in
Dflt := nil;
if Made then Dflt := Lib.GetComponentByName('PCBComponent_1');
// objects added to a library footprint are placed from its origin: coordinates are the footprint's own
OX := 0;
OY := 0;
Rows := TStringList.Create; Rows.Delimiter := ';'; Rows.StrictDelimiter := True;
F := TStringList.Create; F.Delimiter := '|'; F.StrictDelimiter := True;
// an old footprint of this name steps aside (renamed) and goes once the new one is in: a library left with none is not allowed
Old := Lib.GetComponentByName(FpName);
if Old <> nil then Old.Name := FpName + '-replaced';
PCBServer.PreProcess;
FP := PCBServer.CreatePCBLibComp;
FP.Name := FpName;
FP.Description := Description;
FP.Height := MMsToCoord(HeightMM);
Lib.RegisterComponent(FP);
NPads := 0; NLines := 0;
Rows.DelimitedText := Pads;
for I := 0 to Rows.Count - 1 do AddPad(Rows[I]);
Rows.DelimitedText := Lines;
for I := 0 to Rows.Count - 1 do AddLine(Rows[I]);
if (BodyBox <> '') or (StepFile <> '') then
begin
    Body := PCBServer.PCBObjectFactory(eComponentBodyObject, eNoDimension, eCreate_Default);
    Body.Layer := Mech(BodyLayer);
    Body.BodyProjection := eBoardSide_Top;
    if StepFile <> '' then
    begin
        Model := Body.ModelFactory_FromFilename(StepFile, False);
        Body.SetState_FromModel;
        Body.Model := Model;
    end
    else
    begin
        F.DelimitedText := BodyBox;
        Contour := PCBServer.PCBContourFactory;
        Contour.AddPoint(OX + MM(F[0]), OY + MM(F[1]));
        Contour.AddPoint(OX + MM(F[2]), OY + MM(F[1]));
        Contour.AddPoint(OX + MM(F[2]), OY + MM(F[3]));
        Contour.AddPoint(OX + MM(F[0]), OY + MM(F[3]));
        Body.SetOutlineContour(Contour);
        Body.StandoffHeight := MMsToCoord(StandoffMM);
        Body.OverallHeight := MMsToCoord(HeightMM);
        Model := Body.ModelFactory_CreateExtruded(MMsToCoord(StandoffMM), MMsToCoord(HeightMM), $808080);
        Body.Model := Model;
        Body.BodyColor3D := $404040;
        Body.BodyOpacity3D := 1;
    end;
    Add(Body);
end;
PCBServer.PostProcess;
if Old <> nil then
begin
    Lib.RemoveComponent(Old);
    Emit('replaced the old ' + FpName);
end;
if Dflt <> nil then Lib.RemoveComponent(Dflt);
Lib.CurrentComponent := FP;
Lib.Board.ViewManager_FullUpdate;
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
Emit('footprint ' + FP.Name + ': pads ' + IntToStr(NPads) + ', lines ' + IntToStr(NLines) + ', 3D body ' + BoolToStr((BodyBox <> '') or (StepFile <> ''), True) + '; modified=' + BoolToStr(SD.Modified, True));
Rows.Free; F.Free;
