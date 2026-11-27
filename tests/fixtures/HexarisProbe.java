// Compare flow and decompilation for an independently compiled test function.
// @category Hexaris
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.block.BasicBlockModel;

public class HexarisProbe extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] arguments = getScriptArgs();
        if (arguments.length != 1) {
            throw new IllegalArgumentException("Pass a fixture virtual address in hexadecimal");
        }
        var address = toAddr(Long.parseUnsignedLong(arguments[0], 16));
        var function = currentProgram.getFunctionManager().getFunctionAt(address);
        if (function == null) {
            throw new IllegalStateException("No function starts at " + address);
        }
        println("FIXTURE function " + function.getName() + " at " + address);
        var instructions = currentProgram.getListing().getInstructions(function.getBody(), true);
        while (instructions.hasNext()) {
            var instruction = instructions.next();
            println("FIXTURE instruction " + instruction.getAddress() + " " + instruction);
        }
        var blocks = new BasicBlockModel(currentProgram).getCodeBlocksContaining(function.getBody(), monitor);
        while (blocks.hasNext()) {
            var block = blocks.next();
            println("FIXTURE block " + block.getFirstStartAddress());
            var destinations = block.getDestinations(monitor);
            while (destinations.hasNext()) {
                var edge = destinations.next();
                println("FIXTURE edge " + block.getFirstStartAddress() + " " + edge.getDestinationAddress());
            }
        }
        var references = currentProgram.getReferenceManager().getReferencesTo(address);
        while (references.hasNext()) {
            var reference = references.next();
            println("FIXTURE incoming " + reference.getFromAddress() + " " + reference.getReferenceType());
        }
        var decompiler = new DecompInterface();
        try {
            if (!decompiler.openProgram(currentProgram)) {
                throw new IllegalStateException("Ghidra could not open the program for decompilation");
            }
            var result = decompiler.decompileFunction(function, 10, monitor);
            if (!result.decompileCompleted()) {
                throw new IllegalStateException(result.getErrorMessage());
            }
            println("FIXTURE pseudocode\n" + result.getDecompiledFunction().getC());
        } finally {
            decompiler.dispose();
        }
    }
}
