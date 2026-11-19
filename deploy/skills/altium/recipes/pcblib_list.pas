// List a .PcbLib's footprints: name, height, primitive counts, and (for those asked for) every pad (name, x/y mm from the
// footprint origin, size, shape, hole, layer, rotation), each line or arc's layer, and each 3D body (kind, height, layer).
var
    SD : IServerDocument;
    Lib : IPCB_Library;
    LIt : IPCB_LibraryIterator;
    GIt : IPCB_GroupIterator;
    FP : IPCB_LibComponent;
    P : IPCB_Primitive;
    N, NPad, NTrack, NArc, NBody, NOther : Integer;
    OX, OY : Integer;
    Show : Boolean;
    Det : TStringList;
    K : Integer;
    R : TCoordRect;
const
    // ---- edit: placeholders ----
    LibPath = 'C:\Projects\Board\Board.PcbLib';
    Detail = '*';                    // footprints to list primitive by primitive: '*' = all, else one name; '' = summary only
function F2(C : Integer) : String;
begin
    Result := BpFmt('0.###', CoordToMMs(C));
end;
function ShapeName(S : Integer) : String;
begin
    if S = eRectangular then Result := 'rect'
    else if S = eRounded then Result := 'round'
    else if S = eOctagonal then Result := 'oct'
    else if S = eRoundedRectangular then Result := 'roundrect'
    else Result := IntToStr(S);
end;
SD := Client.GetDocumentByPath(LibPath);
if SD = nil then SD := Client.OpenDocument('PCBLIB', LibPath);
Lib := PCBServer.GetPCBLibraryByPath(LibPath);
if Lib = nil then begin Emit('cannot open ' + LibPath); Exit; end;
Det := TStringList.Create;
N := 0;
LIt := Lib.LibraryIterator_Create;
LIt.SetState_FilterAll;
FP := LIt.FirstPCBObject;
while FP <> nil do
begin
    N := N + 1;
    Show := (Detail = '*') or SameText(Detail, FP.Name);
    // a footprint's primitives are stored around its own origin (the one the editor shows is moved to the library's origin)
    OX := FP.X;
    OY := FP.Y;
    NPad := 0; NTrack := 0; NArc := 0; NBody := 0; NOther := 0;
    GIt := FP.GroupIterator_Create;
    GIt.AddFilter_ObjectSet(AllObjects);
    P := GIt.FirstPCBObject;
    while P <> nil do
    begin
        if P.ObjectId = ePadObject then
        begin
            NPad := NPad + 1;
            if Show then Det.Add('  pad ' + P.Name + ' at ' + F2(P.X - OX) + ',' + F2(P.Y - OY) + ' size ' + F2(P.TopXSize) + 'x' + F2(P.TopYSize) + ' ' + ShapeName(P.TopShape) + ' hole ' + F2(P.HoleSize) + ' ' + Layer2String(P.Layer) + ' rot ' + BpFmt('0.#', P.Rotation));
        end
        else if P.ObjectId = eTrackObject then
        begin
            NTrack := NTrack + 1;
            if Show then Det.Add('  line ' + Layer2String(P.Layer) + ' ' + F2(P.X1 - OX) + ',' + F2(P.Y1 - OY) + ' - ' + F2(P.X2 - OX) + ',' + F2(P.Y2 - OY) + ' w ' + F2(P.Width));
        end
        else if P.ObjectId = eArcObject then NArc := NArc + 1
        else if P.ObjectId = eComponentBodyObject then
        begin
            NBody := NBody + 1;
            if Show then
            begin
                R := P.BoundingRectangle;
                Det.Add('  body ' + Layer2String(P.Layer) + ' model kind ' + IntToStr(P.Model.ModelType) + ' over ' + F2(R.Left - OX) + ',' + F2(R.Bottom - OY) + ' - ' + F2(R.Right - OX) + ',' + F2(R.Top - OY) + ' standoff ' + F2(P.StandoffHeight) + ' height ' + F2(P.OverallHeight));
            end;
        end
        else NOther := NOther + 1;
        P := GIt.NextPCBObject;
    end;
    FP.GroupIterator_Destroy(GIt);
    Emit('footprint ' + FP.Name + ' | height ' + F2(FP.Height) + ' | pads ' + IntToStr(NPad) + ' lines ' + IntToStr(NTrack) + ' arcs ' + IntToStr(NArc) + ' bodies ' + IntToStr(NBody) + ' other ' + IntToStr(NOther));
    for K := 0 to Det.Count - 1 do Emit(Det[K]);
    Det.Clear;
    FP := LIt.NextPCBObject;
end;
Lib.LibraryIterator_Destroy(LIt);
Emit('footprints ' + IntToStr(N));
Det.Free;
