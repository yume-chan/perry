// Simulate the pattern from printEasyHelp
function printEasyHelp(options: readonly string[]) {
    const output: string[] = ["header"];
    
    // Nested function that captures and modifies output
    function example(ex: string, desc: string) {
        output.push("  " + ex);
        output.push("  " + desc);
    }
    
    // Nested function with default parameter that is a closure
    function processOption(opt: string = (() => "default")()) {
        example(opt, "description");
    }
    
    example("tsc", "Compiles");
    processOption("--init");
    
    for (const line of output) {
        console.log(line);
    }
}

printEasyHelp(["--version", "--help"]);
