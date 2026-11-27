__declspec(noinline) unsigned helper(unsigned value) {
    return value + 1u;
}

/* Retained by the linker, but unreachable from entry. */
__declspec(noinline) unsigned hidden_unwind(unsigned value) {
    volatile unsigned result = helper(value);
    return result + 2u;
}

__declspec(dllexport) unsigned exported_leaf(unsigned value) {
    return value ^ 42u;
}

unsigned entry(void) {
    return helper(12u);
}
