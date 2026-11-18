// Save a document by path (schematic, board, library, OutJob, project).
var
    SD : IServerDocument;
    Ok : Boolean;
const
    // ---- edit: placeholders ----
    DocPath = 'C:\Projects\Board\Sheet1.SchDoc';
SD := Client.GetDocumentByPath(DocPath);
if SD = nil then begin Emit('not open (nothing to save): ' + DocPath); Exit; end;
Emit('modified before: ' + BoolToStr(SD.Modified, True));
Ok := SD.DoFileSave('');   // '' = the document's own format. DoFileSave(SD.Kind) returns True and writes NOTHING
Emit('saved: ' + BoolToStr(Ok, True) + '; modified after: ' + BoolToStr(SD.Modified, True));
