// Design rule check from a script, no dialog: Altium's own rule engine (IPCB_Rule.ActualCheck) for the clearance,
// short-circuit, width, hole-size and hole-to-hole rules. Matches the batch DRC's report for those rules (each pair once;
// Altium's report counts most pairs twice). Not covered: un-routed nets, polygon, silk, mask, component clearance rules.
// Read-only. Cost grows with object count; ~4 s for 500 objects.
var
    SD : IServerDocument;
    Board : IPCB_Board;
    Rules, Prims : TInterfaceList;
    It : IPCB_BoardIterator;
    SIt : IPCB_SpatialIterator;
    R : IPCB_Rule;
    P1, P2 : IPCB_Primitive;
    V : IPCB_Violation;
    I, J, K, N, Checked : Integer;
    Rect : TCoordRect;
const
    // ---- edit: placeholders ----
    PcbPath = 'C:\Projects\Board\Board.PcbDoc';
    MaxReport = 50;       // violations to print
    ReachMM = 1;          // binary rules: look for partners this far around each object (>= the largest clearance)
function Desc(P : IPCB_Primitive) : String;
begin
    Result := P.Descriptor;
    if P.Net <> nil then Result := Result + ' [' + P.Net.Name + ']';
end;
procedure Report(V : IPCB_Violation; R : IPCB_Rule; A, B : IPCB_Primitive);
var S : String;
begin
    N := N + 1;
    if N > MaxReport then Exit;
    S := R.Name + ' | ' + V.Descriptor + ' | ' + Desc(A);
    if B <> nil then S := S + ' | ' + Desc(B);
    Emit(S);
end;
SD := Client.GetDocumentByPath(PcbPath);
if SD = nil then SD := Client.OpenDocument('PCB', PcbPath);
Board := PCBServer.GetPCBBoardByPath(PcbPath);
if Board = nil then begin Emit('not open: ' + PcbPath); Exit; end;
// the enabled rules of the kinds checked here (others: routing, testpoint, fanout... are not geometric checks)
Rules := TInterfaceList.Create;
It := Board.BoardIterator_Create;
It.AddFilter_ObjectSet(MkSet(eRuleObject));
It.AddFilter_LayerSet(AllLayers);
It.AddFilter_Method(eProcessAll);
R := It.FirstPCBObject;
while R <> nil do
begin
    if R.Enabled and R.DRCEnabled and ((R.RuleKind = eRule_Clearance) or (R.RuleKind = eRule_ShortCircuit) or (R.RuleKind = eRule_MaxMinWidth)
        or (R.RuleKind = eRule_MaxMinHoleSize) or (R.RuleKind = eRule_HoleToHoleClearance)) then Rules.Add(R);
    R := It.NextPCBObject;
end;
Board.BoardIterator_Destroy(It);
// the copper objects
Prims := TInterfaceList.Create;
It := Board.BoardIterator_Create;
It.AddFilter_ObjectSet(MkSet(eTrackObject, eArcObject, eViaObject, ePadObject, eFillObject, eRegionObject));
It.AddFilter_LayerSet(AllLayers);
It.AddFilter_Method(eProcessAll);
P1 := It.FirstPCBObject;
while P1 <> nil do
begin
    // a polygon's pour regions are left out (Altium's own DRC does not report them either); its tracks and arcs stay
    if not (P1.InPolygon and (P1.ObjectId = eRegionObject)) then Prims.Add(P1);
    P1 := It.NextPCBObject;
end;
Board.BoardIterator_Destroy(It);
N := 0; Checked := 0;
for K := 0 to Rules.Count - 1 do
begin
    R := Rules.Items[K];
    for I := 0 to Prims.Count - 1 do
    begin
        P1 := Prims.Items[I];
        if R.IsUnary then
        begin
            if R.Scope1Includes(P1) then
            begin
                Checked := Checked + 1;
                V := R.ActualCheck(P1, nil);
                if V <> nil then Report(V, R, P1, nil);
            end;
        end
        else
        begin
            Rect := P1.BoundingRectangle;
            SIt := Board.SpatialIterator_Create;
            SIt.AddFilter_ObjectSet(MkSet(eTrackObject, eArcObject, eViaObject, ePadObject, eFillObject, eRegionObject));
            SIt.AddFilter_LayerSet(AllLayers);
            SIt.AddFilter_Area(Rect.Left - MMsToCoord(ReachMM), Rect.Bottom - MMsToCoord(ReachMM), Rect.Right + MMsToCoord(ReachMM), Rect.Top + MMsToCoord(ReachMM));
            P2 := SIt.FirstPCBObject;
            while P2 <> nil do
            begin
                // each pair once: by object address order
                if (P2.I_ObjectAddress > P1.I_ObjectAddress) and not (P2.InPolygon and (P2.ObjectId = eRegionObject)) and R.CheckBinaryScope(P1, P2) then
                begin
                    Checked := Checked + 1;
                    V := R.ActualCheck(P1, P2);
                    if V <> nil then Report(V, R, P1, P2);
                end;
                P2 := SIt.NextPCBObject;
            end;
            Board.SpatialIterator_Destroy(SIt);
        end;
    end;
end;
Emit('rules ' + IntToStr(Rules.Count) + ', objects ' + IntToStr(Prims.Count) + ', checks ' + IntToStr(Checked) + ', violations ' + IntToStr(N));
Rules.Free;
Prims.Free;
