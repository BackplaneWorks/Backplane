// IPC-7351 footprints from datasheet dimensions with Altium's own IPC Compliant Footprints Batch generator, no person:
// the script writes the package data as the generator's CSV, BpExpect has the hub work the generator's window (add the
// file, put the footprints in this library, no report, Start), and the footprints land in the .PcbLib with pads, silk,
// assembly, courtyard and a 3D body, named by IPC rules unless FootprintName is given. Columns per package type:
// ipc_batch_columns.txt (beside this recipe). The library must exist (pcblib_footprint.pas makes one) and is brought to
// the front (the generator works on the library in front). Check with pcblib_list.pas; save with save_document.pas.
var
    SD : IServerDocument;
    Lines, Rows : TStringList;
    I : Integer;
    CsvPath : String;
const
    // ---- edit: placeholders ----
    LibPath = 'C:\Projects\Board\Board.PcbLib';
    Package = 'SOIC';              // the package line's first word, from ipc_batch_columns.txt
    Columns = 'FootprintName,FootprintDescription,Hmin,Hmax,Bmin,Bmax,Lmin,Lmax,Emin,Emax,Dmin,Dmax,Amin,Amax,A1min,A1max,A2min,A2max,PinCount,Pitch,AbsentPins,PinOrder,cmin,cmax,D2min,D2max,E2min,E2max,DensityLevel,JTmin,JHmin,JSmin,DrawingNote,Z,C,G,X,Y,E2t,D2t,A,B,R1,R2,V1,V2,PadTrimming';
    // one footprint per line, ';' between lines, columns as above (no commas inside a value)
    Data = 'SOIC127P600X175-8N,SOIC-8 (TI D) IPC nominal,5.8,6.2,0.31,0.51,0.4,1.27,3.8,4.0,4.8,5.0,1.35,1.75,0.1,0.25,,,8,1.27,,,,,,,,,N,,,,,,,,,,,,,,,,,,';
    Folder = 'C:\Users\Public\Backplane\altium\';   // where the CSV is written (no spaces: the file box gets it typed)
SD := Client.GetDocumentByPath(LibPath);
if (SD = nil) and FileExists(LibPath) then SD := Client.OpenDocument('PCBLIB', LibPath);
if SD = nil then begin Emit('no library ' + LibPath + ' (make it with pcblib_footprint.pas first)'); Exit; end;
Client.ShowDocument(SD);
if PCBServer.GetCurrentPCBLibrary = nil then begin Emit('could not bring ' + LibPath + ' to the front'); Exit; end;
if not SameText(PCBServer.GetCurrentPCBLibrary.Board.FileName, LibPath) then begin Emit('could not bring ' + LibPath + ' to the front'); Exit; end;
CsvPath := Folder + 'ipc_batch_' + Package + '.csv';
Lines := TStringList.Create;
Lines.Add(Package + ',Package Specifications');
Lines.Add(Columns);
Rows := TStringList.Create; Rows.Delimiter := ';'; Rows.StrictDelimiter := True; Rows.DelimitedText := Data;
for I := 0 to Rows.Count - 1 do Lines.Add(Rows[I]);
Lines.SaveToFile(CsvPath);
Emit('wrote ' + CsvPath + ' (' + IntToStr(Rows.Count) + ' footprints)');
// the generator's window: add the CSV through its file box, choose "Generate all footprints in <this library>", turn the
// report off, Start; it closes itself when done
BpExpect('Batch generator', 'click Add Files; window Open; text 1 = ' + CsvPath + '; click Open; window Batch generator; ' +
    'click Generate all footprints in*; click Generate report on completion; click Start; gone 300');
ResetParameters;
RunProcess('PCB:BatchPCBLibCreate');
Emit('generator closed; modified=' + BoolToStr(SD.Modified, True));
Lines.Free; Rows.Free;
