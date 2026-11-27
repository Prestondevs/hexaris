/* Small MSVC x64 executable for illustrating the workbench and call browser. */
#define FUNCTION __declspec(dllexport) __declspec(noinline)

typedef struct ReadingWindow {
    unsigned first;
    unsigned second;
    unsigned third;
    unsigned threshold;
} ReadingWindow;

FUNCTION unsigned clamp_reading(unsigned value) {
    return value > 100u ? 100u : value;
}

FUNCTION unsigned mean_reading(const ReadingWindow *window) {
    return (window->first + window->second + window->third) / 3u;
}

FUNCTION unsigned peak_reading(const ReadingWindow *window) {
    unsigned peak = window->first;
    if (window->second > peak) peak = window->second;
    if (window->third > peak) peak = window->third;
    return peak;
}

FUNCTION unsigned weighted_score(unsigned mean, unsigned peak) {
    return (mean * 3u + peak) / 4u;
}

FUNCTION unsigned score_window(const ReadingWindow *window) {
    unsigned mean = mean_reading(window);
    unsigned peak = peak_reading(window);
    unsigned score;
    if (peak > window->threshold) {
        score = weighted_score(mean, peak);
    } else if (mean < 12u) {
        score = mean * 2u;
    } else {
        score = mean + 3u;
    }
    return clamp_reading(score);
}

FUNCTION unsigned score_primary(void) {
    static const ReadingWindow window = {17, 42, 28, 36};
    return score_window(&window);
}

FUNCTION unsigned score_secondary(void) {
    static const ReadingWindow window = {9, 10, 7, 35};
    return score_window(&window);
}

FUNCTION unsigned score_recheck(void) {
    static const ReadingWindow window = {12, 15, 19, 38};
    return score_window(&window);
}

unsigned entry(void) {
    return score_primary() + score_secondary() + score_recheck();
}
