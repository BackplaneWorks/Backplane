// Close a document by path. Refuses when it has unsaved edits unless Discard is True (then they are dropped).
// Close only documents you opened yourself: never the person's.
var
    SD : IServerDocument;
const
    // ---- edit: placeholders ----
    DocPath = 'C:\Projects\Board\Sheet1.SchDoc';
    Discard = False;   // True: drop unsaved edits; False: refuse to close a modified document
SD := Client.GetDocumentByPath(DocPath);
if SD = nil then begin Emit('not open: ' + DocPath); Exit; end;
if (SD.Modified <> False) and not Discard then begin Emit('modified, not closed (save it first, or set Discard): ' + DocPath); Exit; end;
SD.Modified := False;
Client.CloseDocument(SD);
if Client.GetDocumentByPath(DocPath) = nil then Emit('closed ' + DocPath) else Emit('still open ' + DocPath);
