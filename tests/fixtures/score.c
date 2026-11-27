/* Small freestanding fixture. Its branches and direct calls are intentional. */
__declspec(noinline) unsigned score(unsigned value) {
    if (value >= 10u) {
        return value + 3u;
    }
    return value * 2u;
}

unsigned entry(void) {
    volatile unsigned input = 12u;
    return score(input);
}
