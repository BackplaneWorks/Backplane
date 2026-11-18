// The libraries Altium offers this project (installed ones from the Components panel, and the project's own), and the
// symbols in its .SchLib libraries whose name or description contains a word. Place a found part with sch_place_part.pas
// (its library path and symbol name). Integrated libraries (.IntLib) are listed but not searched: place from one by exact
// symbol name. Online parts (Manufacturer Part Search, workspace libraries) are not reachable from a script: see SKILL.md.
var
    M : IIntegratedLibraryManager;
    SD : IServerDocument;
    Lib : ISch_Lib;
    It : ISch_Iterator;
    Comp : ISch_Component;
    Paths : TStringList;
    I, Hits : Integer;
    P, W : String;
const
    // ---- edit: placeholders ----
    Find = 'LDO';                 // a word in the symbol's name or description, any case; '' = list the libraries only
M := IntegratedLibraryManager;
Paths := TStringList.Create;
for I := 0 to M.InstalledLibraryCount - 1 do if Paths.IndexOf(M.InstalledLibraryPath(I)) < 0 then Paths.Add(M.InstalledLibraryPath(I));
for I := 0 to M.AvailableLibraryCount - 1 do if Paths.IndexOf(M.AvailableLibraryPath(I)) < 0 then Paths.Add(M.AvailableLibraryPath(I));
for I := 0 to Paths.Count - 1 do Emit('library ' + Paths[I]);
Hits := 0;
W := LowerCase(Find);
if W <> '' then
    for I := 0 to Paths.Count - 1 do
    begin
        P := Paths[I];
        if SameText(ExtractFileExt(P), '.SchLib') then
        begin
            SD := Client.GetDocumentByPath(P);
            if SD = nil then SD := Client.OpenDocument('SCHLIB', P);
            Lib := SchServer.GetSchDocumentByPath(P);
            if Lib <> nil then
            begin
                It := Lib.SchLibIterator_Create;
                It.AddFilter_ObjectSet(MkSet(eSchComponent));
                Comp := It.FirstSchObject;
                while Comp <> nil do
                begin
                    if (Pos(W, LowerCase(Comp.LibReference)) > 0) or (Pos(W, LowerCase(Comp.ComponentDescription)) > 0) then
                    begin
                        Hits := Hits + 1;
                        Emit('found ' + Comp.LibReference + ' | ' + Comp.ComponentDescription + ' | in ' + P);
                    end;
                    Comp := It.NextSchObject;
                end;
                Lib.SchIterator_Destroy(It);
            end;
        end;
    end;
Emit('libraries ' + IntToStr(Paths.Count) + ', symbols found ' + IntToStr(Hits));
Paths.Free;
