// Export repeatable metadata from the original Windows score fixture.
// @category Hexaris
import java.io.File;
import ghidra.app.script.GhidraScript;
import ghidra.app.util.exporter.XmlExporter;
import ghidra.program.model.symbol.SourceType;

public class ExportFixtureXml extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] arguments = getScriptArgs();
        if (arguments.length != 2) {
            throw new IllegalArgumentException("Pass fixture address and output XML path");
        }
        var address = toAddr(Long.parseUnsignedLong(arguments[0], 16));
        var function = currentProgram.getFunctionManager().getFunctionAt(address);
        if (function == null) {
            throw new IllegalStateException("No function at " + address);
        }
        function.setName("fixture_score", SourceType.USER_DEFINED);
        setEOLComment(address, "Unsigned comparison & return");
        currentProgram.getBookmarkManager().setBookmark(address, "Analysis", "Fixture", "Score entry");
        var exporter = new XmlExporter();
        if (!exporter.export(new File(arguments[1]), currentProgram, currentProgram.getMemory(), monitor)) {
            throw new IllegalStateException(exporter.getMessageLog().toString());
        }
        println("HEXARIS_XML exported the original fixture");
    }
}
