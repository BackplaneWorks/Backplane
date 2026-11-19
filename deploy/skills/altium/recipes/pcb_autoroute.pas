// Autoroute with Situs and read back how far it got (unrouted connections, tracks and vias added). What it routes:
//   Mode 'all'      the whole board. Situs asks for a strategy in a modal dialog; BpExpect has the hub press Route All
//                   (the default strategy for the board's layer count).
//   Mode 'between'  only connections between the components named (Route > Auto Route > Connections Between Selected
//                   Components; no dialog). Two components = the connections joining them.
//   Mode 'on'       every connection of the components named (Connections On Selected Components; no dialog).
// Nets (any mode): route only these nets. A temporary Routing Layers rule (BP_RouteOnly, top priority, every other net,
// no layers) stops Situs on the others; it is removed again at the end. Nets + 'between' = selected connections.
// Rules (width, clearance, via style, layers) come from the board: set them first. Undo in Altium undoes the routing.
// Give the run a long timeout: a small board takes seconds, a dense one minutes. Board in front required (checked).
var
    SD : IServerDocument;
    Board : IPCB_Board;
    Rule : IPCB_Rule;
    Comp : IPCB_Component;
    Parts, NetList : TStringList;
    C0, C1, T0, T1, V0, V1, I, L : Integer;
    Expr : String;
const
    // ---- edit: placeholders ----
    PcbPath = 'C:\Projects\Board\Board.PcbDoc';
    Mode = 'all';                  // 'all', 'between' or 'on'
    Designators = '';              // for 'between' / 'on': 'U1;C3', ';' between them
    Nets = '';                     // '' = every net; else only these: 'GND;+3V3'
function IsCurrent(Path : String) : Boolean;
var B : IPCB_Board;
begin
    Result := False;
    B := PCBServer.GetCurrentPCBBoard;
    if B <> nil then Result := SameText(B.FileName, Path);
end;
function Count(Kind : Integer) : Integer;
var It : IPCB_BoardIterator; O : IPCB_Primitive;
begin
    Result := 0;
    It := Board.BoardIterator_Create;
    It.AddFilter_ObjectSet(MkSet(Kind));
    It.AddFilter_LayerSet(AllLayers);
    It.AddFilter_Method(eProcessAll);
    O := It.FirstPCBObject;
    while O <> nil do begin Result := Result + 1; O := It.NextPCBObject; end;
    Board.BoardIterator_Destroy(It);
end;
// the nets still unrouted, each with its number of connections left
function Left : String;
var It : IPCB_BoardIterator; C : IPCB_Connection; T : TStringList; K : Integer;
begin
    T := TStringList.Create;
    It := Board.BoardIterator_Create;
    It.AddFilter_ObjectSet(MkSet(eConnectionObject));
    It.AddFilter_LayerSet(AllLayers);
    It.AddFilter_Method(eProcessAll);
    C := It.FirstPCBObject;
    while C <> nil do
    begin
        if C.Net <> nil then
        begin
            K := T.IndexOfName(C.Net.Name);
            if K < 0 then T.Add(C.Net.Name + '=1') else T.ValueFromIndex[K] := IntToStr(StrToInt(T.ValueFromIndex[K]) + 1);
        end;
        C := It.NextPCBObject;
    end;
    Board.BoardIterator_Destroy(It);
    T.Sort;
    Result := T.CommaText;
    T.Free;
end;
procedure DeselectAll;
begin
    ResetParameters;
    AddStringParameter('Scope', 'All');
    RunProcess('PCB:DeSelect');
end;
SD := Client.GetDocumentByPath(PcbPath);
if SD = nil then SD := Client.OpenDocument('PCB', PcbPath);
Board := PCBServer.GetPCBBoardByPath(PcbPath);
if Board = nil then begin Emit('not open: ' + PcbPath); Exit; end;
// Situs routes the board in front: make this one current first, and check
if not IsCurrent(PcbPath) then Client.ShowDocument(SD);
if not IsCurrent(PcbPath) then begin Emit('could not make ' + PcbPath + ' the current board'); Exit; end;
Parts := TStringList.Create; Parts.Delimiter := ';'; Parts.StrictDelimiter := True; Parts.DelimitedText := Designators;
NetList := TStringList.Create; NetList.Delimiter := ';'; NetList.StrictDelimiter := True; NetList.DelimitedText := Nets;
if (Mode <> 'all') and (Parts.Count = 0) then begin Emit('mode ' + Mode + ' needs Designators'); Exit; end;
C0 := Count(eConnectionObject); T0 := Count(eTrackObject); V0 := Count(eViaObject);
Emit('before: unrouted connections ' + IntToStr(C0) + ', tracks ' + IntToStr(T0) + ', vias ' + IntToStr(V0));
// only these nets: every other net gets a top-priority rule with no layers to route on
Rule := nil;
if NetList.Count > 0 then
begin
    Expr := '';
    for I := 0 to NetList.Count - 1 do
    begin
        if Expr <> '' then Expr := Expr + ' or ';
        Expr := Expr + 'InNet(''' + Trim(NetList[I]) + ''')';
    end;
    PCBServer.PreProcess;
    Rule := PCBServer.PCBRuleFactory(eRule_RoutingLayers);
    Rule.Name := 'BP_RouteOnly';
    Rule.Comment := 'Backplane: temporary, while Situs routes only some nets';
    Rule.Scope1Expression := 'Not (' + Expr + ')';
    for L := eTopLayer to eBottomLayer do Rule.RoutingLayers[L] := False;
    Board.AddPCBObject(Rule);
    PCBServer.SendMessageToRobots(Board.I_ObjectAddress, c_Broadcast, PCBM_BoardRegisteration, Rule.I_ObjectAddress);
    PCBServer.PostProcess;
    // it must outrank every other routing-layers rule, or the other nets still route
    if Rule.Priority <> 1 then
    begin
        PCBServer.PreProcess;
        Board.RemovePCBObject(Rule);
        PCBServer.SendMessageToRobots(Board.I_ObjectAddress, c_Broadcast, PCBM_BoardRegisteration, Rule.I_ObjectAddress);
        PCBServer.PostProcess;
        BpFail('the temporary rule got priority ' + IntToStr(Rule.Priority) + ', not 1: removed it, routed nothing');
        Exit;
    end;
    Emit('only nets ' + Nets + ' (rule ' + Rule.Name + ', priority 1)');
end;
ResetParameters;
if Mode = 'all' then
begin
    BpExpect('Situs Routing Strategies', 'click Route All');
    AddStringParameter('Action', 'Start');
end
else
begin
    // Situs reads the components selected on the board
    DeselectAll;
    for I := 0 to Parts.Count - 1 do
    begin
        Comp := Board.GetPcbComponentByRefDes(Trim(Parts[I]));
        if Comp = nil then Emit('no component ' + Parts[I]) else Comp.Selected := True;
    end;
    ResetParameters;
    if Mode = 'between' then AddStringParameter('Action', 'BetweenSelectedComponents')
    else AddStringParameter('Action', 'OnSelectedComponents');
end;
// the temporary rule goes whatever happens while routing
try
    RunProcess('PCB:Autoroute');
finally
    if Mode <> 'all' then DeselectAll;
    if Rule <> nil then
    begin
        PCBServer.PreProcess;
        Board.RemovePCBObject(Rule);
        PCBServer.SendMessageToRobots(Board.I_ObjectAddress, c_Broadcast, PCBM_BoardRegisteration, Rule.I_ObjectAddress);
        PCBServer.PostProcess;
        Emit('removed the temporary rule ' + Rule.Name);
    end;
end;
C1 := Count(eConnectionObject); T1 := Count(eTrackObject); V1 := Count(eViaObject);
Emit('after: unrouted connections ' + IntToStr(C1) + ', tracks ' + IntToStr(T1) + ' (+' + IntToStr(T1 - T0) + '), vias ' + IntToStr(V1) + ' (+' + IntToStr(V1 - V0) + ')');
if C1 > 0 then Emit('unrouted nets (net=connections): ' + Left);
Emit('routed ' + IntToStr(C0 - C1) + ' of ' + IntToStr(C0) + ' connections; modified=' + BoolToStr(SD.Modified, True));
Parts.Free; NetList.Free;
