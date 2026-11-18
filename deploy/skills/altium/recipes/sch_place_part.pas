// Place a component from a library file (.SchLib or .IntLib) onto a schematic.
var
    SD : IServerDocument;
    Sch : ISch_Document;
    Comp : ISch_Component;
    I : Integer;
    Uid : String;
const
    // ---- edit: placeholders ----
    SchPath = 'C:\Projects\Board\Sheet1.SchDoc';
    LibPath = 'C:\Projects\Board\Board.SchLib';   // a .SchLib (or .IntLib) file
    LibRef = 'CAP_0402';                            // the symbol's name in that library
    Designator = 'C9';
    Comment = '10n';
    XMils = 3000;
    YMils = 2000;
SD := Client.GetDocumentByPath(SchPath);
if SD = nil then SD := Client.OpenDocument('SCH', SchPath);
Sch := SchServer.GetSchDocumentByPath(SchPath);
if Sch = nil then begin Emit('not open: ' + SchPath); Exit; end;
Comp := SchServer.LoadComponentFromLibrary(LibRef, LibPath);
if Comp = nil then begin Emit('no ' + LibRef + ' in ' + LibPath); Exit; end;
SchServer.ProcessControl.PreProcess(Sch, '');
Comp.Designator.Text := Designator;
Comp.Comment.Text := Comment;
Comp.SourceLibraryName := ExtractFileName(LibPath);   // else it reads '*'
Sch.RegisterSchObjectInContainer(Comp);
// move it only once it is on the sheet: setting Location before moves the component but leaves its pins and graphics at 0,0
Comp.MoveToXY(MilsToCoord(XMils), MilsToCoord(YMils));
Comp.RotateBy90(Comp.Location, eRotate0);   // eRotate90 / eRotate180 / eRotate270 to turn it
SchServer.RobotManager.SendMessage(Sch.I_ObjectAddress, c_BroadCast, SCHM_PrimitiveRegistration, Comp.I_ObjectAddress);
// every part placed from one library symbol comes with that symbol's UniqueId, and the compiler merges parts that share
// one: give this one its own (bracketed as a change, or the new id is not kept)
Randomize;
Uid := '';
for I := 1 to 8 do Uid := Uid + Chr(Ord('A') + Random(26));
SchServer.RobotManager.SendMessage(Comp.I_ObjectAddress, c_BroadCast, SCHM_BeginModify, c_NoEventData);
Comp.UniqueId := Uid;
SchServer.RobotManager.SendMessage(Comp.I_ObjectAddress, c_BroadCast, SCHM_EndModify, c_NoEventData);
SchServer.ProcessControl.PostProcess(Sch, '');
Sch.GraphicallyInvalidate;
Emit('placed ' + Comp.Designator.Text + ' (' + Comp.LibReference + ' from ' + Comp.SourceLibraryName + ') at ' + IntToStr(CoordToMils(Comp.Location.X)) + ',' + IntToStr(CoordToMils(Comp.Location.Y)) + ', unique id ' + Comp.UniqueId);
